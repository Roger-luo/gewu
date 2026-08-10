use std::fs;

use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, Feedback, FeedbackKind, FeedbackTarget,
    Quote, ReviewChunk, ReviewEvent, ReviewInput, ReviewSpec, SourceIdentity,
    harness::{
        Expansion, HARNESS_SCHEMA_VERSION, HarnessStage, InvestigationCheckpoint,
        InvestigationFinding, ReviewOrigin, Revision, WorkItem, WorkList,
    },
};

use super::HarnessRunner;
use crate::harness::artifacts;

struct Fixture {
    _directory: tempfile::TempDir,
    runner: HarnessRunner,
    input: ReviewInput,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary directory");
    let runner = HarnessRunner::at(directory.path());
    let input = ReviewInput::new(
        SourceIdentity::new("base", "head", b"diff"),
        vec![
            Change::new("src/one.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+one\n"),
            Change::new("src/two.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+two\n"),
        ],
    );
    Fixture {
        _directory: directory,
        runner,
        input,
    }
}

impl Fixture {
    /// Runs investigate and plan, then expands the first item.
    fn expand_first(&self) -> WorkList {
        self.runner
            .start(
                "review-test".into(),
                self.input.clone(),
                ReviewOrigin {
                    base: "HEAD".into(),
                    head: Some("HEAD".into()),
                    working_tree: false,
                },
            )
            .expect("start");
        self.runner
            .store
            .save_investigation(&self.investigation())
            .expect("save investigation");
        let plan = self.runner.next().expect("advance to plan");
        assert!(plan.starts_with("# Gewu harness: plan"), "{plan}");

        let list = self.work_list();
        self.runner
            .store
            .save_work_list(&list)
            .expect("save work list");
        let expand = self.runner.next().expect("advance to expand");
        assert!(
            expand.starts_with("# Gewu harness: expand `item-0`"),
            "{expand}"
        );

        self.stage_expansion(&list.items[0]);
        let awaiting = self.runner.next().expect("append expansion");
        assert!(
            awaiting.starts_with("# Gewu harness: waiting for the reviewer"),
            "{awaiting}"
        );
        list
    }

    fn investigation(&self) -> InvestigationCheckpoint {
        InvestigationCheckpoint {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: self.input.source.clone(),
            intent: "Add two modules.".into(),
            changes: vec!["Two modules appear.".into()],
            concepts: vec![],
            findings: vec![InvestigationFinding {
                change_ids: self
                    .input
                    .changes
                    .iter()
                    .map(|change| change.id.clone())
                    .collect(),
                summary: "Two independent additions.".into(),
                evidence: vec![],
            }],
        }
    }

    fn work_list(&self) -> WorkList {
        WorkList {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: self.input.source.clone(),
            items: self
                .input
                .changes
                .iter()
                .enumerate()
                .map(|(index, change)| WorkItem {
                    id: format!("item-{index}"),
                    title: format!("Item {index}"),
                    purpose: "Purpose.".into(),
                    covers: vec![Quote::whole(change.id.clone(), change.body_lines())],
                    expanded: false,
                })
                .collect(),
        }
    }

    fn stage_expansion(&self, item: &WorkItem) {
        let expansion = Expansion {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: self.input.source.clone(),
            item_id: item.id.clone(),
            chunks: vec![ReviewChunk {
                id: format!("chunk-{}", item.id),
                title: item.title.clone(),
                summary: "Summary.".into(),
                kind: ChunkKind::Other,
                content: vec![ChunkBlock::Changes {
                    quotes: item.covers.clone(),
                    presentation: ChangePresentation::AddedCode,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
            }],
        };
        artifacts::write(self.runner.store.expansion_path(), &expansion).expect("write expansion");
    }

    fn spec(&self) -> ReviewSpec {
        artifacts::read(&self.runner.spec_path).expect("spec")
    }

    fn record(&self, event: &ReviewEvent) {
        let path = self
            .runner
            .reviews_root
            .join("review-test")
            .join("events.jsonl");
        fs::create_dir_all(path.parent().expect("parent")).expect("session directory");
        let mut line = serde_json::to_string(event).expect("serialize");
        line.push('\n');
        let existing = fs::read_to_string(&path).unwrap_or_default();
        fs::write(&path, existing + &line).expect("append event");
    }
}

#[test]
fn expands_one_item_and_then_waits_for_the_reviewer() {
    let fixture = fixture();

    fixture.expand_first();

    let spec = fixture.spec();
    assert_eq!(spec.chunks.len(), 1, "only the first item was expanded");
    assert_eq!(spec.review_id, "review-test");
    let work = fixture
        .runner
        .store
        .work_list()
        .expect("load")
        .expect("present");
    assert_eq!(work.remaining(), 1);
    assert!(
        !fixture.runner.store.expansion_path().exists(),
        "the expansion was consumed"
    );

    let waiting = fixture.runner.next().expect("gate");

    assert!(waiting.contains("still unreviewed"), "{waiting}");
    assert_eq!(fixture.spec().chunks.len(), 1, "nothing more was generated");
}

#[test]
fn expands_the_next_item_once_the_batch_is_reviewed() {
    let fixture = fixture();
    let list = fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));

