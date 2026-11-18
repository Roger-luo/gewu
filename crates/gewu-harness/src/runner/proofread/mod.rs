//! Proofreading: a fresh reader scores each batch against the rubric before the
//! reviewer sees it, and the chunks that score too low are rewritten — a
//! bounded number of times, so a batch the rubric never passes still reaches
//! the reviewer, with the scores that held it back.

use gewu_prompts::Prompt;
use gewu_review::{
    ReviewChunk, ValidatedReview,
    harness::{ChunkProofread, HarnessStage},
};

use super::{HarnessRunner, Run, error_strings, replace_chunks};
use crate::{Result, artifacts, prompts};

const PROOFREAD_PATH: &str = ".gewu/harness/proofread.json";

impl HarnessRunner {
    /// Sends a newly appended batch to proofreading when the run asks for it,
    /// and otherwise to the reviewer.
    /// `written` says Gewu wrote the batch itself: there is no author's prose
    /// for a fresh agent to score, so it goes straight to the reviewer. Found
    /// by reviewing the test-section change: with proofreading on, the
    /// default, agents were started to score and rewrite raw test code.
    pub(super) fn after_batch(
        &self,
        run: &mut Run,
        batch: Vec<String>,
        written: bool,
    ) -> Result<String> {
        if !written && run.state.begin_proofread(batch) {
            self.store.discard_proofread()?;
            self.store.save_state(&run.state)?;
            return self.prompt_proofread(run);
        }
        self.advance(&mut run.state, HarnessStage::Await)?;
        self.ahead_or_awaiting(run)
    }

    /// The next item's prompt when the gate lets the agent write ahead of the
    /// reviewer, and otherwise the wait.
    fn ahead_or_awaiting(&self, run: &mut Run) -> Result<String> {
        if self.verdict(run)? == gewu_review::harness::Verdict::Expand {
            return self.evaluate(run);
        }
        self.awaiting(None)
    }

    pub(super) fn prompt_proofread(&self, run: &Run) -> Result<String> {
        let batch = run.state.proofreading.clone().unwrap_or_default();
        let spec = self.spec()?;
        let chunks = named(&spec.chunks, &batch.chunks);
        let round = if batch.rewrites == 0 {
            "This is the first reading.".to_owned()
        } else {
            format!(
                "The chunks were rewritten {} of {} allowed time(s) since the first reading.",
                batch.rewrites, run.state.proofread_rounds
            )
        };
        Ok(Prompt::Proofread
            .text()
            .replace("{{count}}", &chunks.len().to_string())
            .replace("{{round}}", &round)
            .replace("{{chunks}}", &prompts::chunk_json(&chunks)))
    }

    pub(super) fn finish_proofread(&self, run: &mut Run) -> Result<String> {
        let batch = run.state.proofreading.clone().unwrap_or_default();
        let proofread = match self.store.proofread() {
            Ok(Some(proofread)) => proofread,
            Ok(None) => return self.prompt_proofread(run),
            Err(error) => {
                return Ok(prompts::checkpoint_retry(
                    PROOFREAD_PATH,
                    &[error.to_string()],
                ));
            }
        };
        if let Err(errors) = proofread.validate(&run.input, &batch.chunks) {
            return Ok(prompts::checkpoint_retry(
                PROOFREAD_PATH,
                &error_strings(errors.0),
            ));
        }
        let spec = self.spec()?;
        self.store.record_proofread(
            batch.rewrites,
            &named(&spec.chunks, &batch.chunks),
            &proofread,
        )?;
        self.store.discard_proofread()?;
        let failing: Vec<ChunkProofread> = proofread.failing().cloned().collect();
        // A proofreader stops here even when the gate would let the next
        // item be written: it reads, and an author writes.
        if failing.is_empty() {
            self.advance(&mut run.state, HarnessStage::Await)?;
            return self.awaiting(None);
        }
        if run.state.may_polish() {
            run.state.polish(failing);
            self.store.save_state(&run.state)?;
            return self.prompt_polish(run);
        }
        self.advance(&mut run.state, HarnessStage::Await)?;
        self.awaiting(Some(&failing))
    }

