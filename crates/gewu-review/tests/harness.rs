use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, Illustration, Quote, ReviewChunk,
    ReviewInput, SourceIdentity,
    harness::{
        Expansion, HARNESS_SCHEMA_VERSION, HarnessStage, HarnessState, HarnessStore,
        HarnessValidationError, InvestigationCheckpoint, InvestigationFinding, ReviewOrigin,
        WorkItem, WorkList,
    },
};

fn input() -> ReviewInput {
    let source = SourceIdentity::new("base", "head", b"diff");
    let small = Change::new("src/lib.rs", (1, 1, 1, 1), "@@ -1 +1 @@\n-old\n+new\n");
    let large_patch = std::iter::repeat_n("+line\n", 121).collect::<String>();
    let large = Change::new(
        "Cargo.lock",
        (1, 0, 1, 121),
        format!("@@ -1,0 +1,121 @@\n{large_patch}"),
    );
    ReviewInput::new(source, vec![small, large])
}

fn origin() -> ReviewOrigin {
    ReviewOrigin {
        base: "HEAD".into(),
        head: None,
        working_tree: true,
    }
}

fn work_list(input: &ReviewInput) -> WorkList {
    WorkList {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        items: vec![
            WorkItem {
                id: "behavior".into(),
                title: "Change behavior".into(),
                purpose: "The observable change.".into(),
                covers: vec![Quote::whole(
                    input.changes[0].id.clone(),
                    input.changes[0].body_lines(),
                )],
                expanded: false,
                chunks: Vec::new(),
            },
            WorkItem {
                id: "lock".into(),
                title: "Update dependencies".into(),
                purpose: "Generated lockfile.".into(),
                covers: vec![Quote::whole(
                    input.changes[1].id.clone(),
                    input.changes[1].body_lines(),
                )],
                expanded: false,
                chunks: Vec::new(),
            },
        ],
    }
}

fn chunk(id: &str, quotes: Vec<Quote>, presentation: ChangePresentation) -> ReviewChunk {
    ReviewChunk {
        id: id.into(),
        title: "Chunk".into(),
        summary: "Summary.".into(),
        kind: ChunkKind::Other,
        content: vec![ChunkBlock::Changes {
            quotes,
            presentation,
        }],
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    }
}

#[test]
fn a_work_list_must_cover_the_manifest() {
    let input = input();

    work_list(&input).validate(&input).expect("valid work list");

    let mut partial = work_list(&input);
    partial.items.pop();
    let errors = partial.validate(&input).expect_err("uncovered change").0;
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, HarnessValidationError::UncoveredLines { .. })),
        "{errors:?}"
    );

    let mut overlapping = work_list(&input);
    overlapping.items[1].covers.push(Quote::whole(
        input.changes[0].id.clone(),
        input.changes[0].body_lines(),
    ));
    overlapping
        .validate(&input)
        .expect("overlapping items are allowed; the union is what must cover");
}

#[test]
fn a_multi_change_work_item_must_fit_its_expansion_budget() {
    let input = input();
    let mut list = work_list(&input);
    let generated = list.items.pop().expect("generated item");
    list.items[0].covers.extend(generated.covers);

    let errors = list
        .validate_for(&input, 1)
        .expect_err("the combined item cannot fit one chunk")
        .0;
    assert!(
        errors.iter().any(|error| matches!(
            error,
            HarnessValidationError::ImpossibleItem {
                item,
                chunks: 1,
                ..
            } if item == "behavior"
        )),
        "{errors:?}"
    );

    list.validate_for(&input, 4)
        .expect("a larger expansion budget fits the combined source");
}

#[test]
fn a_large_single_change_item_can_still_use_summary_only() {
    let input = input();
    let list = work_list(&input);

    list.validate_for(&input, 1)
        .expect("one generated artifact may be summarized compactly");
}

#[test]
fn views_may_overlap_as_long_as_their_union_covers_the_item() {
    let input = input();
    let change = &input.changes[1];
    let lines = change.body_lines();
    let item = WorkItem {
        id: "lock".into(),
        title: "The lockfile".into(),
        purpose: "Generated.".into(),
        covers: vec![Quote::whole(change.id.clone(), lines)],
        expanded: false,
        chunks: Vec::new(),
    };
    let view = |start, end| Quote {
        change_id: change.id.clone(),
        start,
        end,
    };
    let expansion = |quotes: Vec<Vec<Quote>>| Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks: quotes
            .into_iter()
            .enumerate()
            .map(|(index, quotes)| {
                chunk(
                    &format!("view-{index}"),
                    quotes,
                    ChangePresentation::SummaryOnly,
                )
            })
            .collect(),
    };

    // Showing lines 10-60 after 1-25 repeats the middle on purpose: the reader
    // needs the earlier context again to make sense of what follows.
    expansion(vec![vec![view(1, 25)], vec![view(10, lines)]])
        .validate(&input, &item, 6)
        .expect("overlapping views are good narrative, not a fault");

    // A gap is a fault: nothing would ever show those lines.
    let errors = expansion(vec![vec![view(1, 25)], vec![view(40, lines)]])
        .validate(&input, &item, 6)
        .expect_err("a gap leaves lines unreviewed")
        .0;
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, HarnessValidationError::UncoveredLines { .. })),
        "{errors:?}"
    );
}

