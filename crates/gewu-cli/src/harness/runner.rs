use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use gewu_review::{
    EventLog, Handover, Overview, ReviewChunk, ReviewInput, ReviewSession, ReviewSpec,
    ValidatedReview,
    harness::{
        HARNESS_SCHEMA_VERSION, HarnessStage, HarnessState, HarnessStore, ReviewOrigin, Verdict,
        WorkItem, WorkList, next_step,
    },
};

use super::{
    artifacts::{self, INPUT_PATH, REVIEWS_ROOT, SPEC_PATH, STATE_ROOT},
    prompts,
};

/// The persisted run, loaded once per command.
struct Run {
    state: HarnessState,
    input: ReviewInput,
}

pub struct HarnessRunner {
    store: HarnessStore,
    input_path: PathBuf,
    spec_path: PathBuf,
    reviews_root: PathBuf,
    max_chunks: usize,
    force: bool,
}

impl HarnessRunner {
    #[must_use]
    pub fn new(max_chunks: usize, force: bool) -> Self {
        Self {
            store: HarnessStore::new(STATE_ROOT),
            input_path: INPUT_PATH.into(),
            spec_path: SPEC_PATH.into(),
            reviews_root: REVIEWS_ROOT.into(),
            max_chunks,
            force,
        }
    }

    #[cfg(test)]
    fn at(root: &Path) -> Self {
        Self {
            store: HarnessStore::new(root.join("harness")),
            input_path: root.join("review-input.json"),
            spec_path: root.join("review-spec.json"),
            reviews_root: root.join("reviews"),
            max_chunks: super::DEFAULT_MAX_CHUNKS,
            force: false,
        }
    }

    pub fn start(
        &self,
        review_id: String,
        input: ReviewInput,
        origin: ReviewOrigin,
    ) -> Result<String> {
        ensure!(
            self.store.state()?.is_none(),
            "a harness run already exists; use `gewu harness status` or `gewu harness reset`"
        );
        ensure!(
            !input.changes.is_empty(),
            "the requested range contains no reviewable changes"
        );
        artifacts::write(&self.input_path, &input)?;
        artifacts::remove(&self.spec_path)?;
        self.store
            .save_state(&HarnessState::new(review_id, input.source, origin))?;
        Ok(prompts::investigate())
    }

    pub fn next(&self) -> Result<String> {
        let mut run = self.run()?;
        match run.state.stage {
            HarnessStage::Investigate => self.finish_investigation(&mut run),
            HarnessStage::Plan => self.finish_plan(&mut run),
            HarnessStage::Expand | HarnessStage::Repair => self.finish_expansion(&mut run),
            HarnessStage::Revise => self.finish_revision(&mut run),
            HarnessStage::Await => self.evaluate(&mut run),
            HarnessStage::Complete => Ok(prompts::complete(&run.state.review_id)),
        }
    }

    pub fn status(&self) -> Result<String> {
        let run = self.run()?;
        match run.state.stage {
            HarnessStage::Investigate => Ok(prompts::investigate()),
            HarnessStage::Plan => Ok(prompts::plan(run.input.max_chunk_lines, self.max_chunks)),
            HarnessStage::Expand | HarnessStage::Repair => {
                let prompt = self.prompt_expand(&run, &self.work_list()?)?;
                // Measuring a draft should not require failing first.
                match self.store.expansion() {
                    Ok(Some(expansion)) => Ok(format!(
                        "{}\nRevise `.gewu/harness/expansion.json` if needed, measure again, then \
                         run `gewu harness next`.\n",
                        prompts::measured(&expansion.chunks, &run.input)
                    )),
                    _ => Ok(prompt),
                }
            }
            // Measuring is an inner-loop action, and the revise prompt carries
            // every chunk it asked about. Once a draft exists, the heights are
            // the answer; re-printing what the agent already has buries them.
            HarnessStage::Revise => match self.store.revision() {
                Ok(Some(revision)) => Ok(format!(
                    "{}\nRewrite `.gewu/harness/revision.json` until every chunk fits, then run \
                     `gewu harness next`.\n",
                    prompts::measured(&revision.chunks, &run.input)
                )),
                _ => self.prompt_revise(&run, &run.state.revising.clone()),
            },
            // What the reviewer has done since decides what is being waited
            // for, so the verdict is recomputed — without acting on it, which
            // is what `next` is for.
            HarnessStage::Await => match self.verdict(&run)? {
                Verdict::Handover { feedback_ids } => self.prompt_handover(&run, &feedback_ids),
                Verdict::Revise { chunk_ids } => self.prompt_revise(&run, &chunk_ids),
                _ => Ok(prompts::awaiting(
                    &self.spec()?,
                    self.work_list()?.remaining(),
                )),
            },
            HarnessStage::Complete => Ok(prompts::complete(&run.state.review_id)),
        }
    }

