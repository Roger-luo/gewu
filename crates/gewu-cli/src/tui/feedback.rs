use anyhow::Result;
use crossterm::event::KeyCode;
use gewu_review::{
    ChangePresentation, ChunkBlock, Feedback, FeedbackKind, FeedbackTarget, LineAnchor, ReviewEvent,
};

use super::ReviewTui;

/// What a request is aimed at, once the reviewer has said.
enum Scope {
    /// The code, as a whole chunk.
    Chunk,
    /// The code, at one line of it.
    Line,
    /// The walkthrough explaining the chunk.
    Explanation,
}

const FEEDBACK_SCOPE_HINT: &str = "[Enter]\u{a0}the code   [a]\u{a0}one source line   [e]\u{a0}this explanation   [Esc]\u{a0}cancel";

impl ReviewTui {
    pub(super) fn comment(&mut self) -> Result<()> {
        match self.feedback_scope("Comment — what is it about?")? {
            Some(Scope::Line) => self.add_line_feedback(FeedbackKind::Note),
            Some(Scope::Chunk) => self.add_feedback(FeedbackKind::Note, FeedbackTarget::Code, None),
            Some(Scope::Explanation) => {
                self.add_feedback(FeedbackKind::Note, FeedbackTarget::Explanation, None)
            }
            None => Ok(()),
        }
    }

    pub(super) fn add_feedback(
        &mut self,
        kind: FeedbackKind,
        target: FeedbackTarget,
        anchor: Option<LineAnchor>,
    ) -> Result<()> {
        let title = match (kind, target) {
            (FeedbackKind::Note, FeedbackTarget::Code) => {
                "Comment — non-blocking, about the code in this chunk"
            }
            (FeedbackKind::Note, FeedbackTarget::Explanation) => {
                "Comment — non-blocking, about this explanation"
            }
            (FeedbackKind::Issue, FeedbackTarget::Code) => {
                "Request change to the code — unresolved until explicitly resolved"
            }
            (FeedbackKind::Issue, FeedbackTarget::Explanation) => {
                "Request change to this explanation — the code is not at fault"
            }
        };
        if target == FeedbackTarget::Code && anchor.is_none() {
            self.scroll_to_first_source();
        }
        let Some(body) = self.write(title)? else {
            return Ok(());
        };
        if body.is_empty() {
            return Ok(());
        }
        self.save_feedback(kind, target, anchor, body)
    }

    pub(super) fn save_feedback(
        &mut self,
        kind: FeedbackKind,
        target: FeedbackTarget,
        anchor: Option<LineAnchor>,
        body: String,
    ) -> Result<()> {
        let saved = match kind {
            FeedbackKind::Note => format!("Saved comment {}", target_label(target, None)),
            FeedbackKind::Issue => {
                format!("Saved request {}", target_label(target, anchor.as_ref()))
            }
        };
        let id = Self::unique_id("feedback")?;
        self.record(ReviewEvent::feedback(Feedback {
            id: id.clone(),
            chunk_id: self.current_chunk_id(),
            kind,
            target,
            body,
            anchor,
            resolved: false,
        }))?;
        self.notice = Some(saved);
        self.scroll_to_feedback(&id);
        Ok(())
    }

    pub(super) fn request_change(&mut self) -> Result<()> {
        match self.feedback_scope("Request change — what is wrong?")? {
            Some(Scope::Line) => self.add_line_feedback(FeedbackKind::Issue),
            Some(Scope::Chunk) => {
                self.add_feedback(FeedbackKind::Issue, FeedbackTarget::Code, None)
            }
            Some(Scope::Explanation) => {
                self.add_feedback(FeedbackKind::Issue, FeedbackTarget::Explanation, None)
            }
            None => Ok(()),
        }
    }

    fn add_line_feedback(&mut self, kind: FeedbackKind) -> Result<()> {
        let Some(anchor) = self.choose_line_anchor(kind)? else {
            return Ok(());
        };
        self.add_feedback(kind, FeedbackTarget::Code, Some(anchor))
    }

    /// Lets the reviewer move over exactly the new-side lines this chunk shows.
    /// A path and line are always selected together, so chunks spanning several
    /// files never turn a bare number into an ambiguous anchor.
    fn choose_line_anchor(&mut self, kind: FeedbackKind) -> Result<Option<LineAnchor>> {
        let anchors = self.line_anchors();
        if anchors.is_empty() {
            self.notice = Some("This chunk has no quoted new-side line to attach to".into());
            return Ok(None);
        }
        let mut selected = 0;
        loop {
            let anchor = &anchors[selected];
            self.selected_line = Some(anchor.clone());
            self.scroll_to_selected_line();
            let excerpt = self.anchor_excerpt(anchor);
            let title = format!(
                "Attach {} to {}:{} ({}/{}) — {}",
                match kind {
                    FeedbackKind::Note => "comment",
                    FeedbackKind::Issue => "request",
                },
                anchor.path,
                anchor.start_line,
                selected + 1,
                anchors.len(),
                excerpt,
            );
            let choice = self.ask(
                &title,
                "[Enter] choose   [↑/k ↓/j] line   [PgUp/PgDn] scroll chunk   [Esc] cancel",
            );
            let choice = match choice {
                Ok(choice) => choice,
                Err(error) => {
                    self.selected_line = None;
                    return Err(error);
                }
            };
            match choice {
                KeyCode::Esc => {
                    self.selected_line = None;
                    return Ok(None);
                }
                KeyCode::Enter => {
                    let chosen = anchor.clone();
                    self.selected_line = None;
                    return Ok(Some(chosen));
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(anchors.len() - 1);
                }
                code => self.scroll_by(code),
            }
        }
    }