#[test]
fn a_quote_may_not_run_past_the_end_of_its_change() {
    let input = input();
    let change = &input.changes[0];
    let item = WorkItem {
        id: "behavior".into(),
        title: "Behavior".into(),
        purpose: "Purpose.".into(),
        covers: vec![Quote::whole(change.id.clone(), change.body_lines())],
        expanded: false,
        chunks: Vec::new(),
    };
    let expansion = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks: vec![chunk(
            "past-the-end",
            vec![Quote {
                change_id: change.id.clone(),
                start: 1,
                end: change.body_lines() + 10,
            }],
            ChangePresentation::Diff,
        )],
    };

    let errors = expansion
        .validate(&input, &item, 6)
        .expect_err("past end")
        .0;

    assert!(
        errors
            .iter()
            .any(|error| matches!(error, HarnessValidationError::UnreadableQuote { .. })),
        "{errors:?}"
    );
}

#[test]
fn an_expansion_covers_exactly_its_item() {
    let input = input();
    let work = work_list(&input);
    let item = &work.items[0];
    let expansion = |chunks| Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks,
    };

    expansion(vec![chunk(
        "behavior-1",
        vec![Quote::whole(
            input.changes[0].id.clone(),
            input.changes[0].body_lines(),
        )],
        ChangePresentation::Diff,
    )])
    .validate(&input, item, 6)
    .expect("covers its item");

    let errors = expansion(vec![chunk(
        "behavior-1",
        vec![Quote::whole(
            input.changes[1].id.clone(),
            input.changes[1].body_lines(),
        )],
        ChangePresentation::SummaryOnly,
    )])
    .validate(&input, item, 6)
    .expect_err("quotes another item's change")
    .0;
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, HarnessValidationError::QuoteOutsideItem { .. })),
        "{errors:?}"
    );
}

#[test]
fn an_expansion_may_not_exceed_the_chunk_cap() {
    let input = input();
    let work = work_list(&input);
    let item = &work.items[0];
    let expansion = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks: vec![
            chunk(
                "one",
                vec![Quote::whole(
                    input.changes[0].id.clone(),
                    input.changes[0].body_lines(),
                )],
                ChangePresentation::Diff,
            ),
            chunk("two", vec![], ChangePresentation::Diff),
        ],
    };

    let errors = expansion.validate(&input, item, 1).expect_err("over cap").0;

    assert!(
        errors
            .iter()
            .any(|error| matches!(error, HarnessValidationError::TooManyChunks { .. })),
        "{errors:?}"
    );
}

#[test]
fn a_raised_budget_admits_a_chunk_the_default_rejects() {
    let input = input();
    let work = work_list(&input);
    let item = &work.items[1];
    let expansion = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        item_id: item.id.clone(),
        chunks: vec![chunk(
            "lock-1",
            vec![Quote::whole(
                input.changes[1].id.clone(),
                input.changes[1].body_lines(),
            )],
            ChangePresentation::Diff,
        )],
    };

    expansion
        .validate(&input, item, 6)
        .expect_err("rejected by default");
    expansion
        .validate(&input.clone().with_max_chunk_lines(10_000), item, 6)
        .expect("admitted by a raised budget");
}

#[test]
fn persists_and_resumes_harness_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = HarnessStore::new(directory.path());
    let mut state = HarnessState::new("review-1", input().source, origin());
    state.advance(HarnessStage::Plan);
    store.save_state(&state).expect("save state");

    assert_eq!(store.state().expect("load state"), Some(state));
}

#[test]
fn persists_and_resumes_the_work_list() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = HarnessStore::new(directory.path());
    let input = input();
    let mut list = work_list(&input);
    list.mark_expanded("behavior", &[]);
    store.save_work_list(&list).expect("save work list");

    let loaded = store.work_list().expect("load work list").expect("present");

    assert_eq!(loaded.remaining(), 1);
    assert_eq!(loaded.next_item().expect("next").id, "lock");
}

#[test]
fn investigation_must_account_for_every_change() {
    let input = input();
    let investigation = InvestigationCheckpoint {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        intent: "Change behavior.".into(),
        changes: vec!["Behavior changes.".into()],
        concepts: vec![],
        illustration: None,
        findings: vec![InvestigationFinding {
            evidence_spans: vec![],
            change_ids: input
                .changes
                .iter()
                .map(|change| change.id.clone())
                .collect(),
            summary: "The changes work together.".into(),
            evidence: vec!["src/lib.rs".into()],
        }],
    };

    investigation.validate(&input).expect("valid investigation");
}

/// Found by running the desktop app on a real walkthrough: an agent sent an
/// ASCII drawing as a `diagram`, and Mermaid drew a syntax-error graphic.
#[test]
fn an_overview_diagram_must_be_mermaid() {
    let input = input();
    let mut investigation = InvestigationCheckpoint {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: input.source.clone(),
        intent: "Change behavior.".into(),
        changes: vec!["Behavior changes.".into()],
        concepts: vec![],
        illustration: Some(Illustration::Diagram {
            text: "mise.toml + scripts/build.sh\n   |\n   +--> diagrams/".into(),
        }),
        findings: vec![InvestigationFinding {
            evidence_spans: vec![],
            change_ids: input
                .changes
                .iter()
                .map(|change| change.id.clone())
                .collect(),
            summary: "The changes work together.".into(),
            evidence: vec!["src/lib.rs".into()],
        }],
    };

    let errors = investigation.validate(&input).expect_err("ascii art").0;
    assert_eq!(errors, vec![HarnessValidationError::UndrawableIllustration]);

    investigation.illustration = Some(Illustration::Diagram {
        text: "%% the build\nflowchart LR\n  toml --> script".into(),
    });
    investigation.validate(&input).expect("a Mermaid flowchart");
}