    pub fn reset(&self) -> Result<String> {
        self.store.reset()?;
        artifacts::remove(&self.input_path)?;
        artifacts::remove(&self.spec_path)?;
        if self.reviews_root.exists() {
            std::fs::remove_dir_all(&self.reviews_root)?;
        }
        Ok("# Gewu harness reset\n\nStart a new run with `gewu harness start`.\n".into())
    }

    fn finish_investigation(&self, run: &mut Run) -> Result<String> {
        let checkpoint = match self.store.investigation() {
            Ok(Some(checkpoint)) => checkpoint,
            Ok(None) => return Ok(prompts::investigate()),
            Err(error) => {
                return Ok(prompts::checkpoint_retry(
                    ".gewu/harness/investigation.json",
                    &[error.to_string()],
                ));
            }
        };
        if let Err(errors) = checkpoint.validate(&run.input) {
            return Ok(prompts::checkpoint_retry(
                ".gewu/harness/investigation.json",
                &error_strings(errors.0),
            ));
        }
        self.advance(&mut run.state, HarnessStage::Plan)?;
        Ok(prompts::plan(run.input.max_chunk_lines, self.max_chunks))
    }

    fn finish_plan(&self, run: &mut Run) -> Result<String> {
        let list = match self.store.work_list() {
            Ok(Some(list)) => list,
            Ok(None) => return Ok(prompts::plan(run.input.max_chunk_lines, self.max_chunks)),
            Err(error) => {
                return Ok(prompts::checkpoint_retry(
                    ".gewu/harness/worklist.json",
                    &[error.to_string()],
                ));
            }
        };
        if let Err(errors) = list.validate_for(&run.input, self.max_chunks) {
            return Ok(prompts::checkpoint_retry(
                ".gewu/harness/worklist.json",
                &error_strings(errors.0),
            ));
        }
        self.advance(&mut run.state, HarnessStage::Expand)?;
        self.prompt_expand(run, &list)
    }

    /// Validates the pending expansion and appends its chunks to the spec.
    fn finish_expansion(&self, run: &mut Run) -> Result<String> {
        let mut work = self.work_list()?;
        let Some(item) = work.next_item().cloned() else {
            self.advance(&mut run.state, HarnessStage::Complete)?;
            return Ok(prompts::complete(&run.state.review_id));
        };
        let expansion = match self.store.expansion() {
            Ok(Some(expansion)) => expansion,
            Ok(None) => return self.prompt_expand(run, &work),
            Err(error) => return self.repair(run, &item, &[error.to_string()]),
        };
        if expansion.item_id != item.id {
            let mismatch = format!(
                "expansion names item `{}`, but the next item is `{}`",
                expansion.item_id, item.id
            );
            return self.repair(run, &item, &[mismatch]);
        }
        if let Err(errors) = expansion.validate(&run.input, &item, self.max_chunks) {
            let mut reported = error_strings(errors.0);
            reported.push(prompts::measured(&expansion.chunks, &run.input));
            return self.repair(run, &item, &reported);
        }
        let mut spec = self.spec_so_far(run)?;
        spec.chunks.extend(expansion.chunks);
        work.mark_expanded(&item.id);
        spec = spec.with_remaining_work(work.remaining());
        if let Err(errors) = ValidatedReview::validate_partial(run.input.clone(), spec.clone()) {
            return self.repair(run, &item, &error_strings(errors.0));
        }
        artifacts::write(&self.spec_path, &spec)?;
        self.store.save_work_list(&work)?;
        self.store.discard_expansion()?;
        self.advance(&mut run.state, HarnessStage::Await)?;
        Ok(prompts::awaiting(&spec, work.remaining()))
    }

    /// Applies the continue gates once the reviewer has had the current batch.
    fn evaluate(&self, run: &mut Run) -> Result<String> {
        let work = self.work_list()?;
        match self.verdict(run)? {
            Verdict::Wait { unreviewed } => Ok(prompts::waiting(unreviewed)),
            Verdict::Complete => {
                self.advance(&mut run.state, HarnessStage::Complete)?;
                Ok(prompts::complete(&run.state.review_id))
            }
            Verdict::Expand => {
                self.advance(&mut run.state, HarnessStage::Expand)?;
                self.prompt_expand(run, &work)
            }
            Verdict::Replan { stale_chunks } => {
                let current = self.current_manifest(run)?;
                self.replan(run, current, &stale_chunks)
            }
            // Nothing to advance into: the source is not Gewu's to edit, and the
            // gate stays shut until the reviewer's requests are resolved.
            Verdict::Handover { feedback_ids } => self.prompt_handover(run, &feedback_ids),
            Verdict::Revise { chunk_ids } => {
                run.state.revise(chunk_ids.clone());
                self.store.save_state(&run.state)?;
                self.prompt_revise(run, &chunk_ids)
            }
        }
    }

