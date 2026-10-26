mod entry;
mod feedback;
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
    Decision, EventLog, FeedbackKind, FeedbackTarget, ReviewEvent, ReviewSession, ValidatedReview,
};

use self::prompt::Prompt;

pub struct ReviewTui {
    pub(super) review: ValidatedReview,
    pub(super) log: EventLog,
    pub(super) session: ReviewSession,
    pub(super) stage: usize,
    pub(super) scroll: usize,
    pub(super) show_help: bool,
    /// Which of the current chunk's feedback the reviewer has picked out.
    /// Resolving and deleting act on this one item, so they are offered only
    /// while something is selected.
    pub(super) selected: Option<usize>,
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
            selected: None,
            prompt: None,
            exit: false,
        })
    }

    pub fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        // Bracketed paste keeps a pasted comment whole: without it the terminal
        // replays the paste as key presses, and its line breaks read as Enter.
        execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
        // Plain terminals send the same bytes for Enter and Shift+Enter, so a
        // newline inside a comment is only offered where the keyboard protocol
        // can tell them apart.
        let enhanced = supports_keyboard_enhancement().unwrap_or(false);
        if enhanced {
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }
        let _guard = TerminalGuard { enhanced };
        loop {
            self.draw()?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Release {
                continue;
            }
            if key.code == KeyCode::Char('q') {
                break;
            }
            self.handle(key)?;
            if self.exit {
                break;
            }
        }
        Ok(())
    }

    fn handle(&mut self, key: KeyEvent) -> Result<()> {
        if self.show_help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.show_help = false;
            }
            return Ok(());
        }
        if key.code == KeyCode::Char('?') {
            self.show_help = true;
            self.scroll = 0;
            return Ok(());
        }
        if self.selected.is_some() {
            return self.handle_selection(key.code);
        }
        match key.code {
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
            KeyCode::Char('c') if self.chunk_index().is_some() => {
                self.add_feedback(FeedbackKind::Note, FeedbackTarget::Code, None)?;
            }
            KeyCode::Char('!') if self.chunk_index().is_some() => {
                self.request_change()?;
            }
            KeyCode::Char('f') if self.chunk_index().is_some() => {
                self.select_first_feedback();
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
                    self.scroll = 0;
                }
            }
            KeyCode::Char('g') if self.is_summary() => {
                if let Some(index) = self.first_unresolved_chunk() {
                    self.stage = index + 1;
                    self.scroll = 0;
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
            KeyCode::Esc | KeyCode::Char('f') => self.selected = None,
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => self.move_selection(1),
            KeyCode::Char('x') => self.toggle_selected_resolved()?,
            KeyCode::Char('t') => self.retarget_selected()?,
            KeyCode::Char('d') => self.delete_selected()?,
            code => self.scroll_by(code),
        }
        Ok(())
    }

    /// Leaving a chunk by Enter: the reviewer is done with it.
    ///
    /// Someone who read a chunk and had nothing to say should not have to also
    /// say so, so this records what pressing on already means. It is confined to
    /// Enter, and to a chunk they left no feedback on: `n` and `→` stay pure
    /// navigation, because being marked reviewed decides whether the harness
    /// generates more, and skimming a batch to get oriented must not claim work
    /// nobody did. Feedback of any kind leaves the chunk for `r` to settle —
    /// the reviewer said something about it, so what they meant is theirs.
    fn finish_chunk(&mut self) -> Result<()> {
        if self.chunk_index().is_some() {
            let id = self.current_chunk_id();
            if self.chunk_feedback().is_empty() && !self.session.reviewed.contains(&id) {
                self.record(ReviewEvent::reviewed(id, true))?;
            }
        }
        self.next()
    }

    /// Settling the review, from the summary.
    ///
    /// The decision is read off the feedback rather than picked: a reviewer who
    /// has filed a request is requesting changes, and asking them to say so
    /// again only lets the two disagree. Nothing is confirmed and nothing is
    /// lost — settling a review still being written says the batch is done, and
    /// the harness carries on from there.
    ///
    /// Pressing on once it is settled leaves the review: the reviewer has
    /// nothing left to say here, and the key that carried them through the
    /// walkthrough should not go dead on the last screen.
    fn finish_review(&mut self) -> Result<()> {
        if self.ended() {
            self.exit = true;
            return Ok(());
        }
        self.record(ReviewEvent::decision(self.outcome()))
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
        self.stage = (self.stage + 1).min(last);
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
        self.selected = None;
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
        Change, ChangePresentation, ChunkBlock, ChunkKind, Feedback, Overview, Quote, ReviewChunk,
        ReviewInput, ReviewSpec, SourceIdentity,
    };

    use super::{
        Decision, EventLog, FeedbackKind, FeedbackTarget, KeyCode, KeyEvent, ReviewEvent,
        ReviewTui, ValidatedReview,
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

        tui.handle(press(KeyCode::Enter)).expect("end the review");

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

        tui.handle(press(KeyCode::Enter)).expect("end the review");

        assert_eq!(tui.session.decision, Some(Decision::Comment));
    }

    #[test]
    fn a_chunk_the_reviewer_remarked_on_is_left_for_them_to_settle() {
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

        assert!(!tui.session.reviewed.contains("chunk-0"));
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
}
