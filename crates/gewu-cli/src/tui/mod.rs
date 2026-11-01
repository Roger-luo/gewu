mod entry;
mod feedback;
mod frame;
mod help;
mod highlight;
mod markdown;
mod prompt;
mod render;
mod render_support;
mod style;
mod view;

use std::{io, path::PathBuf};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use gewu_review::{
    Decision, EventLog, FeedbackKind, LineAnchor, ReviewEvent, ReviewSession, ValidatedReview,
};

use self::{
    prompt::Prompt,
    render::{generation_progress, pending_generation_warning},
    style::{Document, TextStyle},
};

pub struct ReviewTui {
    pub(super) review: ValidatedReview,
    pub(super) log: EventLog,
    pub(super) session: ReviewSession,
    pub(super) stage: usize,
    pub(super) scroll: usize,
    pub(super) show_help: bool,
    /// A navigable table of contents for non-linear review.
    pub(super) show_outline: bool,
    pub(super) outline_selection: usize,
    /// Shows literal diff/source rows instead of a rendered document.
    pub(super) show_raw_source: bool,
    /// The line picker exposes its current choice to the ordinary renderer so
    /// the reviewer sees a marked source row, not only a number in a prompt.
    pub(super) selected_line: Option<LineAnchor>,
    /// Which of the current chunk's feedback the reviewer has picked out.
    /// Resolving and deleting act on this one item, so they are offered only
    /// while something is selected.
    pub(super) selected: Option<usize>,
    /// The viewport before feedback selection expanded the document.
    pub(super) selection_origin_scroll: Option<usize>,
    /// Rows the last drawn page showed. Paging advances by no more than this,
    /// because a page shortened to keep a label with its content would
    /// otherwise skip the rows it held back.
    pub(super) shown_rows: Option<usize>,
    /// PageDown may leave the last page short instead of snapping back to a
    /// full one: the self-review UI judge found the snap re-showing most of
    /// the previous page as a near-duplicate.
    pub(super) paged_past_end: bool,
    /// The last row of the current document with content, from the last draw.
    pub(super) last_content_row: Option<usize>,
    /// A one-key-lived confirmation or result shown after an action.
    pub(super) notice: Option<String>,
    /// Ending with unreviewed chunks takes a deliberate second Enter.
    pub(super) confirm_finish: bool,
    /// What is being asked or written at the foot of the screen, if anything.
    prompt: Option<Prompt>,
    /// Set when the reviewer is finished, so the loop can end without every
    /// key having to report whether it was the last one.
    exit: bool,
}

impl ReviewTui {
    pub fn new(review: ValidatedReview, session_path: PathBuf) -> Result<Self> {
        let log = EventLog::new(session_path);
        let mut session = log.load()?;
        let spec_id = review.spec().session_key();
        if let Some(existing) = &session.spec_id {
            anyhow::ensure!(
                existing == &spec_id,
                "session belongs to review {existing}, not {spec_id}"
            );
        } else {
            let event = ReviewEvent::started(spec_id);
            log.append(&event)?;
            session.apply(&event);
        }
        Ok(Self {
            session,
            review,
            log,
            stage: 0,
            scroll: 0,
            show_help: false,
            show_outline: false,
            outline_selection: 0,
            show_raw_source: false,
            selected_line: None,
            selected: None,
            selection_origin_scroll: None,
            shown_rows: None,
            paged_past_end: false,
            last_content_row: None,
            notice: None,
            confirm_finish: false,
            prompt: None,
            exit: false,
        })
    }