    /// What the reviewer's session and the code on disk say should happen next.
    fn verdict(&self, run: &Run) -> Result<Verdict> {
        let spec = self.spec()?;
        let work = self.work_list()?;
        let session = self.session(&spec)?;
        let current = self.current_manifest(run)?;
        Ok(next_step(
            &spec, &session, &work, &current, &run.input, self.force,
        ))
    }

    /// Replaces the chunks the reviewer objected to with their rewrites.
    ///
    /// What was asked for is re-checked first, against both the reviewer and
    /// the tree. They may resolve or withdraw a request while an agent is still
    /// writing the rewrite, and the code underneath may move; a stage that only
    /// ever waited for a file would hold a review shut over work nobody wants,
    /// or accept a rewrite of a chunk it is about to drop.
    fn finish_revision(&self, run: &mut Run) -> Result<String> {
        let mut spec = self.spec()?;
        let session = self.session(&spec)?;
        let still_wanted = gewu_review::handover::chunks_to_revise(&spec.chunks, &session);
        let current = self.current_manifest(run)?;
        let carried = current.unchanged_since(&run.input);
        let settled = |id: &String| {
            spec.chunks
                .iter()
                .find(|chunk| &chunk.id == id)
                .is_some_and(|chunk| chunk.change_ids().all(|id| carried.contains(id)))
        };
        let wanted: Vec<String> = run
            .state
            .revising
            .iter()
            .filter(|id| still_wanted.contains(id) && settled(id))
            .cloned()
            .collect();
        if wanted.is_empty() {
            self.store.discard_revision()?;
            self.advance(&mut run.state, HarnessStage::Await)?;
            return self.evaluate(run);
        }
        if wanted != run.state.revising {
            run.state.revising.clone_from(&wanted);
            self.store.save_state(&run.state)?;
        }
        let originals: Vec<&ReviewChunk> = spec
            .chunks
            .iter()
            .filter(|chunk| wanted.contains(&chunk.id))
            .collect();
        let revision = match self.store.revision() {
            Ok(Some(revision)) => revision,
            Ok(None) => return self.prompt_revise(run, &wanted),
            Err(error) => return self.retry_revision(run, &wanted, &[error.to_string()]),
        };
        if let Err(errors) = revision.validate(&run.input, &originals) {
            return self.retry_revision(run, &wanted, &error_strings(errors.0));
        }
        let mut rewritten = revision.chunks;
        for chunk in &mut spec.chunks {
            if let Some(position) = rewritten.iter().position(|other| other.id == chunk.id) {
                *chunk = rewritten.remove(position);
            }
        }
        if let Err(errors) = ValidatedReview::validate_partial(run.input.clone(), spec.clone()) {
            return self.retry_revision(run, &wanted, &error_strings(errors.0));
        }
        artifacts::write(&self.spec_path, &spec)?;
        self.store.discard_revision()?;
        self.advance(&mut run.state, HarnessStage::Await)?;
        Ok(prompts::revised(&wanted))
    }

    fn retry_revision(&self, run: &Run, wanted: &[String], errors: &[String]) -> Result<String> {
        Ok(prompts::retry(&self.prompt_revise(run, wanted)?, errors))
    }

    fn prompt_handover(&self, run: &Run, feedback_ids: &[String]) -> Result<String> {
        let spec = self.spec()?;
        let session = self.session(&spec)?;
        let review = ValidatedReview::validate_partial(run.input.clone(), spec)
            .map_err(|errors| anyhow::anyhow!("{errors}"))?;
        let handover = Handover::assemble(&review, &session);
        Ok(prompts::handover(&handover, feedback_ids))
    }

    fn prompt_revise(&self, run: &Run, chunk_ids: &[String]) -> Result<String> {
        let spec = self.spec()?;
        let session = self.session(&spec)?;
        let chunks: Vec<&ReviewChunk> = spec
            .chunks
            .iter()
            .filter(|chunk| chunk_ids.contains(&chunk.id))
            .collect();
        Ok(prompts::revise(
            &chunks,
            &session,
            run.input.max_chunk_lines,
        ))
    }

