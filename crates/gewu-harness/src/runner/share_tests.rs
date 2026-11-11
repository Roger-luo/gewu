//! Shares checked when a draft is measured and accepted: an amount, not the
//! whole result again, and not the same saving credited twice.

use gewu_review::{ChunkBlock, Explainer, ReviewSpec};

use super::claims_tests::stage_claim_with;
use super::tests::{Fixture, fixture};
use crate::artifacts;

const CLAIM: &str = "Peak memory falls by 61%";
const RESULT: &str = "940 → 367 MiB (−61%)";

fn describe(fixture: &Fixture) {
    std::fs::write(
        fixture
            .runner
            .input_path
            .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        "Peak memory falls by 61% (940 → 367 MiB).\n",
    )
    .expect("description");
}

/// Found by QA of Cursor's runs of #276: "Enables swapping" and "part of the
/// 18–32% speedups" were accepted as shares.
#[test]
fn a_share_must_give_this_items_part() {
    let fixture = fixture();
    let _ = fixture.reach_expand();
    describe(&fixture);

    stage_claim_with(
        &fixture,
        CLAIM,
        Some(RESULT),
        Some("Only the smaller index"),
    );
    let measured = fixture.runner.status().expect("measure");
    assert!(measured.contains("a share with no amount"), "{measured}");

    stage_claim_with(
        &fixture,
        CLAIM,
        Some(RESULT),
        Some("enables the 32-byte layout (24 MiB)"),
    );
    let measured = fixture.runner.status().expect("measure");
    assert!(
        measured.contains("says what the chunk enables"),
        "{measured}"
    );

    stage_claim_with(
        &fixture,
        CLAIM,
        Some(RESULT),
        Some("part of the 940 → 367 MiB drop"),
    );
    let measured = fixture.runner.status().expect("measure");
    assert!(measured.contains("repeats the whole result"), "{measured}");

    stage_claim_with(
        &fixture,
        CLAIM,
        Some(RESULT),
        Some("about 24 MiB of the 573 MiB drop"),
    );
    let measured = fixture.runner.status().expect("measure");
    assert!(measured.contains("The draft validates"), "{measured}");
}

/// Found by QA of Cursor's run C of #276: two sections credited the same
/// 24 MiB, and neither said so.
#[test]
fn measuring_notes_a_saving_an_earlier_section_already_credited() {
    let fixture = fixture();
    let _ = fixture.append_first();
    let mut spec: ReviewSpec = artifacts::read(&fixture.runner.spec_path).expect("spec");
    spec.chunks[0]
        .content
        .push(ChunkBlock::Explainer(Explainer::Claim {
            claim: CLAIM.into(),
            result: Some(RESULT.into()),
            share: Some("about 24 MiB of the 573 MiB drop".into()),
            how: "Each word shrinks by 16 B.".into(),
        }));
    artifacts::write(&fixture.runner.spec_path, &spec).expect("spec");
    fixture.record_reviewed("chunk-item-0");
    fixture.runner.next().expect("gate opens");
    describe(&fixture);

    stage_claim_with(&fixture, CLAIM, Some(RESULT), Some("about 24 MiB of it"));
    let measured = fixture.runner.status().expect("measure");
    assert!(
        measured.contains("an earlier section's share (24 MiB)"),
        "{measured}"
    );

    stage_claim_with(
        &fixture,
        CLAIM,
        Some(RESULT),
        Some("about 10 MiB of the 573 MiB drop"),
    );
    let measured = fixture.runner.status().expect("measure");
    assert!(
        !measured.contains("an earlier section's share"),
        "{measured}"
    );
    // Found by the same QA: no run drew a timeline.
    assert!(measured.contains("draws a `timeline`"), "{measured}");
    // Found by QA of runs E and F: savings in different phases offered
    // toward one peak.
    assert!(measured.contains("falls at the peak"), "{measured}");

    stage_claim_with(
        &fixture,
        CLAIM,
        Some(RESULT),
        Some("about 10 MiB of the 573 MiB drop, alive at the peak"),
    );
    let measured = fixture.runner.status().expect("measure");
    assert!(!measured.contains("falls at the peak"), "{measured}");
}
