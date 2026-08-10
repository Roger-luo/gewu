use std::collections::HashSet;

use crate::{
    Decision, FeedbackTarget, ReviewChunk, ReviewInput, ReviewSession, ReviewSpec,
    handover::chunks_to_revise,
};

use super::WorkList;

/// What the harness should do when asked to continue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Expand the next work item.
    Expand,
    /// The reviewer has not finished the batch they were given.
    Wait { unreviewed: usize },
    /// Edits reached work that has not been expanded, so the remaining plan is
    /// no longer trustworthy.
    Replan { stale_chunks: Vec<String> },
    /// Unresolved requests about the code. An agent edits the source, then
    /// resolves them.
    Handover { feedback_ids: Vec<String> },
    /// Unresolved requests about the walkthrough. An agent rewrites exactly
    /// these chunks.
    Revise { chunk_ids: Vec<String> },
    /// Nothing is left to expand or the reviewer has finished.
    Complete,
}

/// Decides what happens when the harness is asked to continue.
///
/// The gates are ordered by what would waste the most work if skipped. Drift
/// comes first, because an approval over code that has since moved is not an
/// approval. Then the reviewer's own requests, because they are instructions
/// already given and nothing is gained by sitting on them. Only then does the
/// harness wait for the rest of the batch to be read. `force` skips the request
/// gates, for a reviewer who knows their request is confined to what is already
/// expanded.
#[must_use]
pub fn next_step(
    spec: &ReviewSpec,
    session: &ReviewSession,
    work: &WorkList,
    current: &ReviewInput,
    previous: &ReviewInput,
    force: bool,
) -> Verdict {
    // Approving or commenting ends the review. Requesting changes does not: it
    // is the reviewer handing work back, and the requests below say what it is.
    if matches!(
        session.decision,
        Some(Decision::Approve | Decision::Comment)
    ) {
        return Verdict::Complete;
    }

    let carried = current.unchanged_since(previous);
    let stale_chunks: Vec<_> = spec
        .chunks
        .iter()
        .filter(|chunk| chunk.change_ids().any(|id| !carried.contains(id)))
        .map(|chunk| chunk.id.clone())
        .collect();
    let expanded: HashSet<_> = spec
        .chunks
        .iter()
        .flat_map(ReviewChunk::change_ids)
        .collect();
    let drifted_beyond_expansion = current
        .changes
        .iter()
        .any(|change| !carried.contains(&change.id) && !expanded.contains(&change.id));

    // Requests come before re-planning, but only where the code they are about
    // has held still. Re-planning keeps chunks that survived the drift, so
    // making the reviewer's point wait behind a fresh investigation of code it
    // has nothing to do with would be a long detour to the same place. A
    // request against a chunk that is about to be dropped is a different
    // matter: answering it is work nobody will ever see.
    let settled = || {
        spec.chunks
            .iter()
            .filter(|chunk| !stale_chunks.contains(&chunk.id))
    };
    if !force {
        // Code first: fixing the source moves the diff and can invalidate a
        // chunk, so rewriting an explanation before that would be thrown away.
        let code: Vec<_> = session
            .live_unresolved_issues(settled().map(|chunk| &chunk.id))
            .into_iter()
            .filter(|feedback| feedback.target == FeedbackTarget::Code)
            .map(|feedback| feedback.id.clone())
            .collect();
        if !code.is_empty() {
            return Verdict::Handover { feedback_ids: code };
        }
        let chunk_ids = chunks_to_revise(settled(), session);
        if !chunk_ids.is_empty() {
            return Verdict::Revise { chunk_ids };
        }
    }

    if !stale_chunks.is_empty() || drifted_beyond_expansion {
        return Verdict::Replan { stale_chunks };
    }

    let unreviewed = spec
        .chunks
        .iter()
        .filter(|chunk| !session.reviewed.contains(&chunk.id))
        .count();
    if unreviewed > 0 {
        return Verdict::Wait { unreviewed };
    }
    if work.next_item().is_none() {
        return Verdict::Complete;
    }
    Verdict::Expand
}

#[cfg(test)]
mod tests {
    use crate::{
        Change, ChangePresentation, ChunkBlock, ChunkKind, Decision, Feedback, FeedbackKind,
        FeedbackTarget, Overview, Quote, ReviewChunk, ReviewEvent, ReviewInput, ReviewSession,
        ReviewSpec, SourceIdentity,
        harness::{HARNESS_SCHEMA_VERSION, WorkItem, WorkList},
    };