    pub fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        // Bracketed paste keeps a pasted comment whole: without it the terminal
        // replays the paste as key presses, and its line breaks read as Enter.
        execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
        // Enhanced terminals can distinguish Shift+Enter. Plain terminals
        // still have Ctrl+n as the explicit newline fallback.
        let enhanced = supports_keyboard_enhancement().unwrap_or(false);
        if enhanced {
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }
        let _guard = TerminalGuard { enhanced };
        let mut quit = false;
        {
            loop {
                self.draw()?;
                let Event::Key(key) = event::read()? else {
                    continue;
                };
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if key.code == KeyCode::Char('q') {
                    quit = true;
                    break;
                }
                self.handle(key)?;
                if self.exit {
                    break;
                }
            }
        }
        drop(_guard);
        self.print_exit_summary(quit);
        Ok(())
    }

    fn handle(&mut self, key: KeyEvent) -> Result<()> {
        if !is_reading_navigation(key.code) {
            self.notice = None;
        }
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => {
                    self.show_help = false;
                    self.scroll = 0;
                }
                KeyCode::PageDown => self.page_help(true),
                KeyCode::PageUp => self.page_help(false),
                code => self.scroll_by(code),
            }
            return Ok(());
        }
        if self.show_outline {
            return self.handle_outline(key.code);
        }
        if key.code == KeyCode::Char('?') {
            self.show_help = true;
            self.scroll = 0;
            return Ok(());
        }
        if key.code == KeyCode::Char('o') {
            self.open_outline();
            return Ok(());
        }
        if self.selected.is_some() {
            return self.handle_selection(key.code);
        }
        match key.code {
            KeyCode::Esc if self.is_summary() && self.confirm_finish => {
                let partial_approval = self.partial_approval_pending();
                self.confirm_finish = false;
                self.notice = Some(if partial_approval {
                    "Batch approval cancelled; keep reviewing or quit to preserve progress".into()
                } else {
                    "End cancelled; keep reviewing or quit to preserve progress".into()
                });
            }
            KeyCode::Enter if self.is_summary() => self.finish_review()?,
            KeyCode::Enter => self.finish_chunk()?,
            KeyCode::Char('n') | KeyCode::Right => self.next()?,
            KeyCode::Char('p') | KeyCode::Left => self.previous(),
            KeyCode::Char('j') => self.scroll_by(KeyCode::Down),
            KeyCode::Char('k') => self.scroll_by(KeyCode::Up),
            KeyCode::Char('r') if self.chunk_index().is_some() => {
                let id = self.current_chunk_id();
                let reviewed = !self.session.reviewed.contains(&id);
                self.record(ReviewEvent::reviewed(id, reviewed))?;
            }
            KeyCode::Char('c') if self.chunk_index().is_some() => self.comment()?,
            KeyCode::Char('!') if self.chunk_index().is_some() => {
                self.request_change()?;
            }
            KeyCode::Char('f') if self.chunk_index().is_some() => {
                self.select_first_feedback();
            }
            KeyCode::Char('v') if self.chunk_index().is_some() => {
                self.show_raw_source = !self.show_raw_source;
                self.scroll = 0;
                self.notice = Some(if self.show_raw_source {
                    "Showing raw source with line numbers".into()
                } else {
                    "Showing authored source presentation".into()
                });
            }
            KeyCode::Char('s') if self.chunk_index().is_some() => {
                self.scroll_to_first_source();
            }
            KeyCode::Char('z') if self.chunk_index().is_some() => {
                self.scroll_to_context();
            }
            KeyCode::Char('u') if self.is_summary() => {
                if let Some(index) = self
                    .review
                    .spec()
                    .chunks
                    .iter()
                    .position(|chunk| !self.session.reviewed.contains(&chunk.id))
                {
                    self.stage = index + 1;
                    self.leave_chunk();
                    self.record(ReviewEvent::visited(self.current_chunk_id()))?;
                    self.scroll = 0;
                }
            }
            KeyCode::Char('g') if self.is_summary() => {
                if let Some((index, feedback_index)) = self.first_unresolved_feedback() {
                    self.stage = index + 1;
                    self.leave_chunk();
                    self.selected = Some(feedback_index);
                    self.record(ReviewEvent::visited(self.current_chunk_id()))?;
                    self.scroll_to_selected_feedback();
                    self.notice = Some("Focused the first open request".into());
                }
            }
            code => self.scroll_by(code),
        }
        Ok(())
    }

    /// Keys while one piece of feedback is picked out. Resolving and deleting
    /// live only here, so neither can be aimed at feedback the reviewer is not
    /// looking at.
    fn handle_selection(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc | KeyCode::Char('f') => {
                self.selected = None;
                self.scroll = self.selection_origin_scroll.take().unwrap_or(self.scroll);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(-1);
                self.scroll_to_selected_feedback();
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.move_selection(1);
                self.scroll_to_selected_feedback();
            }
            KeyCode::Char('x')
                if self
                    .selected_feedback()
                    .is_some_and(|feedback| feedback.kind == FeedbackKind::Issue) =>
            {
                self.toggle_selected_resolved()?;
            }
            KeyCode::Char('t') => self.retarget_selected()?,
            KeyCode::Char('d') => self.delete_selected()?,
            code => self.scroll_by(code),
        }
        Ok(())
    }

    /// Leaving a chunk by Enter: the reviewer is done with it.
    ///
    /// Someone who read a chunk should not have to use a different completion
    /// key merely because they also left feedback. Enter always means the same
    /// thing: mark this chunk reviewed and continue. `n` and `→` stay pure
    /// navigation, because skimming ahead must not claim work nobody did.
    fn finish_chunk(&mut self) -> Result<()> {
        if self.chunk_index().is_some() {
            let id = self.current_chunk_id();
            if !self.session.reviewed.contains(&id) {
                self.record(ReviewEvent::reviewed(id, true))?;
            }
        }
        self.next()
    }

    /// Settling the review, from the summary.
    ///
    /// The decision is read off the feedback rather than picked: a reviewer who
    /// has filed a request is requesting changes, and asking them to say so
    /// again only lets the two disagree. When chunks remain unreviewed, a first
    /// press makes that incompleteness explicit and a second confirms it. An
    /// approval while generation is pending gets the same guard even when all
    /// chunks in the current batch were reviewed: it cannot read as approval
    /// of source the reviewer has not been shown yet.
    ///
    /// Pressing on once it is settled leaves the review: the reviewer has
    /// nothing left to say here, and the key that carried them through the
    /// walkthrough should not go dead on the last screen.
    fn finish_review(&mut self) -> Result<()> {
        if self.ended() {
            self.exit = true;
            return Ok(());
        }
        let unreviewed = self.unreviewed_count();
        let partial_approval = self.partial_approval_pending();
        if (unreviewed > 0 || partial_approval) && !self.confirm_finish {
            self.confirm_finish = true;
            self.notice = Some(if partial_approval {
                let progress = generation_progress(&self.review)
                    .expect("pending generation has source coverage");
                let unreviewed = if unreviewed > 0 {
                    format!(" {unreviewed} generated chunk(s) are also unreviewed.")
                } else {
                    String::new()
                };
                format!(
                    "Approve only this generated batch covering {} of {} change hunks? {}{unreviewed} Enter=approve batch · Esc=cancel",
                    progress.covered_hunks,
                    progress.total_hunks,
                    pending_generation_warning(&progress),
                )
            } else {
                format!("End with {unreviewed} unreviewed? Enter=end · Esc=cancel")
            });
            return Ok(());
        }
        self.confirm_finish = false;
        self.record(ReviewEvent::decision(self.outcome()))
    }

    pub(super) fn unreviewed_count(&self) -> usize {
        self.review
            .spec()
            .chunks
            .iter()
            .filter(|chunk| !self.session.reviewed.contains(&chunk.id))
            .count()
    }

    /// What the review currently amounts to.
    pub(super) fn outcome(&self) -> Decision {
        self.session
            .outcome(self.review.spec().chunks.iter().map(|chunk| &chunk.id))
    }

    /// Whether the review has been ended as it now stands. Feedback left after
    /// the fact can change the outcome, and then it is no longer ended: the
    /// recorded decision would be describing a review that has moved on.
    pub(super) fn ended(&self) -> bool {
        self.session.decision == Some(self.outcome())
    }

    fn next(&mut self) -> Result<()> {
        let last = self.review.spec().chunks.len() + 1;
        self.stage = if self.stage == 0 {
            self.resume_chunk_index().map_or(1, |index| index + 1)
        } else {
            (self.stage + 1).min(last)
        };
        self.leave_chunk();
        if self.chunk_index().is_some() {
            self.record(ReviewEvent::visited(self.current_chunk_id()))?;
        }
        Ok(())
    }

    fn previous(&mut self) {
        self.stage = self.stage.saturating_sub(1);
        self.leave_chunk();
    }

    /// A selection names a position in one chunk's feedback, so it cannot
    /// survive leaving that chunk.
    fn leave_chunk(&mut self) {
        self.scroll = 0;
        self.paged_past_end = false;
        self.selected = None;
        self.selection_origin_scroll = None;
        self.confirm_finish = false;
    }

    fn open_outline(&mut self) {
        self.show_outline = true;
        self.outline_selection = self.chunk_index().unwrap_or(0);
        self.scroll = 0;
    }

    fn handle_outline(&mut self, code: KeyCode) -> Result<()> {
        let last = self.review.spec().chunks.len().saturating_sub(1);
        match code {
            KeyCode::Esc | KeyCode::Char('o') => {
                self.show_outline = false;
                self.scroll = 0;
            }
            KeyCode::Char('?') => {
                self.show_outline = false;
                self.show_help = true;
                self.scroll = 0;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.outline_selection = self.outline_selection.saturating_sub(1);
                self.scroll = self.outline_selection.saturating_sub(2);
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.outline_selection = (self.outline_selection + 1).min(last);
                self.scroll = self.outline_selection.saturating_sub(2);
            }
            KeyCode::Enter if !self.review.spec().chunks.is_empty() => {
                self.stage = self.outline_selection + 1;
                self.show_outline = false;
                self.leave_chunk();
                self.record(ReviewEvent::visited(self.current_chunk_id()))?;
            }
            code => self.scroll_by(code),
        }
        Ok(())
    }

    pub(super) fn record(&mut self, event: ReviewEvent) -> Result<()> {
        self.log.append(&event)?;
        self.session.apply(&event);
        Ok(())
    }

    fn chunk_index(&self) -> Option<usize> {
        (self.stage > 0 && self.stage <= self.review.spec().chunks.len())
            .then_some(self.stage.saturating_sub(1))
    }

    fn current_chunk_id(&self) -> String {
        self.review.spec().chunks[self.chunk_index().expect("current stage is a chunk")]
            .id
            .clone()
    }

    fn is_summary(&self) -> bool {
        self.stage == self.review.spec().chunks.len() + 1
    }

    pub(super) fn has_saved_progress(&self) -> bool {
        !self.session.visited.is_empty()
            || !self.session.reviewed.is_empty()
            || !self.session.feedback.is_empty()
            || self.session.decision.is_some()
    }

    fn resume_chunk_index(&self) -> Option<usize> {
        let chunks = &self.review.spec().chunks;
        if let Some(last) = self.session.last_visited.as_ref()
            && let Some(index) = chunks.iter().position(|chunk| &chunk.id == last)
            && !self.session.reviewed.contains(last)
        {
            return Some(index);
        }
        chunks
            .iter()
            .position(|chunk| !self.session.reviewed.contains(&chunk.id))
            .or_else(|| {
                self.session
                    .last_visited
                    .as_ref()
                    .and_then(|last| chunks.iter().position(|chunk| &chunk.id == last))
            })
    }

    pub(super) fn generation_pending(&self) -> bool {
        generation_progress(&self.review).is_some()
    }

    pub(super) fn partial_approval_pending(&self) -> bool {
        self.generation_pending() && self.outcome() == Decision::Approve
    }

    fn print_exit_summary(&self, quit: bool) {
        let reviewed = self
            .review
            .spec()
            .chunks
            .iter()
            .filter(|chunk| self.session.reviewed.contains(&chunk.id))
            .count();
        let feedback = self.session.feedback.len();
        if quit {
            print_terminal_message(&format!(
                "Progress saved: {reviewed}/{} reviewed · {feedback} feedback. Run `gewu review open` to resume.",
                self.review.spec().chunks.len()
            ));
        } else if let Some(decision) = self.session.decision {
            print_terminal_message(&format!(
                "Review recorded as {}: {reviewed}/{} reviewed · {feedback} feedback.",
                decision.label(),
                self.review.spec().chunks.len()
            ));
        }
    }
}

