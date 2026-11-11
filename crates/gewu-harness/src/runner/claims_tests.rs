//! Claims are checked against the author's description beside the review
//! input, at the runner, where every way of submitting an expansion passes.

use gewu_review::{
    ChangePresentation, ChunkBlock, ChunkKind, Explainer, ReviewChunk,
    harness::{Expansion, HARNESS_SCHEMA_VERSION},
};

use super::tests::fixture;
use crate::artifacts;

fn stage_claim(fixture: &super::tests::Fixture, claim: &str) {
    let work = fixture
        .runner
        .store
        .work_list()
        .expect("load")
        .expect("present");
    let item = work.next_item().expect("an item left");
    let expansion = Expansion {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: fixture.input.source.clone(),
        item_id: item.id.clone(),
        chunks: vec![ReviewChunk {
            id: format!("chunk-{}", item.id),
            title: item.title.clone(),
            summary: "Summary.".into(),
            kind: ChunkKind::Other,
            content: vec![
                ChunkBlock::Narrative {
                    markdown: "What to look for.".into(),
                },
                ChunkBlock::Explainer(Explainer::Claim {
                    claim: claim.into(),
                    result: None,
                    share: None,
                    how: "This item delivers it.".into(),
                }),
                ChunkBlock::Changes {
                    quotes: item.covers.clone(),
                    presentation: ChangePresentation::AddedCode,
                },
            ],
            context: None,
            confidence: None,
            oversized_reason: None,
            section: None,
        }],
    };
    artifacts::write(fixture.runner.store.expansion_path(), &expansion).expect("write expansion");
}

/// Found by dogfooding the explainers: a claim must be refused when there is
/// no description to quote, and accepted once the description holds it.
#[test]
fn a_claim_needs_the_authors_description_to_quote() {
    let fixture = fixture();
    let (_, awaiting) = fixture.append_first();
    assert!(
        awaiting.starts_with("# Gewu harness: waiting for the reviewer"),
        "{awaiting}"
    );
    fixture.record_reviewed("chunk-item-0");
    fixture.runner.next().expect("gate opens");

    stage_claim(&fixture, "Lookups no longer copy each word.");
    let repair = fixture.runner.next().expect("validate");
    assert!(
        repair.contains("no author description to quote"),
        "{repair}"
    );

    std::fs::write(
        fixture
            .runner
            .input_path
            .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        "# Faster lookups\n\nLookups no longer copy\neach word.\n",
    )
    .expect("description");
    stage_claim(&fixture, "Lookups no longer copy each word.");
    let accepted = fixture.runner.next().expect("validate");
    assert!(
        accepted.starts_with("# Gewu harness: waiting for the reviewer")
            || accepted.starts_with("# COMPLETE"),
        "{accepted}"
    );
    assert!(
        accepted.contains("Accepted `expansion.json`: 1 chunk for `item-1`."),
        "the agent is told its checkpoint went through: {accepted}"
    );
}

/// Found by running Cursor on #276: with every section's purpose naming the
/// claim it delivered, the draft still had no explainer, so measuring a draft
/// reminds the agent when the description makes claims.
#[test]
fn measuring_a_draft_without_explainers_reminds_when_claims_exist() {
    let fixture = fixture();
    let list = fixture.reach_expand();
    fixture.stage_expansion(&list.items[0]);
    let quiet = fixture.runner.status().expect("measure");
    assert!(
        !quiet.contains("No chunk has an explainer"),
        "no description, no reminder"
    );

    std::fs::write(
        fixture
            .runner
            .input_path
            .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        "Twice as fast.\n",
    )
    .expect("description");
    let reminded = fixture.runner.status().expect("measure");
    assert!(reminded.contains("No chunk has an explainer"), "{reminded}");
    // Found by QA of the #276 rerun: a reminder naming only layouts, cost
    // models, and timelines cost the walkthrough every stepper it had.
    assert!(reminded.contains("`stepper`"), "{reminded}");
}

/// Found by QA of Cursor's #276 run: explainers that were all claims, and no
/// section that stated its share of a measured result.
#[test]
fn measuring_reminds_when_explainers_only_quote_or_no_share_is_stated() {
    let fixture = fixture();
    let list = fixture.reach_expand();
    std::fs::write(
        fixture
            .runner
            .input_path
            .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        "Peak memory falls by 61% (940 → 367 MiB).\n",
    )
    .expect("description");
    let _ = list;
    stage_claim(&fixture, "Peak memory falls by 61%");
    let reminded = fixture.runner.status().expect("measure");
    assert!(
        reminded.contains("Every explainer here is a `claim`"),
        "{reminded}"
    );
    assert!(reminded.contains("no chunk states its share"), "{reminded}");
    // Found by QA of the #276 rerun: shares filled with the new code's own
    // cost, and with results the item only enables.
    assert!(
        reminded.contains("never what its own code costs"),
        "{reminded}"
    );
}

/// Found by QA of Cursor's #276 run: the reminder fired on test-only items,
/// inviting filler claims where nothing in the description is delivered.
#[test]
fn measuring_a_test_only_draft_does_not_remind() {
    let fixture = fixture();
    let list = fixture.reach_expand();
    std::fs::write(
        fixture
            .runner
            .input_path
            .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        "Peak memory falls by 61% (940 → 367 MiB).\n",
    )
    .expect("description");
    fixture.stage_expansion(&list.items[0]);
    let path = fixture.runner.store.expansion_path();
    let mut expansion: Expansion = artifacts::read(&path).expect("read expansion");
    for chunk in &mut expansion.chunks {
        chunk.kind = ChunkKind::Test;
    }
    artifacts::write(&path, &expansion).expect("write expansion");
    let measured = fixture.runner.status().expect("measure");
    assert!(
        !measured.contains("No chunk has an explainer"),
        "{measured}"
    );
}

/// Found by QA of two Cursor reruns of #276: once drafts had layouts, no
/// reminder named steppers, and none was drawn in sixteen sections.
#[test]
fn measuring_reminds_until_some_section_steps_through_code() {
    let fixture = fixture();
    let list = fixture.reach_expand();
    std::fs::write(
        fixture
            .runner
            .input_path
            .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        "Twice as fast.\n",
    )
    .expect("description");
    let _ = list;
    stage_claim(&fixture, "Twice as fast");
    let path = fixture.runner.store.expansion_path();
    let mut expansion: Expansion = artifacts::read(&path).expect("read expansion");
    let stepper = ChunkBlock::Explainer(Explainer::Stepper {
        title: "One probe".into(),
        language: None,
        source: None,
        start_line: None,
        steps: vec![gewu_review::explainer::Step {
            caption: "Hash the key.".into(),
            code: Some("let slot = hash(key);".into()),
            highlight: None,
            state: Vec::new(),
        }],
    });
    artifacts::write(&path, &expansion).expect("write expansion");
    let reminded = fixture.runner.status().expect("measure");
    assert!(reminded.contains("show it with a `stepper`"), "{reminded}");

    expansion.chunks[0].content.push(stepper);
    artifacts::write(&path, &expansion).expect("write expansion");
    let quiet = fixture.runner.status().expect("measure");
    assert!(!quiet.contains("show it with a `stepper`"), "{quiet}");
}
