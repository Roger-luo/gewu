use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{ReviewInput, SourceIdentity};

use super::{HARNESS_SCHEMA_VERSION, HarnessValidationError, HarnessValidationErrors};

/// A reader's scores for the chunks of one batch, against the proofread rubric.
///
/// The proofreader is a fresh agent, not the author: a walkthrough that names a
/// change without explaining it reads as complete to whoever wrote it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Proofread {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub chunks: Vec<ChunkProofread>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChunkProofread {
    pub id: String,
    /// One score from 0 to 3 for each of [`Proofread::DIMENSIONS`].
    pub scores: BTreeMap<String, u8>,
    /// What the rewrite has to fix, one entry per problem. Every score below
    /// [`Proofread::MINIMUM`] needs at least one.
    #[serde(default)]
    pub problems: Vec<String>,
}

impl Proofread {
    /// What each chunk is scored on. The prompt defines them; keep both in step.
    pub const DIMENSIONS: [&'static str; 7] = [
        "grounded",
        "change_framing",
        "rationale",
        "representation_contrast",
        "showing",
        "minimality",
        "connection",
    ];
    /// The lowest score a chunk may keep on any dimension.
    pub const MINIMUM: u8 = 2;
    const MAXIMUM: u8 = 3;

    /// Checks that exactly the batch's chunks are scored on every dimension,
    /// and that a failing score says what to fix.
    pub fn validate(
        &self,
        input: &ReviewInput,
        batch: &[String],
    ) -> Result<(), HarnessValidationErrors> {
        let mut errors = Vec::new();
        if self.schema_version != HARNESS_SCHEMA_VERSION {
            errors.push(HarnessValidationError::Schema(self.schema_version));
        }
        if self.source != input.source {
            errors.push(HarnessValidationError::StaleSource);
        }
        errors.extend(
            batch
                .iter()
                .filter(|id| !self.chunks.iter().any(|chunk| &chunk.id == *id))
                .map(|id| HarnessValidationError::MissingProofread(id.clone())),
        );
        for chunk in &self.chunks {
            if !batch.contains(&chunk.id) {
                errors.push(HarnessValidationError::UnexpectedProofread(
                    chunk.id.clone(),
                ));
                continue;
            }
            errors.extend(chunk.problems_with_scores());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(HarnessValidationErrors(errors))
        }
    }

    /// Chunks scored below the minimum on some dimension.
    pub fn failing(&self) -> impl Iterator<Item = &ChunkProofread> {
        self.chunks.iter().filter(|chunk| !chunk.passes())
    }
}

impl ChunkProofread {
    #[must_use]
    pub fn passes(&self) -> bool {
        self.low_scores().next().is_none()
    }

    /// The dimensions this chunk scored below the minimum on, with the score.
    pub fn low_scores(&self) -> impl Iterator<Item = (&str, u8)> {
        self.scores
            .iter()
            .filter(|(_, score)| **score < Proofread::MINIMUM)
            .map(|(name, score)| (name.as_str(), *score))
    }

    fn problems_with_scores(&self) -> Vec<HarnessValidationError> {
        let mut errors: Vec<_> = Proofread::DIMENSIONS
            .iter()
            .filter(|name| !self.scores.contains_key(**name))
            .map(|name| HarnessValidationError::MissingScore {
                chunk: self.id.clone(),
                dimension: (*name).to_owned(),
            })
            .collect();
        for (name, score) in &self.scores {
            if !Proofread::DIMENSIONS.contains(&name.as_str()) {
                errors.push(HarnessValidationError::UnknownDimension {
                    chunk: self.id.clone(),
                    dimension: name.clone(),
                });
            } else if *score > Proofread::MAXIMUM {
                errors.push(HarnessValidationError::InvalidScore {
                    chunk: self.id.clone(),
                    dimension: name.clone(),
                    score: *score,
                });
            }
        }
        if !self.passes()
            && self
                .problems
                .iter()
                .all(|problem| problem.trim().is_empty())
        {
            errors.push(HarnessValidationError::UnexplainedScore(self.id.clone()));
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scored(id: &str, score: u8, problems: &[&str]) -> ChunkProofread {
        ChunkProofread {
            id: id.into(),
            scores: Proofread::DIMENSIONS
                .iter()
                .map(|name| ((*name).to_owned(), score))
                .collect(),
            problems: problems.iter().map(|&problem| problem.to_owned()).collect(),
        }
    }

    fn proofread(input: &ReviewInput, chunks: Vec<ChunkProofread>) -> Proofread {
        Proofread {
            schema_version: HARNESS_SCHEMA_VERSION,
            source: input.source.clone(),
            chunks,
        }
    }

    fn input() -> ReviewInput {
        ReviewInput::new(SourceIdentity::new("base", "head", b"diff"), vec![])
    }

    #[test]
    fn every_chunk_of_the_batch_is_scored_on_every_dimension() {
        let input = input();
        let batch = ["a".to_owned(), "b".to_owned()];
        let mut partial = scored("a", 3, &[]);
        partial.scores.remove("rationale");
        let errors = proofread(&input, vec![partial, scored("c", 3, &[])])
            .validate(&input, &batch)
            .unwrap_err()
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(errors.contains("`b`"), "{errors}");
        assert!(errors.contains("rationale"), "{errors}");
        assert!(errors.contains("`c`"), "{errors}");
    }

    /// The `Word` walkthrough named a compact representation, and a fresh
    /// reader could not tell from it what was dropped or why. A low score has
    /// to carry that, or the rewrite has nothing to answer.
    #[test]
    fn a_low_score_must_say_what_to_fix() {
        let input = input();
        let batch = ["a".to_owned()];
        let silent = proofread(&input, vec![scored("a", 1, &[])]);
        assert!(silent.validate(&input, &batch).is_err());
        let said = proofread(
            &input,
            vec![scored(
                "a",
                1,
                &["the old `PauliWord` fields are not shown"],
            )],
        );
        assert!(said.validate(&input, &batch).is_ok());
        assert_eq!(said.failing().count(), 1);
        assert!(
            proofread(&input, vec![scored("a", 2, &[])])
                .failing()
                .next()
                .is_none()
        );
    }
}