fn print_terminal_message(message: &str) {
    let width = crossterm::terminal::size().map_or(80, |(width, _)| usize::from(width));
    let mut document = Document::new();
    document.push(message, TextStyle::default());
    for line in document.wrapped(width).lines {
        println!(
            "{}",
            line.spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
        );
    }
}

fn is_reading_navigation(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Char('j' | 'k')
    )
}

struct TerminalGuard {
    enhanced: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if self.enhanced {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen);
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;
    use gewu_review::{
        Change, ChangePresentation, ChunkBlock, ChunkKind, Feedback, FeedbackTarget, Overview,
        Quote, ReviewChunk, ReviewInput, ReviewSpec, SourceIdentity,
    };

    use super::{
        Decision, EventLog, FeedbackKind, KeyCode, KeyEvent, ReviewEvent, ReviewTui,
        ValidatedReview,
    };

    fn review() -> ValidatedReview {
        let source = SourceIdentity::new("base", "head", b"diff");
        let changes: Vec<_> = ["a.rs", "b.rs"]
            .iter()
            .enumerate()
            .map(|(index, path)| {
                Change::new(
                    *path,
                    (0, 0, 1, 1),
                    format!("@@ -0,0 +1,1 @@\n+line {index}\n"),
                )
            })
            .collect();
        let chunks = changes
            .iter()
            .enumerate()
            .map(|(index, change)| ReviewChunk {
                id: format!("chunk-{index}"),
                title: "A chunk".into(),
                summary: "Summary.".into(),
                kind: ChunkKind::Other,
                content: vec![ChunkBlock::Changes {
                    quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
                    presentation: ChangePresentation::AddedCode,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
                section: None,
            })
            .collect();
        let spec = ReviewSpec::new(
            source.clone(),
            Overview {
                intent: "Intent.".into(),
                changes: vec![],
                illustration: None,
                concepts: vec![],
            },
            chunks,
        );
        ValidatedReview::validate_partial(ReviewInput::new(source, changes), spec)
            .expect("valid review")
    }

    fn review_with_context() -> ValidatedReview {
        let source = SourceIdentity::new("base", "head", b"diff");
        let change = Change::new("a.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+line\n");
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "A chunk".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Concept,
            content: vec![ChunkBlock::Changes {
                quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
                presentation: ChangePresentation::AddedCode,
            }],
            context: Some(gewu_review::ChunkContext::Concept {
                definition: "A definition that must remain oriented.".into(),
                example: "example()".into(),
                relation: "It explains this source.".into(),
            }),
            confidence: None,
            oversized_reason: None,
            section: None,
        };
        let spec = ReviewSpec::new(
            source.clone(),
            Overview {
                intent: "Intent.".into(),
                changes: vec![],
                illustration: None,
                concepts: vec![],
            },
            vec![chunk],
        );
        ValidatedReview::validate(ReviewInput::new(source, vec![change]), spec)
            .expect("valid review")
    }

    fn partial_review() -> ValidatedReview {
        let source = SourceIdentity::new("base", "head", b"diff");
        let first = Change::new("a.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+one\n");
        let second = Change::new("b.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+two\n");
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "First batch".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Other,
            content: vec![ChunkBlock::Changes {
                quotes: vec![Quote::whole(first.id.clone(), first.body_lines())],
                presentation: ChangePresentation::AddedCode,
            }],
            context: None,
            confidence: None,
            oversized_reason: None,
            section: None,
        };
        let spec = ReviewSpec::new(
            source.clone(),
            Overview {
                intent: "Intent.".into(),
                changes: vec![],
                illustration: None,
                concepts: vec![],
            },
            vec![chunk],
        )
        .with_remaining_work(1);
        ValidatedReview::validate_partial(ReviewInput::new(source, vec![first, second]), spec)
            .expect("valid partial review")
    }

