//! Fixture loading and committed snapshot freshness.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use gewu_app::{Action, ReviewWorkspace, Snapshot};
use gewu_review::{FeedbackKind, FeedbackTarget, Side};

fn ui_fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui")
}

fn copy_fixture(name: &str) -> tempfile::TempDir {
    let source = ui_fixtures().join(name);
    let temp = tempfile::tempdir().expect("tempdir");
    copy_dir(&source, temp.path());
    temp
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy");
        }
    }
}

fn assert_snapshot(name: &str) {
    let fixture = ui_fixtures().join(name);
    let workspace = copy_fixture(name);
    let opened = ReviewWorkspace::open(workspace.path()).expect("open");
    let actual = opened.snapshot();
    let path = fixture.join("snapshot.json");
    if env::var_os("GEWU_UPDATE_FIXTURES").is_some() {
        let pretty = serde_json::to_string_pretty(&actual).expect("serialize");
        fs::write(&path, format!("{pretty}\n")).expect("write snapshot");
        return;
    }
    let expected: Snapshot =
        serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "missing {}; run with GEWU_UPDATE_FIXTURES=1 to generate",
                path.display()
            )
        }))
        .expect("parse snapshot");
    assert_eq!(
        actual, expected,
        "stale snapshot for {name}; run with GEWU_UPDATE_FIXTURES=1"
    );
}

#[test]
fn showcase_loads_through_validate_partial() {
    let root = copy_fixture("showcase");
    let workspace = ReviewWorkspace::open(root.path()).expect("showcase must open");
    let snapshot = workspace.snapshot();
    assert_eq!(snapshot.review_id, "review-ui-showcase");
    assert_eq!(snapshot.chunks.len(), 7);
    assert!(!snapshot.generation.complete);
    assert_eq!(snapshot.generation.remaining_work_items, Some(2));
    assert!(snapshot.summary.batch_scoped);
    let presentations: Vec<_> = snapshot
        .chunks
        .iter()
        .flat_map(|chunk| &chunk.blocks)
        .filter_map(|block| match block {
            gewu_app::BlockView::Changes { presentation, .. } => Some(*presentation),
            gewu_app::BlockView::Narrative { .. } | gewu_app::BlockView::Explainer(_) => None,
        })
        .collect();
    assert!(presentations.contains(&gewu_review::ChangePresentation::Diff));
    assert!(presentations.contains(&gewu_review::ChangePresentation::AddedCode));
    assert!(presentations.contains(&gewu_review::ChangePresentation::RenderedMarkdown));
    assert!(presentations.contains(&gewu_review::ChangePresentation::SummaryOnly));
    assert!(
        snapshot
            .chunks
            .iter()
            .any(|chunk| chunk.oversized_reason.is_some())
    );
    assert!(snapshot.overview.illustration.is_some());
    assert!(!snapshot.overview.concepts.is_empty());
}

#[test]
fn showcase_feedback_exposes_notes_and_issues() {
    let root = copy_fixture("showcase-feedback");
    let snapshot = ReviewWorkspace::open(root.path()).expect("open").snapshot();
    assert_eq!(snapshot.feedback.len(), 3);
    assert_eq!(snapshot.summary.unresolved_issues, ["feedback-issue-open"]);
    assert_eq!(snapshot.last_visited.as_deref(), Some("showcase-guide-md"));
    assert!(snapshot.chunks.iter().any(|chunk| chunk.status.reviewed));
}

#[test]
fn self_review_snapshot_is_fresh() {
    assert_snapshot("self-review");
}

#[test]
fn showcase_snapshot_is_fresh() {
    assert_snapshot("showcase");
}

#[test]
fn showcase_feedback_snapshot_is_fresh() {
    assert_snapshot("showcase-feedback");
}

#[test]
fn dispatch_rejects_unknown_chunk_without_mutating() {
    let root = copy_fixture("showcase");
    let mut workspace = ReviewWorkspace::open(root.path()).expect("open");
    let before = workspace.snapshot();
    let error = workspace
        .dispatch(Action::Visit {
            chunk_id: "missing".into(),
        })
        .expect_err("unknown chunk");
    assert!(error.to_string().contains("unknown chunk"));
    assert_eq!(workspace.snapshot(), before);
}