    pub(super) fn anchor_excerpt(&self, anchor: &LineAnchor) -> String {
        self.review
            .input()
            .changes
            .iter()
            .find(|change| change.id == anchor.change_id)
            .and_then(|change| {
                change
                    .new_side(anchor.start_line, anchor.end_line)
                    .first()
                    .copied()
            })
            .map(|line| line.get(1..).unwrap_or(line).trim_end())
            .filter(|line| !line.trim().is_empty())
            .map_or_else(
                || "(blank line)".into(),
                |line| format!("`{}`", compact(line, 32)),
            )
    }

    pub(super) fn line_anchors(&self) -> Vec<LineAnchor> {
        let chunk = &self.review.spec().chunks[self.chunk_index().expect("chunk stage")];
        let mut anchors = Vec::new();
        let quotes = chunk.content.iter().flat_map(|block| match block {
            ChunkBlock::Changes {
                presentation: ChangePresentation::SummaryOnly,
                ..
            }
            | ChunkBlock::Narrative { .. } => &[][..],
            ChunkBlock::Changes { quotes, .. } => quotes.as_slice(),
        });
        for quote in quotes {
            let Some(change) = self
                .review
                .input()
                .changes
                .iter()
                .find(|change| change.id == quote.change_id)
            else {
                continue;
            };
            for body_line in quote.start..=quote.end {
                let Some(line) = change.new_line_at(body_line) else {
                    continue;
                };
                let anchor = LineAnchor {
                    change_id: change.id.clone(),
                    path: change.path.clone(),
                    side: "new".into(),
                    start_line: line,
                    end_line: line,
                    patch_sha256: change.patch_sha256(),
                };
                if !anchors.iter().any(|existing: &LineAnchor| {
                    existing.change_id == anchor.change_id && existing.start_line == line
                }) {
                    anchors.push(anchor);
                }
            }
        }
        anchors
    }

    /// What the reviewer is objecting to. Gewu shows two things at once — the
    /// code and its own account of the code — and an agent fixes them in
    /// different places, so it is worth one keystroke to say which.
    fn feedback_scope(&mut self, title: &str) -> Result<Option<Scope>> {
        loop {
            match self.ask(title, FEEDBACK_SCOPE_HINT)? {
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => return Ok(Some(Scope::Chunk)),
                KeyCode::Char('a') => return Ok(Some(Scope::Line)),
                KeyCode::Char('e') => return Ok(Some(Scope::Explanation)),
                code => self.scroll_by(code),
            }
        }
    }

    /// Re-aims feedback the reviewer filed at the wrong thing. Cheaper than
    /// deleting it and writing it again, and it keeps the record of what they
    /// said when they said it.
    pub(super) fn retarget_selected(&mut self) -> Result<()> {
        let Some(feedback) = self.selected_feedback() else {
            return Ok(());
        };
        let (id, target) = (
            feedback.id.clone(),
            match feedback.target {
                FeedbackTarget::Code => FeedbackTarget::Explanation,
                FeedbackTarget::Explanation => FeedbackTarget::Code,
            },
        );
        self.record(ReviewEvent::retargeted(id, target))?;
        self.notice = Some(format!(
            "Feedback retargeted {}",
            target_label(target, None)
        ));
        Ok(())
    }

    /// The feedback the reviewer has picked out, if the current screen is a
    /// chunk and the selection still names one of its items.
    pub(super) fn selected_feedback(&self) -> Option<&Feedback> {
        let index = self.selected?;
        self.chunk_feedback().into_iter().nth(index)
    }

    pub(super) fn chunk_feedback(&self) -> Vec<&Feedback> {
        let Some(index) = self.chunk_index() else {
            return Vec::new();
        };
        self.session
            .chunk_feedback(&self.review.spec().chunks[index].id)
    }

    pub(super) fn select_first_feedback(&mut self) {
        if !self.chunk_feedback().is_empty() {
            self.selection_origin_scroll = Some(self.scroll);
            self.selected = Some(0);
            self.scroll_to_selected_feedback();
        }
    }

