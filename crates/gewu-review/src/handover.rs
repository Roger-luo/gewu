//! What a reviewer produced, assembled for whoever acts on it next.
//!
//! The event log is the record; it is not the message. Replaying it leaves
//! withdrawn feedback, bare chunk ids, and no sight of the code a request is
//! about. A handover resolves all three so an agent reads the review rather
//! than reconstructing it.

use serde::Serialize;

use crate::{
    Change, Decision, Feedback, FeedbackKind, FeedbackTarget, ReviewChunk, ReviewSession,
    ValidatedReview,
};

/// Where a request points, once its anchor has been resolved against the
/// manifest.
#[derive(Debug, Serialize)]
pub struct Location<'a> {
    pub path: &'a str,
    pub side: &'a str,
    pub start_line: u32,
    pub end_line: u32,
    /// The hunk no longer matches what the reviewer commented on, so the lines
    /// quoted below are not the ones they saw.
    pub stale: bool,
    pub lines: Vec<&'a str>,
}

/// One piece of feedback with everything needed to act on it.
#[derive(Debug, Serialize)]
pub struct HandoverItem<'a> {
    pub id: &'a str,
    pub target: FeedbackTarget,
    pub chunk_id: &'a str,
    /// Absent when re-planning dropped the chunk out from under the feedback.
    pub chunk_title: Option<&'a str>,
    pub chunk_summary: Option<&'a str>,
    pub body: &'a str,
    pub location: Option<Location<'a>>,
}

#[derive(Debug, Serialize)]
pub struct Handover<'a> {
    pub review_id: String,
    pub decision: Option<Decision>,
    /// Unresolved requests, oldest first. These block the harness.
    pub requests: Vec<HandoverItem<'a>>,
    /// Unresolved comments, oldest first. These do not block anything, but
    /// they are still what the reviewer thought.
    pub comments: Vec<HandoverItem<'a>>,
    pub chunks_reviewed: usize,
    pub chunks_total: usize,
    pub resolved: usize,
}

impl<'a> Handover<'a> {
    #[must_use]
    pub fn assemble(review: &'a ValidatedReview, session: &'a ReviewSession) -> Self {
        let spec = review.spec();
        let mut requests = Vec::new();
        let mut comments = Vec::new();
        let mut resolved = 0;
        for feedback in session.ordered() {
            if feedback.resolved {
                resolved += 1;
                continue;
            }
            let item = HandoverItem::assemble(feedback, review);
            match feedback.kind {
                FeedbackKind::Issue => requests.push(item),
                FeedbackKind::Note => comments.push(item),
            }
        }
        Self {
            review_id: spec.session_key(),
            decision: session.decision,
            requests,
            comments,
            chunks_reviewed: spec
                .chunks
                .iter()
                .filter(|chunk| session.reviewed.contains(&chunk.id))
                .count(),
            chunks_total: spec.chunks.len(),
            resolved,
        }
    }

    /// Whether there is anything for an agent to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty() && self.comments.is_empty()
    }

    /// Unresolved requests aimed at one target, in the order they were made.
    pub fn requests_for(&self, target: FeedbackTarget) -> impl Iterator<Item = &HandoverItem<'a>> {
        self.requests
            .iter()
            .filter(move |item| item.target == target)
    }
}

impl<'a> HandoverItem<'a> {
    fn assemble(feedback: &'a Feedback, review: &'a ValidatedReview) -> Self {
        let chunk = review
            .spec()
            .chunks
            .iter()
            .find(|chunk| chunk.id == feedback.chunk_id);
        Self {
            id: &feedback.id,
            target: feedback.target,
            chunk_id: &feedback.chunk_id,
            chunk_title: chunk.map(|chunk| chunk.title.as_str()),
            chunk_summary: chunk.map(|chunk| chunk.summary.as_str()),
            body: &feedback.body,
            location: Self::locate(feedback, review),
        }
    }

    fn locate(feedback: &'a Feedback, review: &'a ValidatedReview) -> Option<Location<'a>> {
        let anchor = feedback.anchor.as_ref()?;
        let change = review
            .input()
            .changes
            .iter()
            .find(|change: &&Change| change.id == anchor.change_id);
        Some(Location {
            path: &anchor.path,
            side: &anchor.side,
            start_line: anchor.start_line,
            end_line: anchor.end_line,
            stale: change.is_none_or(|change| change.patch_sha256() != anchor.patch_sha256),
            lines: change
                .map(|change| change.new_side(anchor.start_line, anchor.end_line))
                .unwrap_or_default(),
        })
    }
}

/// The chunks unresolved explanation requests name, in the order the review
/// presents them. Rewriting follows the reviewer's reading order, not the order
/// the complaints happened to arrive.
#[must_use]
pub fn chunks_to_revise<'a>(
    chunks: impl IntoIterator<Item = &'a ReviewChunk>,
    session: &ReviewSession,
) -> Vec<String> {
    let chunks: Vec<_> = chunks.into_iter().collect();
    let wanted: Vec<_> = session
        .live_unresolved_issues(chunks.iter().map(|chunk| &chunk.id))
        .into_iter()
        .filter(|feedback| feedback.target == FeedbackTarget::Explanation)
        .map(|feedback| &feedback.chunk_id)
        .collect();
    chunks
        .into_iter()
        .filter(|chunk| wanted.contains(&&chunk.id))
        .map(|chunk| chunk.id.clone())
        .collect()
}