    let expand = fixture.runner.next().expect("gate opens");

    assert!(
        expand.starts_with("# Gewu harness: expand `item-1`"),
        "{expand}"
    );
    fixture.stage_expansion(&list.items[1]);
    fixture.runner.next().expect("append second expansion");
    let spec = fixture.spec();
    assert_eq!(spec.chunks.len(), 2, "chunks accumulate");
    assert_eq!(spec.review_id, "review-test", "session identity is stable");
}

fn request(chunk_id: &str, target: FeedbackTarget) -> Feedback {
    Feedback {
        id: "feedback-1".into(),
        chunk_id: chunk_id.into(),
        kind: FeedbackKind::Issue,
        target,
        body: "Rework this.".into(),
        anchor: None,
        resolved: false,
    }
}

#[test]
fn a_request_about_the_code_is_handed_back_instead_of_expanding() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.record(&ReviewEvent::feedback(request(
        "chunk-item-0",
        FeedbackTarget::Code,
    )));

    let prompt = fixture.runner.next().expect("gate");

    assert!(prompt.contains("asked for changes to the code"), "{prompt}");
    assert!(prompt.contains("Rework this."), "{prompt}");
    assert!(prompt.contains("feedback-1"), "{prompt}");
    assert_eq!(fixture.spec().chunks.len(), 1, "nothing new is generated");
}

/// The whole point of the split: a complaint about the walkthrough must not
/// send an agent off to edit source that the reviewer never faulted.
#[test]
fn a_request_about_the_walkthrough_asks_for_a_rewrite() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.record(&ReviewEvent::feedback(request(
        "chunk-item-0",
        FeedbackTarget::Explanation,
    )));

    let prompt = fixture.runner.next().expect("gate");

    assert!(
        prompt.starts_with("# Gewu harness: rewrite 1 chunk"),
        "{prompt}"
    );
    assert!(prompt.contains("chunk-item-0"), "{prompt}");
    let state = fixture
        .runner
        .store
        .state()
        .expect("state")
        .expect("a run exists");
    assert_eq!(state.stage, HarnessStage::Revise);
    assert_eq!(state.revising, vec!["chunk-item-0".to_owned()]);
    // Reprinting must not lose what the harness asked for.
    let again = fixture.runner.status().expect("status");
    assert!(again.contains("chunk-item-0"), "{again}");
}

#[test]
fn a_rewrite_replaces_the_chunk_and_reopens_the_gate() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.record(&ReviewEvent::feedback(request(
        "chunk-item-0",
        FeedbackTarget::Explanation,
    )));
    fixture.runner.next().expect("rewrite requested");
    let mut rewritten = fixture.spec().chunks[0].clone();
    rewritten.summary = "Said plainly, without the invented word.".into();
    artifacts::write(
        fixture.runner.store.revision_path(),
        &Revision {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: fixture.input.source.clone(),
            chunks: vec![rewritten],
        },
    )
    .expect("write revision");

    let accepted = fixture.runner.next().expect("accept rewrite");

    assert!(accepted.contains("rewrite accepted"), "{accepted}");
    assert_eq!(
        fixture.spec().chunks.len(),
        1,
        "a rewrite replaces in place"
    );
    assert_eq!(
        fixture.spec().chunks[0].summary,
        "Said plainly, without the invented word."
    );
    // Still gated: the reviewer's request is theirs to close, not the agent's
    // to outrun by writing a rewrite.
    let still = fixture.runner.next().expect("re-evaluate");
    assert!(still.starts_with("# Gewu harness: rewrite"), "{still}");

    fixture.record(&ReviewEvent::resolved("feedback-1", true));
    let onward = fixture.runner.next().expect("gate opens");
    assert!(onward.contains("expand `item-1`"), "{onward}");
}