    /// Keeps the review work that survived the reviewer's edits and asks for a
    /// fresh investigation of everything else.
    fn replan(&self, run: &mut Run, current: ReviewInput, stale: &[String]) -> Result<String> {
        let mut spec = self.spec()?;
        spec.chunks.retain(|chunk| !stale.contains(&chunk.id));
        spec.source = current.source.clone();
        let covered: Vec<String> = spec
            .chunks
            .iter()
            .flat_map(|chunk| chunk.change_ids().cloned())
            .collect();
        let mut work = self.work_list()?;
        work.retain_expanded();
        work.items
            .retain(|item| item.change_ids().all(|id| covered.contains(id)));
        work.source = current.source.clone();
        let remainder = current
            .changes
            .iter()
            .filter(|change| !covered.contains(&change.id))
            .count();
        artifacts::write(&self.input_path, &current)?;
        artifacts::write(&self.spec_path, &spec)?;
        self.store.save_work_list(&work)?;
        self.store.discard_expansion()?;
        run.state.source = current.source.clone();
        run.input = current;
        self.advance(&mut run.state, HarnessStage::Investigate)?;
        Ok(prompts::replan(stale, remainder))
    }

    fn prompt_expand(&self, run: &Run, work: &WorkList) -> Result<String> {
        match work.next_item() {
            Some(item) => {
                let investigation = self.store.investigation()?;
                Ok(prompts::expand(
                    item,
                    &run.input,
                    investigation.as_ref(),
                    run.input.max_chunk_lines,
                    self.max_chunks,
                ))
            }
            None => Ok(prompts::complete(&run.state.review_id)),
        }
    }

    fn repair(&self, run: &mut Run, item: &WorkItem, errors: &[String]) -> Result<String> {
        if run.state.stage != HarnessStage::Repair {
            self.advance(&mut run.state, HarnessStage::Repair)?;
        }
        let investigation = self.store.investigation()?;
        Ok(prompts::retry(
            &prompts::expand(
                item,
                &run.input,
                investigation.as_ref(),
                run.input.max_chunk_lines,
                self.max_chunks,
            ),
            errors,
        ))
    }

    fn advance(&self, state: &mut HarnessState, stage: HarnessStage) -> Result<()> {
        state.advance(stage);
        self.store.save_state(state)?;
        Ok(())
    }

    fn spec(&self) -> Result<ReviewSpec> {
        artifacts::read(&self.spec_path)
    }

    /// The spec built so far, or an empty one carrying the run's identity.
    ///
    /// The overview is refreshed from the current investigation every time, so
    /// a re-planned run does not keep describing the change set it started
    /// from.
    fn spec_so_far(&self, run: &Run) -> Result<ReviewSpec> {
        if self.spec_path.exists() {
            let mut spec = self.spec()?;
            if let Some(checkpoint) = self.store.investigation()? {
                spec.overview.intent = checkpoint.intent;
                spec.overview.changes = checkpoint.changes;
                spec.overview.concepts = checkpoint.concepts;
            }
            if let Ok(work) = self.work_list() {
                spec.generation = Some(gewu_review::GenerationStatus {
                    remaining_work_items: work.remaining(),
                });
            }
            return Ok(spec);
        }
        let overview = self.store.investigation()?.map_or_else(
            || Overview {
                intent: "Review the selected changes.".to_owned(),
                changes: vec![],
                illustration: None,
                concepts: vec![],
            },
            |checkpoint| Overview {
                intent: checkpoint.intent,
                changes: checkpoint.changes,
                illustration: None,
                concepts: checkpoint.concepts,
            },
        );
        let remaining = self.work_list().map_or(0, |work| work.remaining());
        Ok(ReviewSpec::new(run.input.source.clone(), overview, vec![])
            .for_review(run.state.review_id.clone())
            .with_remaining_work(remaining))
    }

    fn work_list(&self) -> Result<WorkList> {
        self.store
            .work_list()?
            .context("work list is missing; the plan stage has not completed")
    }

    fn session(&self, spec: &ReviewSpec) -> Result<ReviewSession> {
        Ok(EventLog::new(
            self.reviews_root
                .join(spec.session_key())
                .join("events.jsonl"),
        )
        .load()?)
    }

    /// The manifest as it stands now. A fixed commit range cannot move, so only
    /// a working-tree review is re-diffed to see the reviewer's edits.
    fn current_manifest(&self, run: &Run) -> Result<ReviewInput> {
        if !run.state.origin.working_tree {
            return Ok(run.input.clone());
        }
        crate::git::Repository::new(Path::new("."))
            .worktree_input(&run.state.origin.base, run.input.max_chunk_lines)
    }

    fn run(&self) -> Result<Run> {
        let state = self
            .store
            .state()?
            .context("no harness run; use `gewu harness start`")?;
        ensure!(
            state.schema_version == HARNESS_SCHEMA_VERSION,
            "unsupported harness state version {}",
            state.schema_version
        );
        let input: ReviewInput = artifacts::read(&self.input_path)?;
        ensure!(
            state.source == input.source,
            "harness state does not match review input; reset and restart"
        );
        Ok(Run { state, input })
    }
}

fn error_strings(errors: Vec<impl ToString>) -> Vec<String> {
    errors.into_iter().map(|error| error.to_string()).collect()
}

#[cfg(test)]
mod tests;