    use super::{Verdict, next_step};

    fn change(path: &str, body: &str) -> Change {
        Change::new(path, (0, 0, 1, 1), format!("@@ -0,0 +1,1 @@\n+{body}\n"))
    }

    fn fixture(
        changes: Vec<Change>,
        expanded_ids: &[String],
    ) -> (ReviewSpec, WorkList, ReviewInput) {
        let source = SourceIdentity::new("base", "head", b"diff");
        let chunks = expanded_ids
            .iter()
            .enumerate()
            .map(|(index, id)| ReviewChunk {
                id: format!("chunk-{index}"),
                title: "Chunk".into(),
                summary: "Summary.".into(),
                kind: ChunkKind::Other,
                content: vec![ChunkBlock::Changes {
                    quotes: vec![Quote::whole(id.clone(), 1)],
                    presentation: ChangePresentation::Diff,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
            })
            .collect();
        let items = changes
            .iter()
            .enumerate()
            .map(|(index, change)| WorkItem {
                id: format!("item-{index}"),
                title: "Item".into(),
                purpose: "Purpose.".into(),
                covers: vec![Quote::whole(change.id.clone(), change.body_lines())],
                expanded: expanded_ids.contains(&change.id),
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
        let work = WorkList {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: source.clone(),
            items,
        };
        (spec, work, ReviewInput::new(source, changes))
    }

    fn reviewed(spec: &ReviewSpec) -> ReviewSession {
        let mut session = ReviewSession::default();
        for chunk in &spec.chunks {
            session.apply(&ReviewEvent::reviewed(chunk.id.clone(), true));
        }
        session
    }

    #[test]
    fn waits_while_chunks_are_unreviewed() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );

        let verdict = next_step(
            &spec,
            &ReviewSession::default(),
            &work,
            &input,
            &input,
            false,
        );

        assert_eq!(verdict, Verdict::Wait { unreviewed: 1 });
    }

    fn request(id: &str, chunk_id: &str, target: FeedbackTarget) -> Feedback {
        Feedback {
            id: id.into(),
            chunk_id: chunk_id.into(),
            kind: FeedbackKind::Issue,
            target,
            body: "Please rework this.".into(),
            anchor: None,
            resolved: false,
        }
    }

    #[test]
    fn hands_a_request_about_the_code_back_unless_forced() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-0",
            FeedbackTarget::Code,
        )));

        assert_eq!(
            next_step(&spec, &session, &work, &input, &input, false),
            Verdict::Handover {
                feedback_ids: vec!["feedback-1".into()]
            }
        );
        assert_eq!(
            next_step(&spec, &session, &work, &input, &input, true),
            Verdict::Expand
        );
    }

    #[test]
    fn asks_for_a_rewrite_of_the_chunk_a_request_about_the_walkthrough_names() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-0",
            FeedbackTarget::Explanation,
        )));

        assert_eq!(
            next_step(&spec, &session, &work, &input, &input, false),
            Verdict::Revise {
                chunk_ids: vec!["chunk-0".into()]
            }
        );
    }

    /// Fixing the code moves the diff and can drop a chunk, so a rewrite of
    /// that chunk's prose would be thrown away before anyone read it.
    #[test]
    fn fixes_the_code_before_rewriting_any_explanation() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::feedback(request(
            "wording",
            "chunk-0",
            FeedbackTarget::Explanation,
        )));
        session.apply(&ReviewEvent::feedback(request(
            "bug",
            "chunk-0",
            FeedbackTarget::Code,
        )));

        assert_eq!(
            next_step(&spec, &session, &work, &input, &input, false),
            Verdict::Handover {
                feedback_ids: vec!["bug".into()]
            }
        );
    }

    /// A request is an instruction the reviewer has already given; making the
    /// agent sit on it until they finish reading wastes the round trip.
    #[test]
    fn a_request_does_not_wait_for_the_rest_of_the_batch() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-0",
            FeedbackTarget::Code,
        )));

        assert_eq!(
            next_step(&spec, &session, &work, &input, &input, false),
            Verdict::Handover {
                feedback_ids: vec!["feedback-1".into()]
            }
        );
    }

    /// Requesting changes is the reviewer handing work back, not ending the
    /// review; the requests themselves say what the work is.
    #[test]
    fn requesting_changes_hands_the_requests_over() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-0",
            FeedbackTarget::Explanation,
        )));
        session.apply(&ReviewEvent::decision(Decision::RequestChanges));

        assert_eq!(
            next_step(&spec, &session, &work, &input, &input, false),
            Verdict::Revise {
                chunk_ids: vec!["chunk-0".into()]
            }
        );
    }

    #[test]
    fn expands_when_the_batch_is_done_and_nothing_moved() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );

        let verdict = next_step(&spec, &reviewed(&spec), &work, &input, &input, false);

        assert_eq!(verdict, Verdict::Expand);
    }

    #[test]
    fn completes_once_every_item_is_expanded() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(vec![first.clone()], std::slice::from_ref(&first.id));

        let verdict = next_step(&spec, &reviewed(&spec), &work, &input, &input, false);

        assert_eq!(verdict, Verdict::Complete);
    }

    #[test]
    fn a_recorded_decision_ends_the_review() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::decision(Decision::Approve));

        let verdict = next_step(&spec, &session, &work, &input, &input, false);

        assert_eq!(verdict, Verdict::Complete);
    }

    #[test]
    fn a_request_about_a_dropped_chunk_no_longer_blocks() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        // A request against a chunk that re-planning has since removed.
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-removed",
            FeedbackTarget::Code,
        )));

        let verdict = next_step(&spec, &session, &work, &input, &input, false);

        // Nothing on screen can resolve it, so it must not hold the gate shut.
        assert_eq!(verdict, Verdict::Expand);
    }

    /// Found by using Gewu on itself: every request sat behind a fresh
    /// investigation of fifty-odd changes that had nothing to do with what the
    /// reviewer was pointing at.
    #[test]
    fn a_request_about_settled_code_is_answered_before_re_planning() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        // The reviewer edited b.rs, which no chunk has quoted yet, and left a
        // request on chunk-0, which quotes a.rs and has not moved.
        let (_, _, drifted) = fixture(vec![first, change("b.rs", "rewritten")], &[]);
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-0",
            FeedbackTarget::Explanation,
        )));

        assert_eq!(
            next_step(&spec, &session, &work, &drifted, &input, false),
            Verdict::Revise {
                chunk_ids: vec!["chunk-0".into()]
            }
        );
    }

    /// Answering a request about a chunk that is about to be dropped is work
    /// nobody will ever see.
    #[test]
    fn a_request_about_a_chunk_the_drift_invalidated_re_plans_instead() {
        let first = change("a.rs", "one");
        let second = change("b.rs", "two");
        let (spec, work, input) = fixture(
            vec![first.clone(), second.clone()],
            std::slice::from_ref(&first.id),
        );
        let (_, _, drifted) = fixture(vec![change("a.rs", "rewritten"), second], &[]);
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::feedback(request(
            "feedback-1",
            "chunk-0",
            FeedbackTarget::Explanation,
        )));

        assert_eq!(
            next_step(&spec, &session, &work, &drifted, &input, false),
            Verdict::Replan {
                stale_chunks: vec!["chunk-0".into()]
            }
        );
    }

    #[test]
    fn replans_when_an_edit_lands_outside_the_expansion() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        // The reviewer edited b.rs, which no chunk has quoted yet.
        let (_, _, drifted) = fixture(vec![first, change("b.rs", "rewritten")], &[]);

        let verdict = next_step(&spec, &reviewed(&spec), &work, &drifted, &input, false);

        assert_eq!(
            verdict,
            Verdict::Replan {
                stale_chunks: vec![]
            }
        );
    }

    #[test]
    fn an_edit_under_a_reviewed_chunk_marks_it_stale() {
        let first = change("a.rs", "one");
        let second = change("b.rs", "two");
        let (spec, work, input) = fixture(
            vec![first.clone(), second.clone()],
            std::slice::from_ref(&first.id),
        );
        let (_, _, drifted) = fixture(vec![change("a.rs", "rewritten"), second], &[]);

        let verdict = next_step(&spec, &reviewed(&spec), &work, &drifted, &input, false);

        assert_eq!(
            verdict,
            Verdict::Replan {
                stale_chunks: vec!["chunk-0".into()]
            }
        );
    }
}
