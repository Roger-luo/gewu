use gewu_review::{
    ReviewChunk,
    harness::{ChunkProofread, HARNESS_SCHEMA_VERSION, HarnessStage, Proofread, Revision},
};

use crate::{
    artifacts,
    runner::tests::{Fixture, fixture},
};

const CHUNK: &str = "chunk-item-0";

fn proofreading(rounds: u32) -> Fixture {
    let mut fixture = fixture();
    fixture.runner.proofread_rounds = rounds;
    let (_, next) = fixture.append_first();
    assert!(
        next.starts_with("# Gewu harness: proofread 1 chunk(s)"),
        "{next}"
    );
    assert!(
        next.contains(CHUNK),
        "the proofreader sees the chunk: {next}"
    );
    fixture
}

impl Fixture {
    fn score(&self, score: u8) {
        let problems = if score < Proofread::MINIMUM {
            vec!["Show the old `PauliWord` struct and name its dropped fields.".to_owned()]
        } else {
            vec![]
        };
        let proofread = Proofread {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: self.input.source.clone(),
            chunks: vec![ChunkProofread {
                id: CHUNK.into(),
                scores: Proofread::DIMENSIONS
                    .iter()
                    .map(|name| ((*name).to_owned(), score))
                    .collect(),
                problems,
            }],
        };
        artifacts::write(self.runner.store.proofread_path(), &proofread).expect("write proofread");
    }

    fn rewrite(&self) {
        let chunk = self.spec().chunks.remove(0);
        let revision = Revision {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: self.input.source.clone(),
            chunks: vec![ReviewChunk {
                summary: "Rewritten with the old definition beside the new one.".into(),
                section: None,
                ..chunk
            }],
        };
        artifacts::write(self.runner.store.revision_path(), &revision).expect("write revision");
    }

    fn stage(&self) -> HarnessStage {
        self.runner
            .store
            .state()
            .expect("state")
            .expect("run")
            .stage
    }
}

#[test]
fn a_batch_that_passes_goes_to_the_reviewer() {
    let fixture = proofreading(2);
    fixture.score(3);
    let next = fixture.runner.next().expect("proofread");
    assert!(
        next.starts_with("# Gewu harness: waiting for the reviewer"),
        "{next}"
    );
    assert!(!fixture.runner.store.proofread_path().exists(), "consumed");
}

#[test]
fn a_low_score_is_rewritten_and_read_again() {
    let fixture = proofreading(2);
    fixture.score(1);
    let polish = fixture.runner.next().expect("proofread");
    assert!(
        polish.starts_with("# Gewu harness: polish 1 chunk(s)"),
        "{polish}"
    );
    assert!(
        polish.contains("`PauliWord`"),
        "the problem is the brief: {polish}"
    );
    assert!(polish.contains("rewrite 1 of 2"), "{polish}");

    fixture.rewrite();
    let again = fixture.runner.next().expect("polish");
    assert!(
        again.starts_with("# Gewu harness: proofread 1 chunk(s)"),
        "{again}"
    );
    assert!(again.contains("rewritten 1 of 2"), "{again}");
    let spec = fixture.spec();
    assert!(spec.chunks[0].summary.starts_with("Rewritten"));
    let log = std::fs::read_to_string(fixture.runner.store.verdicts_path()).expect("log");
    let first: serde_json::Value =
        serde_json::from_str(log.lines().next().expect("a line")).expect("json");
    assert_eq!(first["rewrites"], 0);
    assert!(
        !first["chunks"][0]["summary"]
            .as_str()
            .expect("summary")
            .starts_with("Rewritten"),
        "the log keeps the chunk the proofreader read, not its rewrite"
    );
    assert_eq!(
        spec.chunks[0].section.as_deref(),
        Some("Item 0"),
        "a rewrite keeps the chunk's place in the walkthrough"
    );
}

#[test]
fn rewrites_stop_at_the_limit_and_the_reviewer_is_told_why() {
    let fixture = proofreading(1);
    fixture.score(0);
    fixture.runner.next().expect("proofread");
    fixture.rewrite();
    fixture.runner.next().expect("polish");
    fixture.score(1);
    let next = fixture.runner.next().expect("proofread again");
    assert!(
        next.starts_with("# Gewu harness: waiting for the reviewer"),
        "{next}"
    );
    assert!(next.contains("ran out of rewrites"), "{next}");
    assert!(next.contains(CHUNK), "{next}");
    assert_eq!(fixture.stage(), HarnessStage::Await);
}

#[test]
fn an_incomplete_proofread_is_sent_back() {
    let fixture = proofreading(1);
    let proofread = Proofread {
        schema_version: HARNESS_SCHEMA_VERSION,
        source: fixture.input.source.clone(),
        chunks: vec![],
    };
    artifacts::write(fixture.runner.store.proofread_path(), &proofread).expect("write");
    let next = fixture.runner.next().expect("proofread");
    assert!(
        next.starts_with("# Gewu harness: checkpoint needs correction"),
        "{next}"
    );
    assert!(next.contains(CHUNK), "{next}");
    assert_eq!(fixture.stage(), HarnessStage::Proofread);
}

#[test]
fn status_repeats_the_stage_prompt() {
    let fixture = proofreading(1);
    let status = fixture.runner.status().expect("status");
    assert!(status.starts_with("# Gewu harness: proofread"), "{status}");
}