/// Measuring a draft is an inner-loop action, and the revise prompt carries
/// every chunk it asked about. Found by using Gewu on itself: each measurement
/// pass re-printed some 250 lines the agent already had.
#[test]
fn measuring_a_draft_rewrite_reports_heights_rather_than_the_prompt_again() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.record(&ReviewEvent::feedback(request(
        "chunk-item-0",
        FeedbackTarget::Explanation,
    )));
    fixture.runner.next().expect("rewrite requested");
    artifacts::write(
        fixture.runner.store.revision_path(),
        &Revision {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: fixture.input.source.clone(),
            chunks: vec![fixture.spec().chunks[0].clone()],
        },
    )
    .expect("write revision");

    let measured = fixture.runner.status().expect("status");

    assert!(measured.starts_with("Measured heights:"), "{measured}");
    assert!(measured.contains("chunk-item-0"), "{measured}");
    assert!(
        !measured.contains("The code is not at fault here"),
        "the prompt is repeated: {measured}"
    );
}

/// A rewrite that renamed a chunk would orphan the very feedback it answers,
/// and one that dropped lines would spend the spec's coverage of the manifest.
#[test]
fn a_rewrite_may_not_rename_a_chunk_or_drop_its_lines() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.record(&ReviewEvent::feedback(request(
        "chunk-item-0",
        FeedbackTarget::Explanation,
    )));
    fixture.runner.next().expect("rewrite requested");
    let mut renamed = fixture.spec().chunks[0].clone();
    renamed.id = "chunk-renamed".into();
    artifacts::write(
        fixture.runner.store.revision_path(),
        &Revision {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: fixture.input.source.clone(),
            chunks: vec![renamed],
        },
    )
    .expect("write revision");

    let rejected = fixture.runner.next().expect("reject rewrite");

    assert!(rejected.contains("chunk-item-0"), "{rejected}");
    assert!(rejected.contains("needs correction"), "{rejected}");
    assert_eq!(
        fixture.spec().chunks[0].id,
        "chunk-item-0",
        "spec untouched"
    );
}

#[test]
fn a_rejected_expansion_leaves_the_spec_untouched() {
    let fixture = fixture();
    let list = fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.runner.next().expect("gate opens");
    // Quote a change owned by the item that was already expanded.
    let mut wrong = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: fixture.input.source.clone(),
        item_id: list.items[1].id.clone(),
        chunks: vec![ReviewChunk {
            id: "chunk-wrong".into(),
            title: "Wrong".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Other,
            content: vec![],
            context: None,
            confidence: None,
            oversized_reason: None,
        }],
    };
    wrong.chunks[0].content = vec![ChunkBlock::Changes {
        quotes: vec![Quote::whole(
            fixture.input.changes[0].id.clone(),
            fixture.input.changes[0].body_lines(),
        )],
        presentation: ChangePresentation::AddedCode,
    }];
    artifacts::write(fixture.runner.store.expansion_path(), &wrong).expect("write expansion");

    let repair = fixture.runner.next().expect("validate expansion");

    assert!(
        repair.starts_with("# Gewu harness: checkpoint needs correction"),
        "{repair}"
    );
    assert_eq!(
        fixture.spec().chunks.len(),
        1,
        "the bad expansion was not appended"
    );
    let state = fixture
        .runner
        .store
        .state()
        .expect("state")
        .expect("present");
    assert_eq!(state.stage, HarnessStage::Repair);
}

#[test]
fn a_decision_completes_the_review_even_with_items_left() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::decision(gewu_review::Decision::Approve));

    let complete = fixture.runner.next().expect("gate");

    assert!(complete.starts_with("# COMPLETE"), "{complete}");
    assert!(complete.contains("review-test"));
}
