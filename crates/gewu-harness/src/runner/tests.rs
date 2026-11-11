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
use crate::artifacts;

pub(in crate::runner) struct Fixture {
    _directory: tempfile::TempDir,
    pub(in crate::runner) runner: HarnessRunner,
    pub(in crate::runner) input: ReviewInput,
}

pub(in crate::runner) fn fixture() -> Fixture {
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
        let (list, awaiting) = self.append_first();
        assert!(
            awaiting.starts_with("# Gewu harness: waiting for the reviewer"),
            "{awaiting}"
        );
        list
    }

    /// Runs investigate and plan, appends the first item's expansion, and
    /// returns what the harness asked for next.
    pub(in crate::runner) fn append_first(&self) -> (WorkList, String) {
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
        let next = self.runner.next().expect("append expansion");
        (list, next)
    }

    pub(in crate::runner) fn investigation(&self) -> InvestigationCheckpoint {
        InvestigationCheckpoint {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: self.input.source.clone(),
            intent: "Add two modules.".into(),
            changes: vec!["Two modules appear.".into()],
            concepts: vec![],
            illustration: None,
            findings: vec![InvestigationFinding {
                evidence_spans: vec![],
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

    pub(in crate::runner) fn work_list(&self) -> WorkList {
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
                    chunks: Vec::new(),
                    tests: false,
                })
                .collect(),
        }
    }

    /// Starts a run and walks it to the expand stage, as an agent would.
    pub(in crate::runner) fn reach_expand(&self) -> WorkList {
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
        self.runner.next().expect("advance to plan");
        let list = self.work_list();
        self.runner
            .store
            .save_work_list(&list)
            .expect("save work list");
        self.runner.next().expect("advance to expand");
        list
    }

    pub(in crate::runner) fn stage_expansion(&self, item: &WorkItem) {
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
                section: None,
            }],
        };
        artifacts::write(self.runner.store.expansion_path(), &expansion).expect("write expansion");
    }

    pub(in crate::runner) fn spec(&self) -> ReviewSpec {
        artifacts::read(&self.runner.spec_path).expect("spec")
    }

    /// Marks a chunk reviewed, as the reader would.
    pub(in crate::runner) fn record_reviewed(&self, chunk_id: &str) {
        self.record(&ReviewEvent::reviewed(chunk_id.to_string(), true));
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
    assert_eq!(
        spec.remaining_work_items(),
        Some(1),
        "the UI must know this is a generated batch, not the whole review"
    );
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
fn plan_rejects_a_multi_change_item_that_cannot_fit_its_expansion() {
    let mut fixture = fixture();
    fixture.input = fixture.input.clone().with_max_chunk_lines(1);
    fixture.runner.max_chunks = 1;
    fixture
        .runner
        .start(
            "review-test".into(),
            fixture.input.clone(),
            ReviewOrigin {
                base: "HEAD".into(),
                head: Some("HEAD".into()),
                working_tree: false,
            },
        )
        .expect("start");
    fixture
        .runner
        .store
        .save_investigation(&fixture.investigation())
        .expect("save investigation");
    fixture.runner.next().expect("advance to plan");
    let mut list = fixture.work_list();
    let second = list.items.pop().expect("second item");
    list.items[0].covers.extend(second.covers);
    fixture
        .runner
        .store
        .save_work_list(&list)
        .expect("save impossible plan");

    let retry = fixture.runner.next().expect("validate plan");

    assert!(retry.contains("checkpoint needs correction"), "{retry}");
    assert!(retry.contains("split the item"), "{retry}");
    assert!(retry.contains("1-chunk budget"), "{retry}");
    assert_eq!(
        fixture
            .runner
            .store
            .state()
            .expect("state")
            .expect("present")
            .stage,
        HarnessStage::Plan
    );
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
    // Self-review authors could not tell "no feedback" from "ignored".
    assert!(expand.contains("left no feedback"), "{expand}");
    // Each chunk names the part of the walkthrough it was written for.
    assert_eq!(
        fixture.spec().chunks[0].section.as_deref(),
        Some(list.items[0].title.as_str())
    );
    fixture.stage_expansion(&list.items[1]);
    fixture.runner.next().expect("append second expansion");
    let spec = fixture.spec();
    assert_eq!(spec.chunks.len(), 2, "chunks accumulate");
    assert_eq!(spec.review_id, "review-test", "session identity is stable");
    assert_eq!(spec.remaining_work_items(), Some(0));
}

/// Draft measurement is an edit-loop action. Once the author has the prompt
/// and a draft, repeating the full prompt hides the numbers they asked for.
#[test]
fn measuring_an_expansion_reports_heights_without_repeating_the_prompt() {
    let fixture = fixture();
    fixture
        .runner
        .start(
            "review-test".into(),
            fixture.input.clone(),
            ReviewOrigin {
                base: "HEAD".into(),
                head: Some("HEAD".into()),
                working_tree: false,
            },
        )
        .expect("start");
    fixture
        .runner
        .store
        .save_investigation(&fixture.investigation())
        .expect("save investigation");
    fixture.runner.next().expect("advance to plan");
    let list = fixture.work_list();
    fixture
        .runner
        .store
        .save_work_list(&list)
        .expect("save work list");
    fixture.runner.next().expect("advance to expand");
    fixture.stage_expansion(&list.items[0]);

    let measured = fixture.runner.status().expect("measure expansion");

    assert!(measured.starts_with("Measured heights:"), "{measured}");
    assert!(measured.contains("chunk-item-0"), "{measured}");
    // Self-review found authors guessing whether "40 of 40" passes.
    assert!(measured.contains("rows, fits"), "{measured}");
    assert!(measured.contains("wrapped source"), "{measured}");
    assert!(measured.contains("blanks after prose"), "{measured}");
    assert!(measured.contains("measure again"), "{measured}");
    assert!(
        !measured.contains("Spans this item covers"),
        "the prompt is repeated: {measured}"
    );
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
            section: None,
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

/// Found by using Gewu on itself: settling the first batch of ninety-four read
/// as a finished review of one change in sixty-two.
#[test]
fn settling_a_review_with_items_left_expands_the_next_one() {
    let fixture = fixture();
    fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.record(&ReviewEvent::decision(gewu_review::Decision::Approve));

    let prompt = fixture.runner.next().expect("gate");

    assert!(prompt.contains("expand `item-1`"), "{prompt}");
}

#[test]
fn a_decision_completes_a_review_that_covers_everything() {
    let fixture = fixture();
    let list = fixture.expand_first();
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-0"), true));
    fixture.runner.next().expect("gate opens");
    fixture.stage_expansion(&list.items[1]);
    fixture.runner.next().expect("append the last expansion");
    fixture.record(&ReviewEvent::reviewed(String::from("chunk-item-1"), true));
    fixture.record(&ReviewEvent::decision(gewu_review::Decision::Approve));

    let complete = fixture.runner.next().expect("gate");

    assert!(complete.starts_with("# COMPLETE"), "{complete}");
    assert!(complete.contains("review-test"));
}

#[test]
fn author_prompts_preserve_findings_and_explain_enforced_limits() {
    let f = fixture();
    f.runner
        .start(
            "review-test".into(),
            f.input.clone(),
            ReviewOrigin {
                base: "HEAD".into(),
                head: Some("HEAD".into()),
                working_tree: false,
            },
        )
        .expect("start");
    let investigation = f.investigation();
    f.runner
        .store
        .save_investigation(&investigation)
        .expect("investigation");
    let plan = f.runner.next().expect("plan");
    assert!(plan.contains("40 × 6 non-blank source rows"), "{plan}");
    f.runner
        .store
        .save_work_list(&f.work_list())
        .expect("worklist");
    let expand = f.runner.next().expect("expand");
    assert!(
        expand.contains("Two independent additions."),
        "findings must survive: {expand}"
    );
    assert!(
        expand.contains("500"),
        "summary validation limit must be explicit: {expand}"
    );
}