    pub(super) fn move_selection(&mut self, step: isize) {
        let last = self.chunk_feedback().len().saturating_sub(1);
        if let Some(index) = self.selected {
            self.selected = Some(index.saturating_add_signed(step).min(last));
        }
    }

    pub(super) fn toggle_selected_resolved(&mut self) -> Result<()> {
        let Some(feedback) = self.selected_feedback() else {
            return Ok(());
        };
        let (id, resolved) = (feedback.id.clone(), feedback.resolved);
        self.record(ReviewEvent::resolved(id, !resolved))
    }

    /// Withdraws feedback the reviewer did not mean to leave. Deleting is the
    /// one action here that cannot be undone from the screen, so it is
    /// confirmed.
    pub(super) fn delete_selected(&mut self) -> Result<()> {
        let Some(feedback) = self.selected_feedback() else {
            return Ok(());
        };
        let id = feedback.id.clone();
        let title = delete_confirmation(feedback);
        if !self.confirm(&title)? {
            return Ok(());
        }
        self.record(ReviewEvent::deleted(id))?;
        let remaining = self.chunk_feedback().len();
        self.selected = match self.selected {
            Some(index) if remaining > 0 => Some(index.min(remaining - 1)),
            _ => None,
        };
        Ok(())
    }

    pub(super) fn first_unresolved_feedback(&self) -> Option<(usize, usize)> {
        let feedback = *self
            .session
            .live_unresolved_issues(self.review.spec().chunks.iter().map(|chunk| &chunk.id))
            .first()?;
        let chunk = self
            .review
            .spec()
            .chunks
            .iter()
            .position(|chunk| chunk.id == feedback.chunk_id)?;
        let index = self
            .session
            .chunk_feedback(&feedback.chunk_id)
            .iter()
            .position(|candidate| candidate.id == feedback.id)?;
        Some((chunk, index))
    }

    /// Applies the scrolling keys, leaving every other key to the caller.
    pub(super) fn scroll_by(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.page_step()),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(self.page_step()),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll_to_end(),
            _ => {}
        }
    }

    fn unique_id(prefix: &str) -> Result<String> {
        Ok(format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ))
    }
}

fn target_label(target: FeedbackTarget, anchor: Option<&LineAnchor>) -> String {
    match (target, anchor) {
        (FeedbackTarget::Explanation, _) => "about this explanation".into(),
        (FeedbackTarget::Code, Some(anchor)) => {
            format!("about {}:{}", anchor.path, anchor.start_line)
        }
        (FeedbackTarget::Code, None) => "about the code".into(),
    }
}

fn compact(value: &str, limit: usize) -> String {
    let mut shortened: String = value.chars().take(limit).collect();
    if value.chars().count() > limit {
        shortened.push('…');
    }
    shortened
}

fn delete_confirmation(feedback: &Feedback) -> String {
    const LIMIT: usize = 72;
    let first = feedback.body.lines().next().unwrap_or_default();
    let mut preview: String = first.chars().take(LIMIT).collect();
    if first.chars().count() > LIMIT || feedback.body.lines().count() > 1 {
        preview.push('…');
    }
    format!(
        "Delete \"{preview}\" {}? It leaves the review entirely.",
        target_label(feedback.target, feedback.anchor.as_ref())
    )
}

#[cfg(test)]
mod tests {
    use gewu_review::{Feedback, FeedbackKind, FeedbackTarget, LineAnchor};

    use super::{FEEDBACK_SCOPE_HINT, compact, delete_confirmation, target_label};

    #[test]
    fn comments_and_requests_share_the_source_line_choice() {
        assert!(FEEDBACK_SCOPE_HINT.contains("[a]\u{a0}one source line"));
    }

    #[test]
    fn deletion_names_the_feedback_body_and_target() {
        let feedback = Feedback {
            id: "feedback".into(),
            chunk_id: "chunk".into(),
            kind: FeedbackKind::Issue,
            target: FeedbackTarget::Explanation,
            body: "The explanation contradicts the quote.".into(),
            anchor: None,
            resolved: true,
        };

        let prompt = delete_confirmation(&feedback);

        assert!(prompt.contains("The explanation contradicts the quote."));
        assert!(prompt.contains("about this explanation"));
    }

    #[test]
    fn line_target_names_both_file_and_line() {
        let anchor = LineAnchor {
            change_id: "change".into(),
            path: "src/very/deep/file.rs".into(),
            side: "new".into(),
            start_line: 42,
            end_line: 42,
            patch_sha256: "hash".into(),
        };

        assert_eq!(
            target_label(FeedbackTarget::Code, Some(&anchor)),
            "about src/very/deep/file.rs:42"
        );
    }

    #[test]
    fn line_picker_excerpt_is_bounded_at_character_boundaries() {
        let excerpt = compact("résumé β source text that keeps going for a while", 12);

        assert_eq!(excerpt, "résumé β sou…");
    }
}