    pub(super) fn prompt_polish(&self, run: &Run) -> Result<String> {
        let batch = run.state.proofreading.clone().unwrap_or_default();
        let spec = self.spec()?;
        let chunks = named(&spec.chunks, &batch.failing_ids());
        let findings = batch
            .findings
            .iter()
            .map(|finding| {
                let title = chunks
                    .iter()
                    .find(|chunk| chunk.id == finding.id)
                    .map_or("", |chunk| chunk.title.as_str());
                format!("## `{}` — {title}\n\n{}", finding.id, scored(finding))
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let round = format!(
            "This is rewrite {} of {}; after the last, the batch goes to the reviewer as it \
             stands.",
            batch.rewrites + 1,
            run.state.proofread_rounds
        );
        Ok(prompts::budgeted(&Prompt::Polish.text(), &run.input)
            .replace("{{count}}", &chunks.len().to_string())
            .replace("{{round}}", &round)
            .replace("{{findings}}", &findings)
            .replace("{{chunks}}", &prompts::chunk_json(&chunks))
            .replace("{{rubric}}", &prompts::rubric()))
    }

    /// Replaces the chunks the proofread failed with their rewrites, then
    /// sends the batch back to be read again.
    pub(super) fn finish_polish(&self, run: &mut Run) -> Result<String> {
        let ids = run
            .state
            .proofreading
            .as_ref()
            .map(gewu_review::harness::Proofreading::failing_ids)
            .unwrap_or_default();
        let mut spec = self.spec()?;
        let revision = match self.store.revision() {
            Ok(Some(revision)) => revision,
            Ok(None) => return self.prompt_polish(run),
            Err(error) => {
                return Ok(prompts::retry(
                    &self.prompt_polish(run)?,
                    &[error.to_string()],
                ));
            }
        };
        let originals = named(&spec.chunks, &ids);
        if let Err(errors) = revision.validate(&run.input, &originals) {
            return Ok(prompts::retry(
                &self.prompt_polish(run)?,
                &error_strings(errors.0),
            ));
        }
        replace_chunks(&mut spec, revision.chunks);
        if let Err(errors) = ValidatedReview::validate_partial(run.input.clone(), spec.clone()) {
            return Ok(prompts::retry(
                &self.prompt_polish(run)?,
                &error_strings(errors.0),
            ));
        }
        artifacts::write(&self.spec_path, &spec)?;
        self.store.discard_revision()?;
        run.state.polished();
        self.store.save_state(&run.state)?;
        self.prompt_proofread(run)
    }

    /// The gate prompt, noting what proofreading could not fix. The heading
    /// stays first: drivers recognise the gate by it.
    fn awaiting(&self, unresolved: Option<&[ChunkProofread]>) -> Result<String> {
        let prompt = prompts::awaiting(&self.spec()?, self.work_list()?.remaining());
        let Some(unresolved) = unresolved else {
            return Ok(prompt);
        };
        let notes = unresolved
            .iter()
            .map(|finding| format!("- `{}`: {}", finding.id, scored(finding).replace('\n', " ")))
            .collect::<Vec<_>>()
            .join("\n");
        let (heading, body) = prompt.split_once("\n\n").unwrap_or((&prompt, ""));
        Ok(format!(
            "{heading}\n\nProofreading ran out of rewrites with these chunks still below the \
             rubric; tell the reviewer, so they read them knowing what is missing:\n\n{notes}\n\n\
             {body}"
        ))
    }
}

/// The chunks with these ids, in review order.
fn named<'a>(chunks: &'a [ReviewChunk], ids: &[String]) -> Vec<&'a ReviewChunk> {
    chunks
        .iter()
        .filter(|chunk| ids.contains(&chunk.id))
        .collect()
}

/// A failing chunk's low scores and the problems the proofreader named.
fn scored(finding: &ChunkProofread) -> String {
    let low = finding
        .low_scores()
        .map(|(name, score)| format!("`{name}` {score}/3"))
        .collect::<Vec<_>>()
        .join(", ");
    let problems = finding
        .problems
        .iter()
        .map(|problem| format!("- {problem}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Scored below 2 on {low}.\n\n{problems}")
}

#[cfg(test)]
mod tests;
