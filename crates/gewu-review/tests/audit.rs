use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, Overview, Quote, ReviewChunk, ReviewInput,
    ReviewSpec, SourceIdentity, ValidatedReview,
    audit::{Audit, LineRange, SymbolIndex},
};

fn model() -> Change {
    Change::new(
        "src/model.rs",
        (1, 0, 1, 3),
        "@@ -0,0 +1,3 @@\n+pub struct Ledger;\n+\n+impl Ledger { fn settle() {} }\n",
    )
}

fn caller() -> Change {
    Change::new(
        "src/caller.rs",
        (1, 0, 1, 1),
        "@@ -0,0 +1 @@\n+fn run() { Ledger::settle(); }\n",
    )
}

fn narrative(markdown: &str) -> ChunkBlock {
    ChunkBlock::Narrative {
        markdown: markdown.into(),
    }
}

fn shows(change: &Change, presentation: ChangePresentation) -> ChunkBlock {
    ChunkBlock::Changes {
        quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
        presentation,
    }
}

fn chunk(id: &str, content: Vec<ChunkBlock>) -> ReviewChunk {
    ReviewChunk {
        id: id.into(),
        title: "Untitled".into(),
        summary: "Untitled.".into(),
        kind: ChunkKind::Other,
        content,
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    }
}

fn review(changes: Vec<Change>, chunks: Vec<ReviewChunk>) -> ValidatedReview {
    let source = SourceIdentity::new("base", "head", b"diff");
    let overview = Overview {
        intent: "Settle a ledger.".into(),
        changes: vec!["Adds a ledger".into()],
        illustration: None,
        concepts: vec![],
    };
    let spec = ReviewSpec::new(source.clone(), overview, chunks);
    ValidatedReview::validate(ReviewInput::new(source, changes), spec).expect("valid review")
}

fn narrated() -> ValidatedReview {
    let (model, caller) = (model(), caller());
    review(
        vec![model.clone(), caller.clone()],
        vec![
            chunk(
                "ledger",
                vec![
                    narrative("`Ledger` settles on demand."),
                    shows(&model, ChangePresentation::AddedCode),
                ],
            ),
            chunk(
                "caller",
                vec![
                    narrative("`run` calls `Ledger::settle`."),
                    shows(&caller, ChangePresentation::AddedCode),
                ],
            ),
        ],
    )
}

#[test]
fn narrated_views_explain_every_reviewable_line() {
    let review = narrated();
    let report = Audit::new(&review, SymbolIndex::from_review(&review)).run();
    assert!(report.passes(), "{report:#?}");
    let ledger = &report.coverage.changes[0];
    // The blank line between the struct and its impl is not reviewable.
    assert_eq!((ledger.reviewable_lines, ledger.explained_lines), (2, 2));
}

#[test]
fn a_view_whose_prose_names_nothing_in_it_is_unexplained() {
    let model = model();
    let review = review(
        vec![model.clone()],
        vec![chunk(
            "bare",
            vec![shows(&model, ChangePresentation::AddedCode)],
        )],
    );
    let report = Audit::new(&review, SymbolIndex::from_review(&review)).run();
    assert!(!report.passes());
    assert_eq!(
        report.coverage.changes[0].unexplained,
        [
            LineRange { start: 1, end: 1 },
            LineRange { start: 3, end: 3 }
        ]
    );
}

/// Self-review found minimal chunks failing for having only a summary.
#[test]
fn a_summary_naming_the_code_explains_its_view() {
    let model = model();
    let mut only = chunk(
        "summary",
        vec![shows(&model, ChangePresentation::AddedCode)],
    );
    only.summary = "`Ledger` settles on demand.".into();
    let review = review(vec![model], vec![only]);
    assert!(
        Audit::new(&review, SymbolIndex::from_review(&review))
            .run()
            .passes()
    );
}

/// Self-review: a summary naming `next` explained its view, but the audit
/// ignored the word as common English.
#[test]
fn a_common_word_in_backticks_still_anchors_a_view() {
    let change = Change::new(
        "a.rs",
        (1, 0, 1, 2),
        "@@ -0,0 +1,2 @@\n+let line = next;\n+next = next + 1;\n",
    );
    let mut only = chunk(
        "anchor",
        vec![shows(&change, ChangePresentation::AddedCode)],
    );
    only.summary = "Only lines that start with `+` advance `next`.".into();
    let review = review(vec![change], vec![only]);
    assert!(
        Audit::new(&review, SymbolIndex::from_review(&review))
            .run()
            .coverage
            .passes()
    );
}

