use std::collections::HashSet;

use crate::{
    Decision, FeedbackTarget, ReviewChunk, ReviewInput, ReviewSession, ReviewSpec,
    handover::chunks_to_revise,
};

use super::{WorkList, WriteAhead};

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
    /// Nothing is left to expand and every chunk is read, or the reviewer
    /// settled a whole, read review with a decision.
    Complete,
}

/// Decides what happens when the harness is asked to continue.
///
/// The gates are ordered by what would waste the most work if skipped. A
/// review that is whole, read, and settled by a decision is complete. Then the
/// reviewer's own requests about code that has held still, because they are
/// instructions already given and nothing is gained by sitting on them. Then
/// drift, because chunks over code that has since moved must be re-planned.
/// Only then does the harness wait for the rest of the batch to be read.
/// `force` skips the request gates, for a reviewer who knows their request is
/// confined to what is already expanded. `ahead` is how many batches the
/// agent may write beyond the one the reviewer is reading.
#[must_use]
pub fn next_step(
    spec: &ReviewSpec,
    session: &ReviewSession,
    work: &WorkList,
    current: &ReviewInput,
    previous: &ReviewInput,
    force: bool,
    ahead: WriteAhead,
) -> Verdict {
    // Approving or commenting settles what the reviewer has read, and settles
    // the review itself only once there is nothing left to read. On a review
    // still being written, "I am done here" means the batch: it is the same
    // gesture either way, and taking it for the whole review would end one that
    // has covered a fraction of the change set. Nothing has to say stop —
    // the harness only ever runs because somebody ran it.
    let written: HashSet<_> = spec
        .chunks
        .iter()
        .flat_map(ReviewChunk::change_ids)
        .collect();
    let whole = current
        .changes
        .iter()
        .all(|change| written.contains(&change.id));
    // Nor does a decision outlive the batch it was made about: it is never
    // cleared, so without the read check an approval of the first batch would
    // complete the review as soon as the last one was written.
    let read = spec
        .chunks
        .iter()
        .all(|chunk| session.reviewed.contains(&chunk.id));
    if whole
        && read
        && matches!(
            session.decision,
            Some(Decision::Approve | Decision::Comment)
        )
    {
        return Verdict::Complete;
    }

    let carried = current.unchanged_since(previous);
    let stale_chunks: Vec<_> = spec
        .chunks
        .iter()
        .filter(|chunk| chunk.change_ids().any(|id| !carried.contains(id)))
        .map(|chunk| chunk.id.clone())
        .collect();
    let drifted_beyond_expansion = current
        .changes
        .iter()
        .any(|change| !carried.contains(&change.id) && !written.contains(&change.id))
        || previous.changes.iter().any(|change| {
            !written.contains(&change.id) && !current.changes.iter().any(|now| now.id == change.id)
        });

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

    // A decision settles what the reviewer had read when they made it, and
    // says nothing about a batch written since. It must not stand in for
    // reading one: this is the gate the whole design rests on, and letting a
    // decision retire it would let an agent write the entire plan unread.
    //
    // The gate lets the agent run `ahead` of the reviewer rather than level
    // with them: while they read, the next batches are written, so finishing
    // one opens another without waiting minutes for an agent. A chunk unread
    // from before the latest `ahead` batches still stops it, and the last
    // batch must be read to complete.
    let unread: Vec<&ReviewChunk> = spec
        .chunks
        .iter()
        .filter(|chunk| !session.reviewed.contains(&chunk.id))
        .collect();
    let free = ahead.allows(work, unread.iter().map(|chunk| chunk.id.as_str()));
    if !free || (!unread.is_empty() && work.next_item().is_none()) {
        return Verdict::Wait {
            unreviewed: unread.len(),
        };
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

    use super::{Verdict, WriteAhead, next_step};

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
                section: None,
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
                chunks: expanded_ids
                    .iter()
                    .position(|id| id == &change.id)
                    .map(|index| vec![format!("chunk-{index}")])
                    .unwrap_or_default(),
                tests: false,
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

    /// Asked for by the reviewer after a Cursor run of #276 left them
    /// waiting minutes after every section: the agent writes the next batch
    /// while the reviewer reads the latest, and no further.
    #[test]
    fn writes_one_batch_ahead_of_the_reviewer_and_no_further() {
        let (a, b, c) = (
            change("a.rs", "one"),
            change("b.rs", "two"),
            change("c.rs", "three"),
        );
        let changes = vec![a.clone(), b.clone(), c.clone()];

        // The first batch is unread, and it is the latest: write the next.
        let (spec, work, input) = fixture(changes.clone(), std::slice::from_ref(&a.id));
        let verdict = next_step(
            &spec,
            &ReviewSession::default(),
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );
        assert_eq!(verdict, Verdict::Expand);

        // Two batches unread: the older one holds the gate.
        let (spec, work, input) = fixture(changes.clone(), &[a.id.clone(), b.id.clone()]);
        let verdict = next_step(
            &spec,
            &ReviewSession::default(),
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );
        assert_eq!(verdict, Verdict::Wait { unreviewed: 2 });

        // Reading the older one opens it again.
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::reviewed("chunk-0", true));
        assert_eq!(
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
            Verdict::Expand
        );
    }

    /// The reviewer chooses how far ahead: with room for two batches, two
    /// unread batches still let the agent write, and a third holds it.
    #[test]
    fn writes_as_many_batches_ahead_as_the_reviewer_allows() {
        let changes: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|name| change(&format!("{name}.rs"), name))
            .collect();
        let ids: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        let step = |expanded: &[String], ahead| {
            let (spec, work, input) = fixture(changes.clone(), expanded);
            next_step(
                &spec,
                &ReviewSession::default(),
                &work,
                &input,
                &input,
                false,
                ahead,
            )
        };
        assert_eq!(step(&ids[..2], WriteAhead::Batches(2)), Verdict::Expand);
        assert_eq!(
            step(&ids[..3], WriteAhead::Batches(2)),
            Verdict::Wait { unreviewed: 3 }
        );
        assert_eq!(
            step(&ids[..1], WriteAhead::Batches(0)),
            Verdict::Wait { unreviewed: 1 }
        );
    }

    /// With no limit the agent writes the whole plan unread, and the review
    /// still waits for every chunk to be read before it completes.
    #[test]
    fn writes_the_whole_plan_unread_without_a_limit_but_completes_only_when_read() {
        let changes: Vec<_> = ["a", "b", "c"]
            .iter()
            .map(|name| change(&format!("{name}.rs"), name))
            .collect();
        let ids: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        let (spec, work, input) = fixture(changes.clone(), &ids[..2]);
        assert_eq!(
            next_step(
                &spec,
                &ReviewSession::default(),
                &work,
                &input,
                &input,
                false,
                WriteAhead::Unlimited
            ),
            Verdict::Expand
        );
        let (spec, work, input) = fixture(changes, &ids);
        assert_eq!(
            next_step(
                &spec,
                &ReviewSession::default(),
                &work,
                &input,
                &input,
                false,
                WriteAhead::Unlimited
            ),
            Verdict::Wait { unreviewed: 3 }
        );
        assert_eq!(
            next_step(
                &spec,
                &reviewed(&spec),
                &work,
                &input,
                &input,
                false,
                WriteAhead::Unlimited
            ),
            Verdict::Complete
        );
    }

    /// Plans written before items recorded their chunks have no latest batch,
    /// so the gate stays level with the reviewer rather than open for good.
    #[test]
    fn waits_while_chunks_are_unreviewed_when_no_batch_is_recorded() {
        let first = change("a.rs", "one");
        let (spec, mut work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        for item in &mut work.items {
            item.chunks.clear();
        }

        let verdict = next_step(
            &spec,
            &ReviewSession::default(),
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
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
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
            Verdict::Handover {
                feedback_ids: vec!["feedback-1".into()]
            }
        );
        assert_eq!(
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                true,
                WriteAhead::Batches(1)
            ),
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
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
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
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
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
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
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
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
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

        let verdict = next_step(
            &spec,
            &reviewed(&spec),
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );

        assert_eq!(verdict, Verdict::Expand);
    }

    #[test]
    fn completes_once_every_item_is_expanded() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(vec![first.clone()], std::slice::from_ref(&first.id));

        let verdict = next_step(
            &spec,
            &reviewed(&spec),
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );

        assert_eq!(verdict, Verdict::Complete);
    }

    #[test]
    fn a_recorded_decision_ends_a_review_that_covers_everything() {
        let only = change("a.rs", "one");
        let (spec, work, input) = fixture(vec![only.clone()], std::slice::from_ref(&only.id));
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::decision(Decision::Approve));

        let verdict = next_step(
            &spec,
            &session,
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );

        assert_eq!(verdict, Verdict::Complete);
    }

    /// A decision settles the batch it was made about, not the one written
    /// afterwards. Without this the first approval would retire the waiting
    /// gate for good, and an agent could write all ninety-four items unread.
    #[test]
    fn a_settled_review_still_waits_for_a_batch_written_since() {
        let (first, second) = (change("a.rs", "one"), change("b.rs", "two"));
        let (spec, work, input) = fixture(
            vec![first.clone(), second.clone(), change("c.rs", "three")],
            &[first.id.clone(), second.id.clone()],
        );
        // Settled, and then two batches arrived that the reviewer has not read.
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::decision(Decision::Approve));

        assert_eq!(
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
            Verdict::Wait { unreviewed: 2 }
        );
    }

    /// Found by self-review: a decision is never cleared, so approving the
    /// first batch used to complete the review the moment the last batch was
    /// written, before anyone read it.
    #[test]
    fn an_earlier_decision_does_not_complete_an_unread_final_batch() {
        let (first, second) = (change("a.rs", "one"), change("b.rs", "two"));
        let (spec, work, input) = fixture(
            vec![first.clone(), second.clone()],
            &[first.id.clone(), second.id.clone()],
        );
        // Batch one was read and approved; batch two covers the rest, unread.
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::reviewed("chunk-0", true));
        session.apply(&ReviewEvent::decision(Decision::Approve));

        assert_eq!(
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
            Verdict::Wait { unreviewed: 1 }
        );
    }

    /// Found by using Gewu on itself: a reviewer pressed on from the summary of
    /// the first batch of ninety-four and approved, which read as a finished
    /// review of one change in sixty-two. "I am done here" is the same gesture
    /// for a batch and for the whole thing; only the second can end it.
    #[test]
    fn approving_a_review_still_being_written_settles_the_batch_not_the_review() {
        let first = change("a.rs", "one");
        let (spec, work, input) = fixture(
            vec![first.clone(), change("b.rs", "two")],
            std::slice::from_ref(&first.id),
        );
        let mut session = reviewed(&spec);
        session.apply(&ReviewEvent::decision(Decision::Approve));

        let verdict = next_step(
            &spec,
            &session,
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );

        assert_eq!(verdict, Verdict::Expand);
    }

    /// The requests still route: settling the batch says nothing about them.
    #[test]
    fn approving_a_partial_review_does_not_bury_an_open_request() {
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
        session.apply(&ReviewEvent::decision(Decision::Comment));

        assert_eq!(
            next_step(
                &spec,
                &session,
                &work,
                &input,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
            Verdict::Handover {
                feedback_ids: vec!["feedback-1".into()]
            }
        );
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

        let verdict = next_step(
            &spec,
            &session,
            &work,
            &input,
            &input,
            false,
            WriteAhead::Batches(1),
        );

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
            next_step(
                &spec,
                &session,
                &work,
                &drifted,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
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
            next_step(
                &spec,
                &session,
                &work,
                &drifted,
                &input,
                false,
                WriteAhead::Batches(1)
            ),
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

        let verdict = next_step(
            &spec,
            &reviewed(&spec),
            &work,
            &drifted,
            &input,
            false,
            WriteAhead::Batches(1),
        );

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

        let verdict = next_step(
            &spec,
            &reviewed(&spec),
            &work,
            &drifted,
            &input,
            false,
            WriteAhead::Batches(1),
        );

        assert_eq!(
            verdict,
            Verdict::Replan {
                stale_chunks: vec!["chunk-0".into()]
            }
        );
    }
}
