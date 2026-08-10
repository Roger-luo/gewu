use anyhow::Result;
use crossterm::event::KeyCode;
use gewu_review::{Feedback, FeedbackKind, FeedbackTarget, LineAnchor, ReviewEvent};

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

impl ReviewTui {
    pub(super) fn add_feedback(
        &mut self,
        kind: FeedbackKind,
        target: FeedbackTarget,
        anchor: Option<LineAnchor>,
    ) -> Result<()> {
        let title = match (kind, target) {
            (FeedbackKind::Note, _) => "Comment — non-blocking, attached to this chunk",
            (FeedbackKind::Issue, FeedbackTarget::Code) => {
                "Request change to the code — unresolved until explicitly resolved"
            }
            (FeedbackKind::Issue, FeedbackTarget::Explanation) => {
                "Request change to this explanation — the code is not at fault"
            }
        };
        let Some(body) = self.write(title)? else {
            return Ok(());
        };
        if body.is_empty() {
            return Ok(());
        }
        self.record(ReviewEvent::feedback(Feedback {
            id: Self::unique_id("feedback")?,
            chunk_id: self.current_chunk_id(),
            kind,
            target,
            body,
            anchor,
            resolved: false,
        }))
    }

    pub(super) fn request_change(&mut self) -> Result<()> {
        match self.request_scope()? {
            Some(Scope::Line) => self.add_line_request(),
            Some(Scope::Chunk) => {
                self.add_feedback(FeedbackKind::Issue, FeedbackTarget::Code, None)
            }
            Some(Scope::Explanation) => {
                self.add_feedback(FeedbackKind::Issue, FeedbackTarget::Explanation, None)
            }
            None => Ok(()),
        }
    }

    fn add_line_request(&mut self) -> Result<()> {
        let Some(line) = self.write("Attach to a line — its number on the new side")? else {
            return Ok(());
        };
        let Ok(line) = line.parse() else {
            return Ok(());
        };
        let chunk = &self.review.spec().chunks[self.chunk_index().expect("chunk stage")];
        let Some(change) = chunk.change_ids().next().and_then(|id| {
            self.review
                .input()
                .changes
                .iter()
                .find(|change| &change.id == id)
        }) else {
            return Ok(());
        };
        let end = change
            .new_start
            .saturating_add(change.new_lines.saturating_sub(1));
        if line < change.new_start || line > end {
            return Ok(());
        }
        let anchor = LineAnchor {
            change_id: change.id.clone(),
            path: change.path.clone(),
            side: "new".into(),
            start_line: line,
            end_line: line,
            patch_sha256: change.patch_sha256(),
        };
        self.add_feedback(FeedbackKind::Issue, FeedbackTarget::Code, Some(anchor))
    }

    /// What the reviewer is objecting to. Gewu shows two things at once — the
    /// code and its own account of the code — and an agent fixes them in
    /// different places, so it is worth one keystroke to say which.
    fn request_scope(&mut self) -> Result<Option<Scope>> {
        loop {
            match self.ask(
                "Request change — what is wrong?",
                "[Enter] the code   [a] the code, at one line   [e] this explanation   [Esc] \
                 cancel",
            )? {
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
        self.record(ReviewEvent::retargeted(id, target))
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
            self.selected = Some(0);
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
        if !self.confirm("Delete this feedback? It leaves the review entirely.")? {
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

    pub(super) fn first_unresolved_chunk(&self) -> Option<usize> {
        let feedback = *self
            .session
            .live_unresolved_issues(self.review.spec().chunks.iter().map(|chunk| &chunk.id))
            .first()?;
        self.review
            .spec()
            .chunks
            .iter()
            .position(|chunk| chunk.id == feedback.chunk_id)
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