/// Self-review: `core.quotePath` and `/dev/null` name things outside any
/// repository, so they are reported rather than failing the audit.
#[test]
fn names_outside_the_repository_are_reported_not_failed() {
    let model = model();
    let review = review(
        vec![model.clone()],
        vec![chunk(
            "ledger",
            vec![
                narrative("`Ledger` ignores `core.quotePath` and reads `/dev/null`."),
                shows(&model, ChangePresentation::AddedCode),
            ],
        )],
    );
    let report = Audit::new(&review, SymbolIndex::from_review(&review)).run();
    assert!(report.passes(), "{report:#?}");
    let external: Vec<_> = report
        .references
        .external
        .iter()
        .map(|r| r.reference.as_str())
        .collect();
    assert_eq!(external, ["/dev/null", "core.quotePath"]);
}

#[test]
fn narration_about_something_else_does_not_explain_a_view() {
    let model = model();
    let review = review(
        vec![model.clone()],
        vec![chunk(
            "unrelated",
            vec![
                narrative("Configures the release pipeline."),
                shows(&model, ChangePresentation::AddedCode),
            ],
        )],
    );
    let report = Audit::new(&review, SymbolIndex::from_review(&review)).run();
    assert!(!report.coverage.passes());
}

#[test]
fn summary_only_is_disclosed_rather_than_failed() {
    let model = model();
    let review = review(
        vec![model.clone()],
        vec![chunk(
            "generated",
            vec![
                narrative("Generated by the build; `model.rs` is not read by hand."),
                shows(&model, ChangePresentation::SummaryOnly),
            ],
        )],
    );
    let report = Audit::new(&review, SymbolIndex::from_review(&review)).run();
    assert!(report.passes());
    assert_eq!(report.coverage.changes[0].summarized_lines, 2);
}

#[test]
fn a_scope_path_missing_from_the_manifest_fails() {
    let review = narrated();
    let report = Audit::new(&review, SymbolIndex::from_review(&review))
        .with_scope(vec!["src/model.rs".into(), "src/forgotten.rs".into()])
        .run();
    assert!(!report.passes());
    assert_eq!(report.coverage.missing_paths, ["src/forgotten.rs"]);
}

#[test]
fn references_must_resolve_in_the_corpus_or_the_supplied_tree() {
    let model = model();
    let content = || {
        vec![
            narrative("`Ledger` delegates to `Journal::post` in `src/journal.rs`."),
            shows(&model, ChangePresentation::AddedCode),
        ]
    };
    let review = review(vec![model.clone()], vec![chunk("ledger", content())]);
    let report = Audit::new(&review, SymbolIndex::from_review(&review)).run();
    let unresolved: Vec<_> = report
        .references
        .unresolved
        .iter()
        .map(|r| r.reference.as_str())
        .collect();
    assert_eq!(unresolved, ["Journal::post", "src/journal.rs"]);

    let tree = SymbolIndex::from_review(&review)
        .with_words(["Journal".into(), "post".into()])
        .with_paths(["src/journal.rs".into()]);
    assert!(Audit::new(&review, tree).run().passes());

    // A run-time path written only in unchanged code resolves as a literal.
    let literal = SymbolIndex::from_review(&review)
        .with_words(["Journal".into(), "post".into()])
        .with_literals(["let path = root.join(\"src/journal.rs\");".into()]);
    assert!(Audit::new(&review, literal).run().passes());
}

#[test]
fn forward_and_distant_references_are_reported_without_failing() {
    let (model, caller) = (model(), caller());
    let mut chunks = vec![chunk(
        "caller",
        vec![
            narrative("`run` calls `settle` on `Ledger`."),
            shows(&caller, ChangePresentation::AddedCode),
        ],
    )];
    chunks.push(chunk(
        "ledger",
        vec![
            narrative("`Ledger` settles on demand."),
            shows(&model, ChangePresentation::AddedCode),
        ],
    ));
    chunks.push(chunk("later", vec![narrative("`settle` again.")]));
    let review = review(vec![model, caller], chunks);
    let report = Audit::new(&review, SymbolIndex::from_review(&review))
        .with_anchor_distance(1)
        .run();
    assert!(report.passes(), "{report:#?}");
    let forward: Vec<_> = report
        .references
        .forward
        .iter()
        .map(|f| {
            (
                f.chunk_id.as_str(),
                f.symbol.as_str(),
                f.defined_in.as_str(),
            )
        })
        .collect();
    assert_eq!(
        forward,
        [
            ("caller", "Ledger", "ledger"),
            ("caller", "settle", "ledger")
        ]
    );
    let distant = &report.references.distant;
    assert_eq!(distant.len(), 1);
    assert_eq!(
        (distant[0].chunk_id.as_str(), distant[0].symbol.as_str()),
        ("later", "settle")
    );
}
