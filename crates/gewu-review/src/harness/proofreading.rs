use serde::{Deserialize, Serialize};

use super::{ChunkProofread, HarnessStage, HarnessState};

/// One batch on its way through proofreading.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Proofreading {
    pub chunks: Vec<String>,
    pub rewrites: u32,
    /// The scores the polish stage owes an answer to; empty while proofreading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<ChunkProofread>,
}

impl Proofreading {
    /// The chunks the polish stage rewrites.
    #[must_use]
    pub fn failing_ids(&self) -> Vec<String> {
        self.findings
            .iter()
            .map(|finding| finding.id.clone())
            .collect()
    }
}

impl HarnessState {
    /// Writes one batch ahead of the reviewer when `on`.
    #[must_use]
    pub fn writing_ahead(mut self, on: bool) -> Self {
        self.write_ahead = on;
        self
    }

    /// Proofreads every batch, allowing up to `rounds` rewrites of each.
    #[must_use]
    pub fn proofread(mut self, rounds: u32) -> Self {
        self.proofread_rounds = rounds;
        self
    }

    /// Enters proofreading for a newly appended batch, when it is enabled.
    /// Returns whether it was.
    pub fn begin_proofread(&mut self, chunks: Vec<String>) -> bool {
        if self.proofread_rounds == 0 || chunks.is_empty() {
            return false;
        }
        self.advance(HarnessStage::Proofread);
        self.proofreading = Some(Proofreading {
            chunks,
            ..Proofreading::default()
        });
        true
    }

    /// Whether the batch may be rewritten once more before it goes to the
    /// reviewer as it stands.
    #[must_use]
    pub fn may_polish(&self) -> bool {
        self.proofreading
            .as_ref()
            .is_some_and(|batch| batch.rewrites < self.proofread_rounds)
    }

    /// Sends the chunks that scored too low to be rewritten.
    pub fn polish(&mut self, findings: Vec<ChunkProofread>) {
        self.advance(HarnessStage::Polish);
        if let Some(batch) = self.proofreading.as_mut() {
            batch.findings = findings;
        }
    }

    /// Counts a rewrite and returns to proofreading.
    pub fn polished(&mut self) {
        self.advance(HarnessStage::Proofread);
        if let Some(batch) = self.proofreading.as_mut() {
            batch.rewrites += 1;
            batch.findings.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SourceIdentity, harness::ReviewOrigin};

    fn state(rounds: u32) -> HarnessState {
        HarnessState::new(
            "review",
            SourceIdentity::new("base", "head", b"diff"),
            ReviewOrigin {
                base: "HEAD".into(),
                head: None,
                working_tree: true,
            },
        )
        .proofread(rounds)
    }

    #[test]
    fn rewrites_stop_at_the_round_limit() {
        let mut state = state(1);
        assert!(state.begin_proofread(vec!["a".into()]));
        assert!(state.may_polish());
        state.polish(vec![]);
        assert_eq!(state.stage, HarnessStage::Polish);
        state.polished();
        assert_eq!(state.stage, HarnessStage::Proofread);
        assert!(!state.may_polish(), "one round was allowed and spent");
        state.advance(HarnessStage::Await);
        assert!(state.proofreading.is_none());
    }

    #[test]
    fn zero_rounds_skip_proofreading() {
        let mut state = state(0);
        assert!(!state.begin_proofread(vec!["a".into()]));
        assert_eq!(state.stage, HarnessStage::Investigate);
    }
}
