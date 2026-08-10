use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, Decision, EventLog, Feedback, FeedbackKind,
    FeedbackTarget, Overview, Quote, ReviewChunk, ReviewEvent, ReviewInput, ReviewSpec,
    SourceIdentity, ValidatedReview, ValidationError,
};

fn fixture() -> (ReviewInput, ReviewSpec) {
    let source = SourceIdentity::new("base", "head", b"diff");
    let first = Change::new(
        "src/model.rs",
        (1, 1, 1, 2),
        "@@ -1 +1,2 @@\n-old\n+new\n+line\n",
    );
    let second = Change::new("tests/model.rs", (4, 0, 4, 3), "@@ -4,0 +4,3 @@\n+test\n");
    let spec = ReviewSpec::new(
        source.clone(),
        Overview {
            intent: "Introduce a model and verify its behavior.".into(),
            changes: vec!["Adds the model".into(), "Tests the behavior".into()],
            illustration: None,
            concepts: vec![],
        },
        vec![
            ReviewChunk {
                id: "model".into(),
                title: "Define the model".into(),
                summary: "Introduces the domain model.".into(),
                kind: ChunkKind::Concept,
                content: vec![ChunkBlock::Changes {
                    quotes: vec![Quote::whole(first.id.clone(), first.body_lines())],
                    presentation: ChangePresentation::Diff,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
            },
            ReviewChunk {
                id: "tests".into(),
                title: "Verify the model".into(),
                summary: "Covers the model behavior.".into(),
                kind: ChunkKind::Test,
                content: vec![ChunkBlock::Changes {
                    quotes: vec![Quote::whole(second.id.clone(), second.body_lines())],
                    presentation: ChangePresentation::Diff,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
            },
        ],
    );
    (ReviewInput::new(source, vec![first, second]), spec)
}

#[test]
fn accepts_different_agent_groupings() {
    let (input, spec) = fixture();
    ValidatedReview::validate(input.clone(), spec).expect("first agent spec");

    let mut alternative = fixture().1;
    alternative.chunks = vec![ReviewChunk {
        id: "model-and-tests".into(),
        title: "Add the tested model".into(),
        summary: "Introduces the model together with its behavioral coverage.".into(),
        kind: ChunkKind::Other,
        content: vec![ChunkBlock::Changes {
            quotes: input
                .changes
                .iter()
                .map(|change| Quote::whole(change.id.clone(), change.body_lines()))
                .collect(),
            presentation: ChangePresentation::Diff,
        }],
        context: None,
        confidence: Some("Grouping chosen by another agent".into()),
        oversized_reason: None,
    }];

    ValidatedReview::validate(input, alternative).expect("second agent spec");
}

#[test]
fn rejects_stale_and_incomplete_agent_output() {
    let (input, mut spec) = fixture();
    spec.source.head = "other-head".into();
    spec.chunks.pop();

    let errors = ValidatedReview::validate(input, spec)
        .expect_err("invalid spec")
        .0;

    assert!(errors.contains(&ValidationError::StaleSource));
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::UncoveredLines { .. }))
    );
}

#[test]
fn replays_durable_review_session() {
    let directory = tempfile::tempdir().expect("temp directory");
    let log = EventLog::new(directory.path().join("events.jsonl"));
    let feedback = Feedback {
        id: "issue-1".into(),
        chunk_id: "model".into(),
        kind: FeedbackKind::Issue,
        target: FeedbackTarget::Code,
        body: "Handle the empty case.".into(),
        anchor: None,
        resolved: false,
    };
    for event in [
        ReviewEvent::started("review-spec"),
        ReviewEvent::visited("model"),
        ReviewEvent::reviewed("model", true),
        ReviewEvent::feedback(feedback),
        ReviewEvent::retargeted("issue-1", FeedbackTarget::Explanation),
        ReviewEvent::resolved("issue-1", true),
        ReviewEvent::decision(Decision::Approve),
    ] {
        log.append(&event).expect("append event");
    }

    let session = log.load().expect("replay events");

    assert_eq!(session.spec_id.as_deref(), Some("review-spec"));
    assert!(session.visited.contains("model"));
    assert!(session.reviewed.contains("model"));
    assert!(session.feedback["issue-1"].resolved);
    assert_eq!(
        session.feedback["issue-1"].target,
        FeedbackTarget::Explanation
    );
    assert_eq!(session.decision, Some(Decision::Approve));
}

#[test]
fn a_spec_may_show_the_same_lines_in_more_than_one_view() {
    let source = SourceIdentity::new("base", "head", b"diff");
    let body: String = (1..=50).map(|line| format!("+line {line}\n")).collect();
    let change = Change::new("a.rs", (0, 0, 1, 50), format!("@@ -0,0 +1,50 @@\n{body}"));
    let view = |id: &str, start, end| ReviewChunk {
        id: id.into(),
        title: "View".into(),
        summary: "Summary.".into(),
        kind: ChunkKind::Other,
        content: vec![ChunkBlock::Changes {
            quotes: vec![Quote {
                change_id: change.id.clone(),
                start,
                end,
            }],
            presentation: ChangePresentation::SummaryOnly,
        }],
        context: None,
        confidence: None,
        oversized_reason: None,
    };
    let spec = |chunks| {
        ReviewSpec::new(
            source.clone(),
            Overview {
                intent: "Intent.".into(),
                changes: vec![],
                illustration: None,
                concepts: vec![],
            },
            chunks,
        )
    };
    let input = ReviewInput::new(source.clone(), vec![change.clone()]);

    // The reader needs lines 10-25 again to follow the second view.
    ValidatedReview::validate(
        input.clone(),
        spec(vec![view("first", 1, 25), view("second", 10, 50)]),
    )
    .expect("overlapping views are how a walkthrough repeats context");

    // A gap is still a gap.
    let errors = ValidatedReview::validate(
        input,
        spec(vec![view("first", 1, 25), view("second", 40, 50)]),
    )
    .expect_err("lines 26-39 would never be shown")
    .0;
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, ValidationError::UncoveredLines { .. })),
        "{errors:?}"
    );
}
