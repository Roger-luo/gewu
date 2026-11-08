use std::{
    fs,
    path::{Path, PathBuf},
};

use gewu_review::{
    EventLog, Feedback, LineAnchor, ReviewEvent, ReviewInput, ReviewSession, ReviewSpec,
    ValidatedReview,
};

use crate::{Action, AnchorRef, AppError, Snapshot};

/// A review workspace root and the reviewer's live session against it.
pub struct ReviewWorkspace {
    root: PathBuf,
    review: ValidatedReview,
    log: EventLog,
    session: ReviewSession,
}

impl ReviewWorkspace {
    /// Load `<root>/.gewu/review-input.json` and `review-spec.json`, then replay
    /// events from `.gewu/reviews/<session_key>/events.jsonl`.
    pub fn open(root: &Path) -> Result<Self, AppError> {
        let input_path = root.join(".gewu/review-input.json");
        let spec_path = root.join(".gewu/review-spec.json");
        let input: ReviewInput = read_json(&input_path)?;
        let spec: ReviewSpec = read_json(&spec_path)?;
        let review =
            ValidatedReview::validate_partial(input, spec).map_err(AppError::validation)?;
        let session_path = root.join(format!(
            ".gewu/reviews/{}/events.jsonl",
            review.spec().session_key()
        ));
        let log = EventLog::new(session_path);
        let mut session = log.load()?;
        let expected = review.spec().session_key();
        if let Some(existing) = &session.spec_id {
            if existing != &expected {
                return Err(AppError::SessionMismatch {
                    existing: existing.clone(),
                    expected,
                });
            }
        } else {
            let event = ReviewEvent::started(expected);
            log.append(&event)?;
            session.apply(&event);
        }
        Ok(Self {
            root: root.to_path_buf(),
            review,
            log,
            session,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot::from_review(&self.review, &self.session)
    }

    /// Apply an action, appending to the event log. Rejected actions leave
    /// state unchanged.
    pub fn dispatch(&mut self, action: Action) -> Result<Snapshot, AppError> {
        let event = self.event_for(action)?;
        self.log.append(&event)?;
        self.session.apply(&event);
        Ok(self.snapshot())
    }

    fn event_for(&self, action: Action) -> Result<ReviewEvent, AppError> {
        match action {
            Action::Visit { chunk_id } => {
                self.require_chunk(&chunk_id)?;
                Ok(ReviewEvent::visited(chunk_id))
            }
            Action::SetReviewed { chunk_id, reviewed } => {
                self.require_chunk(&chunk_id)?;
                Ok(ReviewEvent::reviewed(chunk_id, reviewed))
            }
            Action::AddFeedback {
                chunk_id,
                kind,
                target,
                body,
                anchor,
            } => {
                self.require_chunk(&chunk_id)?;
                if body.trim().is_empty() {
                    return Err(AppError::EmptyBody);
                }
                let anchor = match anchor {
                    Some(reference) => Some(self.resolve_anchor(&chunk_id, &reference)?),
                    None => None,
                };
                Ok(ReviewEvent::feedback(Feedback {
                    id: unique_feedback_id(),
                    chunk_id,
                    kind,
                    target,
                    body,
                    anchor,
                    resolved: false,
                }))
            }
            Action::ResolveFeedback {
                feedback_id,
                resolved,
            } => {
                self.require_feedback(&feedback_id)?;
                Ok(ReviewEvent::resolved(feedback_id, resolved))
            }
            Action::DeleteFeedback { feedback_id } => {
                self.require_feedback(&feedback_id)?;
                Ok(ReviewEvent::deleted(feedback_id))
            }
            Action::RetargetFeedback {
                feedback_id,
                target,
            } => {
                self.require_feedback(&feedback_id)?;
                Ok(ReviewEvent::retargeted(feedback_id, target))
            }
            Action::SetDecision { decision } => Ok(ReviewEvent::decision(decision)),
        }
    }

    fn require_chunk(&self, chunk_id: &str) -> Result<(), AppError> {
        self.review
            .spec()
            .chunks
            .iter()
            .any(|chunk| chunk.id == chunk_id)
            .then_some(())
            .ok_or_else(|| AppError::UnknownChunk(chunk_id.into()))
    }

    fn require_feedback(&self, feedback_id: &str) -> Result<(), AppError> {
        self.session
            .feedback
            .contains_key(feedback_id)
            .then_some(())
            .ok_or_else(|| AppError::UnknownFeedback(feedback_id.into()))
    }

    /// Fill path, side, and patch hash, and check the new-side lines appear in
    /// this chunk's quotes for the named change — matching the terminal UI's
    /// line-picker anchors.
    fn resolve_anchor(
        &self,
        chunk_id: &str,
        reference: &AnchorRef,
    ) -> Result<LineAnchor, AppError> {
        if reference.start_line == 0
            || reference.end_line == 0
            || reference.end_line < reference.start_line
        {
            return Err(AppError::AnchorOutsideQuote);
        }
        let change = self
            .review
            .input()
            .changes
            .iter()
            .find(|change| change.id == reference.change_id)
            .ok_or_else(|| AppError::UnknownChange(reference.change_id.clone()))?;
        let chunk = self
            .review
            .spec()
            .chunks
            .iter()
            .find(|chunk| chunk.id == chunk_id)
            .ok_or_else(|| AppError::UnknownChunk(chunk_id.into()))?;
        let quoted_new: std::collections::HashSet<u32> = chunk
            .quotes()
            .filter(|quote| quote.change_id == change.id)
            .flat_map(|quote| {
                (quote.start..=quote.end).filter_map(|body_line| change.new_line_at(body_line))
            })
            .collect();
        for line in reference.start_line..=reference.end_line {
            if !quoted_new.contains(&line) {
                return Err(AppError::AnchorOutsideQuote);
            }
        }
        Ok(LineAnchor {
            change_id: change.id.clone(),
            path: change.path.clone(),
            side: "new".into(),
            start_line: reference.start_line,
            end_line: reference.end_line,
            patch_sha256: change.patch_sha256(),
        })
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, AppError> {
    if !path.exists() {
        return Err(AppError::MissingArtifact(path.to_path_buf()));
    }
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn unique_feedback_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("feedback-{nanos}")
}