#[test]
fn dispatch_adds_feedback_with_filled_anchor() {
    let root = copy_fixture("showcase");
    let mut workspace = ReviewWorkspace::open(root.path()).expect("open");
    let chunk = &workspace.snapshot().chunks[0];
    let quote = chunk
        .blocks
        .iter()
        .find_map(|block| match block {
            gewu_app::BlockView::Changes { quotes, .. } => quotes.first(),
            gewu_app::BlockView::Narrative { .. } | gewu_app::BlockView::Explainer(_) => None,
        })
        .expect("quote");
    let new_line = quote
        .lines
        .iter()
        .find_map(|line| line.new_line)
        .expect("new-side line");
    let snapshot = workspace
        .dispatch(Action::AddFeedback {
            chunk_id: chunk.id.clone(),
            kind: FeedbackKind::Note,
            target: FeedbackTarget::Code,
            body: "Looks good.".into(),
            anchor: Some(gewu_app::AnchorRef {
                change_id: quote.change_id.clone(),
                start_line: new_line,
                end_line: new_line,
                side: Side::New,
            }),
        })
        .expect("add feedback");
    let feedback = snapshot.feedback.last().expect("feedback");
    let anchor = feedback.anchor.as_ref().expect("anchor");
    assert_eq!(anchor.path, quote.path);
    assert_eq!(anchor.side, "new");
    assert!(!anchor.patch_sha256.is_empty());
}

#[test]
fn dispatch_rejects_empty_body_and_bad_anchor() {
    let root = copy_fixture("showcase");
    let mut workspace = ReviewWorkspace::open(root.path()).expect("open");
    let chunk_id = workspace.snapshot().chunks[0].id.clone();
    assert!(
        workspace
            .dispatch(Action::AddFeedback {
                chunk_id: chunk_id.clone(),
                kind: FeedbackKind::Note,
                target: FeedbackTarget::Code,
                body: "   ".into(),
                anchor: None,
            })
            .is_err()
    );
    assert!(
        workspace
            .dispatch(Action::AddFeedback {
                chunk_id,
                kind: FeedbackKind::Issue,
                target: FeedbackTarget::Code,
                body: "request".into(),
                anchor: Some(gewu_app::AnchorRef {
                    change_id: "change-missing".into(),
                    start_line: 1,
                    end_line: 1,
                    side: Side::New,
                }),
            })
            .is_err()
    );
}

/// The first chunk quote that carries an old-side line, with its path and the
/// old-side span of the lines the quote shows.
fn quote_with_old_lines(workspace: &ReviewWorkspace) -> (String, String, u32, u32) {
    let snapshot = workspace.snapshot();
    let quote = snapshot
        .chunks
        .iter()
        .flat_map(|chunk| chunk.blocks.iter())
        .find_map(|block| match block {
            gewu_app::BlockView::Changes { quotes, .. } => quotes.iter().find(|quote| {
                quote
                    .lines
                    .iter()
                    .any(|line| line.old_line.is_some() && line.new_line.is_none())
            }),
            gewu_app::BlockView::Narrative { .. } | gewu_app::BlockView::Explainer(_) => None,
        })
        .expect("a quote with a removed line");
    let old: Vec<u32> = quote
        .lines
        .iter()
        .filter_map(|line| line.old_line)
        .collect();
    (
        quote.change_id.clone(),
        quote.path.clone(),
        *old.iter().min().expect("old line"),
        *old.iter().max().expect("old line"),
    )
}

#[test]
fn dispatch_anchors_a_span_on_the_old_side() {
    let root = copy_fixture("showcase");
    let mut workspace = ReviewWorkspace::open(root.path()).expect("open");
    let (change_id, path, start, end) = quote_with_old_lines(&workspace);
    let chunk_id = workspace
        .snapshot()
        .chunks
        .iter()
        .find(|chunk| {
            chunk.blocks.iter().any(|block| match block {
                gewu_app::BlockView::Changes { quotes, .. } => {
                    quotes.iter().any(|quote| quote.change_id == change_id)
                }
                gewu_app::BlockView::Narrative { .. } | gewu_app::BlockView::Explainer(_) => false,
            })
        })
        .expect("chunk")
        .id
        .clone();

    let snapshot = workspace
        .dispatch(Action::AddFeedback {
            chunk_id: chunk_id.clone(),
            kind: FeedbackKind::Issue,
            target: FeedbackTarget::Code,
            body: "Removed on purpose?".into(),
            anchor: Some(gewu_app::AnchorRef {
                change_id: change_id.clone(),
                start_line: start,
                end_line: end,
                side: Side::Old,
            }),
        })
        .expect("old-side anchor");
    let anchor = snapshot
        .feedback
        .last()
        .and_then(|item| item.anchor.clone())
        .expect("anchor");
    assert_eq!(anchor.side, "old");
    assert_eq!(anchor.path, path);

    let outside = workspace.dispatch(Action::AddFeedback {
        chunk_id,
        kind: FeedbackKind::Issue,
        target: FeedbackTarget::Code,
        body: "Too far.".into(),
        anchor: Some(gewu_app::AnchorRef {
            change_id,
            start_line: end + 100,
            end_line: end + 100,
            side: Side::Old,
        }),
    });
    assert!(outside.is_err(), "a line outside the quote is rejected");
}