    /// A reviewer at the first chunk, having read nothing else.
    fn opened(directory: &tempfile::TempDir) -> ReviewTui {
        let mut tui = ReviewTui::new(review(), directory.path().join("events.jsonl"))
            .expect("open the review");
        tui.handle(press(KeyCode::Enter))
            .expect("leave the overview");
        assert!(
            tui.session.reviewed.is_empty(),
            "the overview is not a chunk"
        );
        tui
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn remark(kind: FeedbackKind) -> Feedback {
        Feedback {
            id: "feedback-1".into(),
            chunk_id: "chunk-0".into(),
            kind,
            target: FeedbackTarget::Code,
            body: "Something to say.".into(),
            anchor: None,
            resolved: false,
        }
    }

    #[test]
    fn pressing_on_from_a_chunk_with_nothing_to_say_marks_it_reviewed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);

        tui.handle(press(KeyCode::Enter)).expect("advance");

        assert!(tui.session.reviewed.contains("chunk-0"));
        // Durable, not just on screen: the harness reads the log, not the UI.
        let replayed = EventLog::new(directory.path().join("events.jsonl"))
            .load()
            .expect("replay");
        assert!(replayed.reviewed.contains("chunk-0"));
    }

    /// Navigation must stay navigation. Being marked reviewed decides whether
    /// the harness generates more, so reading ahead cannot claim it.
    #[test]
    fn moving_between_chunks_claims_nothing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);

        tui.handle(press(KeyCode::Char('n'))).expect("next");
        tui.handle(press(KeyCode::Left)).expect("back");
        tui.handle(press(KeyCode::Right)).expect("forward again");

        assert!(
            tui.session.reviewed.is_empty(),
            "{:?}",
            tui.session.reviewed
        );
    }

    #[test]
    fn help_scrolls_instead_of_swallowing_page_keys() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.handle(press(KeyCode::Char('?'))).expect("open help");

        tui.handle(press(KeyCode::PageDown)).expect("scroll help");

        assert!(tui.show_help);
        assert!(tui.scroll > 0, "PageDown must move overflowing help");
    }

    /// Independent review found that the context column scrolled to an
    /// unlabeled tail after reflow. The sidebar is now a fresh, sticky document
    /// while only the main review body owns the shared scroll offset.
    #[test]
    fn wide_context_is_a_sticky_sidebar_not_part_of_the_scrolled_body() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = ReviewTui::new(review_with_context(), directory.path().join("events.jsonl"))
            .expect("open review");
        tui.handle(press(KeyCode::Enter)).expect("leave overview");

        let sidebar: String = tui
            .sidebar_document(150)
            .expect("wide context sidebar")
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();
        let body: String = tui
            .current_document(150)
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(sidebar.starts_with("Context · New concept"), "{sidebar}");
        assert!(sidebar.contains("In this change"), "{sidebar}");
        assert!(!body.contains("A definition that must remain oriented."));
    }

    #[test]
    fn narrow_context_keeps_its_identity_pinned_after_its_heading_scrolls_away() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = ReviewTui::new(review_with_context(), directory.path().join("events.jsonl"))
            .expect("open review");
        tui.handle(press(KeyCode::Enter)).expect("leave overview");
        tui.scroll = usize::MAX;

        let pinned: String = tui
            .pinned_document(80)
            .expect("chunk header")
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(pinned.contains("CONTEXT · New concept"), "{pinned}");
    }

    #[test]
    fn outline_jumps_directly_without_marking_skipped_chunks() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.handle(press(KeyCode::Char('o'))).expect("open outline");
        tui.handle(press(KeyCode::Down))
            .expect("select second chunk");

        tui.handle(press(KeyCode::Enter)).expect("jump");

        assert_eq!(tui.stage, 2);
        assert!(!tui.show_outline);
        assert!(tui.session.reviewed.is_empty());
        assert!(tui.session.visited.contains("chunk-1"));
    }

    #[test]
    fn ending_with_unreviewed_chunks_requires_a_second_enter_and_persists() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("events.jsonl");
        let mut tui = opened(&directory);
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("visit only");
        }

        tui.handle(press(KeyCode::Enter)).expect("warn");
        assert_eq!(tui.session.decision, None);
        assert!(tui.confirm_finish);
        assert!(
            tui.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("unreviewed"))
        );

        tui.handle(press(KeyCode::Enter)).expect("confirm");
        assert_eq!(tui.session.decision, Some(Decision::Approve));
        assert_eq!(
            EventLog::new(path).load().expect("replay").decision,
            Some(Decision::Approve)
        );
    }

    /// Independent review found that an approval covering only the generated
    /// batch was durable after one Enter even though most source was pending.
    #[test]
    fn partial_generation_approval_requires_a_second_enter() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = ReviewTui::new(partial_review(), directory.path().join("events.jsonl"))
            .expect("open review");
        tui.handle(press(KeyCode::Enter)).expect("open first chunk");
        tui.handle(press(KeyCode::Enter)).expect("review batch");
        assert!(tui.is_summary());

        tui.handle(press(KeyCode::Enter))
            .expect("warn about partial approval");

        assert_eq!(tui.session.decision, None);
        assert!(tui.confirm_finish);
        assert!(
            tui.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("1 of 2 change hunks")),
            "{:?}",
            tui.notice
        );

        tui.handle(press(KeyCode::Enter))
            .expect("confirm batch approval");

        assert_eq!(tui.session.decision, Some(Decision::Approve));
    }

    #[test]
    fn returning_to_unreviewed_work_cancels_end_review_confirmation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("visit only");
        }
        tui.handle(press(KeyCode::Enter)).expect("warn");
        assert!(tui.confirm_finish);

        tui.handle(press(KeyCode::Char('u')))
            .expect("return to unreviewed chunk");

        assert!(!tui.confirm_finish);
        assert_eq!(tui.stage, 1);
        assert!(tui.session.visited.contains("chunk-0"));
        assert_eq!(
            tui.scroll, 0,
            "unreviewed work should reopen at its introduction"
        );
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("visit only");
        }
        tui.handle(press(KeyCode::Enter)).expect("warn again");
        assert_eq!(tui.session.decision, None);
        assert!(tui.confirm_finish);
    }

    #[test]
    fn escape_disarms_an_incomplete_review_confirmation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("visit only");
        }
        tui.handle(press(KeyCode::Enter)).expect("warn");

        tui.handle(press(KeyCode::Esc)).expect("cancel");

        assert!(!tui.confirm_finish);
        assert_eq!(tui.session.decision, None);
        assert!(
            tui.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("cancelled"))
        );
    }

    #[test]
    fn reopening_resumes_at_the_first_unreviewed_chunk() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui =
            ReviewTui::new(review(), directory.path().join("events.jsonl")).expect("open review");
        tui.record(ReviewEvent::visited(String::from("chunk-0")))
            .expect("visit first");
        tui.record(ReviewEvent::reviewed(String::from("chunk-0"), true))
            .expect("review first");

        tui.handle(press(KeyCode::Enter)).expect("resume");

        assert_eq!(tui.stage, 2, "resume should skip the reviewed first chunk");
        assert!(tui.session.visited.contains("chunk-1"));
    }

    #[test]
    fn open_request_jump_selects_the_request_it_names() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.record(ReviewEvent::feedback(Feedback {
            id: "later-request".into(),
            chunk_id: "chunk-1".into(),
            kind: FeedbackKind::Issue,
            target: FeedbackTarget::Code,
            body: "Fix the second chunk.".into(),
            anchor: None,
            resolved: false,
        }))
        .expect("request");
        tui.stage = tui.review.spec().chunks.len() + 1;

        tui.handle(press(KeyCode::Char('g'))).expect("jump");

        assert_eq!(tui.stage, 2);
        assert_eq!(
            tui.selected_feedback().map(|feedback| feedback.id.as_str()),
            Some("later-request")
        );
        assert!(
            tui.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("Focused"))
        );
    }

    /// Independent review found that leaving a request reached with `g`
    /// snapped back to the chunk introduction instead of keeping the request.
    #[test]
    fn closing_an_open_request_jump_keeps_the_request_in_view() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.record(ReviewEvent::feedback(Feedback {
            id: "later-request".into(),
            chunk_id: "chunk-1".into(),
            kind: FeedbackKind::Issue,
            target: FeedbackTarget::Code,
            body: "Fix the second chunk.".into(),
            anchor: None,
            resolved: false,
        }))
        .expect("request");
        tui.stage = tui.review.spec().chunks.len() + 1;
        tui.handle(press(KeyCode::Char('g'))).expect("jump");
        let request_scroll = tui.scroll;
        assert!(request_scroll > 0, "request should be below the chunk top");

        tui.handle(press(KeyCode::Esc)).expect("close selection");

        assert_eq!(tui.scroll, request_scroll);
        assert!(tui.selected.is_none());
    }

    #[test]
    fn closing_feedback_selection_restores_the_reading_position() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.record(ReviewEvent::feedback(remark(FeedbackKind::Note)))
            .expect("record feedback");
        tui.scroll = 7;

        tui.handle(press(KeyCode::Char('f')))
            .expect("select feedback");
        assert!(tui.selected.is_some());
        tui.scroll = 19;
        tui.handle(press(KeyCode::Esc)).expect("close selection");

        assert_eq!(tui.scroll, 7);
        assert!(tui.selected.is_none());
    }

    /// Pressing on to the end of a review nobody had anything to say about is
    /// an approval; making them then pick "approve" from a list asks them to
    /// state a conclusion they have already reached by getting there.
    #[test]
    fn pressing_on_from_the_summary_ends_the_review_as_it_stands() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        for _ in 0..2 {
            tui.handle(press(KeyCode::Enter)).expect("through a chunk");
        }
        assert!(tui.is_summary(), "two chunks then the summary");
        assert_eq!(tui.session.decision, None);

        tui.handle(press(KeyCode::Enter)).expect("end the review");

        assert_eq!(tui.session.decision, Some(Decision::Approve));
    }

    /// The key that carried the reviewer through the walkthrough must not go
    /// dead on the last screen.
    #[test]
    fn pressing_on_from_an_ended_review_leaves_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Enter)).expect("through a chunk");
        }
        tui.handle(press(KeyCode::Enter)).expect("end the review");
        assert_eq!(tui.session.decision, Some(Decision::Approve));
        assert!(!tui.exit, "recording is not leaving");

        tui.handle(press(KeyCode::Enter)).expect("leave");

        assert!(tui.exit);
    }

    #[test]
    fn recorded_incomplete_review_advertises_revisit_controls() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("advance");
        }
        tui.handle(press(KeyCode::Enter)).expect("warn");
        tui.handle(press(KeyCode::Enter)).expect("record");

        let footer: String = tui
            .footer_command_document(80)
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(footer.contains("[u] Unreviewed"), "{footer}");
        assert!(footer.contains("[←/p] Previous"), "{footer}");
    }

    /// Feedback left after the fact changes what the review amounts to, so the
    /// recorded decision no longer describes it and Enter records again.
    #[test]
    fn feedback_after_the_end_reopens_the_outcome_rather_than_leaving() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Enter)).expect("through a chunk");
        }
        tui.handle(press(KeyCode::Enter)).expect("end the review");
        tui.record(ReviewEvent::feedback(remark(FeedbackKind::Issue)))
            .expect("a late request");

        tui.handle(press(KeyCode::Enter))
            .expect("record the new outcome");

        assert!(!tui.exit, "{:?}", tui.session.decision);
        assert_eq!(tui.session.decision, Some(Decision::RequestChanges));
    }

    #[test]
    fn an_open_request_ends_the_review_as_request_changes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.record(ReviewEvent::feedback(remark(FeedbackKind::Issue)))
            .expect("record a request");
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("advance");
        }

        tui.handle(press(KeyCode::Enter)).expect("show warning");
        assert_eq!(tui.session.decision, None);
        tui.handle(press(KeyCode::Enter))
            .expect("confirm the review");

        assert_eq!(tui.session.decision, Some(Decision::RequestChanges));
    }

    #[test]
    fn a_remark_alone_ends_the_review_as_a_comment() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.record(ReviewEvent::feedback(remark(FeedbackKind::Note)))
            .expect("record a remark");
        while !tui.is_summary() {
            tui.handle(press(KeyCode::Char('n'))).expect("advance");
        }

        tui.handle(press(KeyCode::Enter)).expect("show warning");
        tui.handle(press(KeyCode::Enter))
            .expect("confirm the review");

        assert_eq!(tui.session.decision, Some(Decision::Comment));
    }

    #[test]
    fn enter_marks_a_chunk_reviewed_even_when_it_has_feedback() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.record(ReviewEvent::feedback(Feedback {
            id: "feedback-1".into(),
            chunk_id: "chunk-0".into(),
            kind: FeedbackKind::Issue,
            target: FeedbackTarget::Code,
            body: "Rework this.".into(),
            anchor: None,
            resolved: false,
        }))
        .expect("record feedback");

        tui.handle(press(KeyCode::Enter)).expect("advance");

        assert!(tui.session.reviewed.contains("chunk-0"));
    }

    #[test]
    fn saving_feedback_confirms_its_target_and_scrolls_it_into_view() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);

        tui.save_feedback(
            FeedbackKind::Issue,
            FeedbackTarget::Explanation,
            None,
            "Clarify the claim.".into(),
        )
        .expect("save feedback");

        assert_eq!(tui.chunk_feedback().len(), 1);
        assert_eq!(
            tui.notice.as_deref(),
            Some("Saved request about this explanation")
        );
        assert!(
            tui.scroll > 0,
            "new feedback should be brought to the viewport end"
        );

        tui.handle(press(KeyCode::End))
            .expect("inspect lower content");
        assert_eq!(
            tui.notice.as_deref(),
            Some("Saved request about this explanation"),
            "scrolling should not erase the save target before it is checked"
        );
    }

    #[test]
    fn line_picker_offers_only_quoted_file_locations() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let tui = opened(&directory);

        let anchors = tui.line_anchors();

        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].path, "a.rs");
        assert_eq!(anchors[0].start_line, 1);
    }

    /// Advancing twice off the same chunk must not append a second event.
    #[test]
    fn a_chunk_already_settled_is_not_recorded_again() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = opened(&directory);
        tui.handle(press(KeyCode::Enter)).expect("advance");
        tui.handle(press(KeyCode::Left)).expect("back");

        tui.handle(press(KeyCode::Enter)).expect("advance again");

        let log = std::fs::read_to_string(directory.path().join("events.jsonl")).expect("log");
        assert_eq!(
            log.lines()
                .filter(|line| line.contains("review_changed"))
                .count(),
            1
        );
    }
    #[test]
    fn context_and_source_jumps_are_direct_without_marking_reviewed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = ReviewTui::new(review_with_context(), directory.path().join("events.jsonl"))
            .expect("open");
        tui.handle(press(KeyCode::Char('n'))).expect("chunk");
        tui.handle(press(KeyCode::Char('z'))).expect("context");
        assert!(tui.scroll > 0, "context should be directly reachable");
        tui.handle(press(KeyCode::Char('s'))).expect("source");
        let source_scroll = tui.scroll;
        assert!(source_scroll > 0);
        tui.handle(press(KeyCode::Home)).expect("introduction");
        assert_eq!(tui.scroll, 0);
        assert!(tui.session.reviewed.is_empty());
        assert_eq!(tui.session.decision, None);
    }

    #[test]
    fn narrow_chunk_controls_fit_one_row_and_offer_context() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut tui = ReviewTui::new(review_with_context(), directory.path().join("events.jsonl"))
            .expect("open");
        tui.handle(press(KeyCode::Char('n'))).expect("chunk");
        let footer = tui.footer_command_document(65);
        assert_eq!(footer.lines.len(), 1);
        let text: String = footer
            .lines
            .iter()
            .flat_map(|l| &l.spans)
            .map(|s| s.text.as_str())
            .collect();
        assert!(text.contains("[s/z] Src/ctx"), "{text}");
        assert!(text.contains("[p/n] Chunks"), "{text}");
        assert!(text.contains("[q] Quit"), "{text}");
        assert!(text.contains("Review+next"), "{text}");
    }
}
