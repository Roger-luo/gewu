use std::path::{Path, PathBuf};

use crate::{Error, Result};
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
    project: PathBuf,
    store: HarnessStore,
    input_path: PathBuf,
    spec_path: PathBuf,
    reviews_root: PathBuf,
    max_chunks: usize,
    force: bool,
    proofread_rounds: u32,
    /// Whether runs this runner starts write one batch ahead of the reviewer.
    write_ahead: bool,
    /// Whether runs this runner starts have the agent explain items of tests.
    explain_tests: bool,
}

impl HarnessRunner {
    /// Has runs this runner starts write items of tests with the agent
    /// (`true`, the default) or from the test code itself (`false`).
    #[must_use]
    pub fn with_explain_tests(mut self, on: bool) -> Self {
        self.explain_tests = on;
        self
    }

    /// Has runs this runner starts write one batch ahead of the reviewer, so
    /// the next is ready when they finish. Off by default: the gate holds the
    /// agent level with the reviewer.
    #[must_use]
    pub fn with_write_ahead(mut self, on: bool) -> Self {
        self.write_ahead = on;
        self
    }

    /// Proofreads each batch a run started by this runner expands, allowing up
    /// to `rounds` rewrites before the reviewer sees it. Zero, the default,
    /// hands batches straight to the reviewer.
    #[must_use]
    pub fn with_proofread(mut self, rounds: u32) -> Self {
        self.proofread_rounds = rounds;
        self
    }

    pub fn expansion_guide(&self) -> Result<String> {
        let run = self.run()?;
        if !matches!(run.state.stage, HarnessStage::Expand | HarnessStage::Repair) {
            return Err(Error::message("not expanding a work item"));
        }
        let work = self.work_list()?;
        let item = work
            .next_item()
            .ok_or_else(|| Error::message("no pending work item"))?;
        Ok(prompts::expand(item, &run.input, None, self.max_chunks))
    }

    pub fn measure_expansion(&self, path: &Path) -> Result<serde_json::Value> {
        let run = self.run()?;
        if !matches!(run.state.stage, HarnessStage::Expand | HarnessStage::Repair) {
            return Err(Error::message("not expanding a work item"));
        }
        let work = self.work_list()?;
        let item = work
            .next_item()
            .ok_or_else(|| Error::message("no pending work item"))?;
        let expansion: gewu_review::harness::Expansion = artifacts::read(path)?;
        if expansion.item_id != item.id {
            return Err(Error::message("candidate names a different work item"));
        }
        let mut errors = self
            .check_expansion(&run, &expansion, item)
            .err()
            .unwrap_or_default();
        let mut spec = self.spec_so_far(&run)?;
        spec.chunks.extend(expansion.chunks.clone());
        if let Err(invalid) = ValidatedReview::validate_partial(run.input.clone(), spec) {
            errors.extend(error_strings(invalid.0));
        }
        let known = run
            .input
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        let chunks: Vec<_> = expansion.chunks.iter().map(|chunk| serde_json::json!({
            "id": chunk.id, "rows": gewu_review::chunk_height(chunk, &known),
            "context_rows": gewu_review::pane_height(chunk), "limit": run.input.max_chunk_lines
        })).collect();
        Ok(serde_json::json!({"valid": errors.is_empty(), "errors": errors, "chunks": chunks}))
    }

    #[must_use]
    pub fn new(max_chunks: usize, force: bool) -> Self {
        Self {
            project: PathBuf::from("."),
            store: HarnessStore::new(STATE_ROOT),
            input_path: INPUT_PATH.into(),
            spec_path: SPEC_PATH.into(),
            reviews_root: REVIEWS_ROOT.into(),
            max_chunks,
            force,
            proofread_rounds: 0,
            write_ahead: false,
            explain_tests: true,
        }
    }

    /// A runner whose `.gewu/` state lives in `project` rather than the
    /// current directory, so one process can drive several checkouts.
    #[must_use]
    pub fn in_project(project: &Path, max_chunks: usize, force: bool) -> Self {
        Self {
            project: project.to_path_buf(),
            store: HarnessStore::new(project.join(STATE_ROOT)),
            input_path: project.join(INPUT_PATH),
            spec_path: project.join(SPEC_PATH),
            reviews_root: project.join(REVIEWS_ROOT),
            max_chunks,
            force,
            proofread_rounds: 0,
            write_ahead: false,
            explain_tests: true,
        }
    }

    /// An expansion's own checks, plus claims checked against the author's
    /// description beside the review input.
    fn check_expansion(
        &self,
        run: &Run,
        expansion: &gewu_review::harness::Expansion,
        item: &gewu_review::harness::WorkItem,
    ) -> std::result::Result<(), Vec<String>> {
        let mut errors = expansion
            .validate(&run.input, item, self.max_chunks)
            .err()
            .map_or_else(Vec::new, |errors| error_strings(errors.0));
        let description = std::fs::read_to_string(
            self.input_path
                .with_file_name(gewu_review::harness::DESCRIPTION_FILE),
        )
        .ok();
        errors.extend(gewu_review::explainer::unquoted_claims(
            &expansion.chunks,
            description.as_deref(),
        ));
        errors.extend(gewu_review::explainer::vague_shares(&expansion.chunks));
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[cfg(test)]
    fn at(root: &Path) -> Self {
        Self {
            project: PathBuf::from("."),
            store: HarnessStore::new(root.join("harness")),
            input_path: root.join("review-input.json"),
            spec_path: root.join("review-spec.json"),
            reviews_root: root.join("reviews"),
            max_chunks: crate::DEFAULT_MAX_CHUNKS,
            force: false,
            proofread_rounds: 0,
            write_ahead: false,
            explain_tests: true,
        }
    }

    pub fn start(
        &self,
        review_id: String,
        input: ReviewInput,
        origin: ReviewOrigin,
    ) -> Result<String> {
        if self.store.state()?.is_some() {
            return Err(Error::message(
                "a harness run already exists; use `gewu harness status` or `gewu harness reset`",
            ));
        }
        if input.changes.is_empty() {
            return Err(Error::message(
                "the requested range contains no reviewable changes",
            ));
        }
        artifacts::write(&self.input_path, &input)?;
        artifacts::remove(&self.spec_path)?;
        self.store.save_state(
            &HarnessState::new(review_id, input.source, origin)
                .proofread(self.proofread_rounds)
                .writing_ahead(self.write_ahead)
                .explaining_tests(self.explain_tests),
        )?;
        Ok(prompts::investigate())
    }

    pub fn next(&self) -> Result<String> {
        let mut run = self.run()?;
        match run.state.stage {
            HarnessStage::Investigate => self.finish_investigation(&mut run),
            HarnessStage::Plan => self.finish_plan(&mut run),
            HarnessStage::Expand | HarnessStage::Repair => self.finish_expansion(&mut run),
            HarnessStage::Revise => self.finish_revision(&mut run),
            HarnessStage::Proofread => self.finish_proofread(&mut run),
            HarnessStage::Polish => self.finish_polish(&mut run),
            HarnessStage::Await => self.evaluate(&mut run),
            HarnessStage::Complete => Ok(prompts::complete(&run.state.review_id)),
        }
    }

    /// Whether the reviewer's reading has opened the gate for the next item,
    /// so a host may start writing it without being asked. Feedback, drift,
    /// and completion are left for the reviewer to continue into.
    pub fn would_expand(&self) -> Result<bool> {
        let run = self.run()?;
        if run.state.stage != HarnessStage::Await {
            return Ok(false);
        }
        Ok(self.verdict(&run)? == Verdict::Expand)
    }

    /// The stage the run is at.
    pub fn stage(&self) -> Result<HarnessStage> {
        Ok(self.run()?.state.stage)
    }

    pub fn status(&self) -> Result<String> {
        let run = self.run()?;
        match run.state.stage {
            HarnessStage::Investigate => Ok(self.investigation_prompt(&run)),
            HarnessStage::Plan => Ok(prompts::plan(&run.input, self.max_chunks)),
            HarnessStage::Expand | HarnessStage::Repair => {
                let work = self.work_list()?;
                let prompt = self.prompt_expand(&run, &work)?;
                // Measuring a draft should not require failing first, and
                // neither should checking it: self-review authors could learn
                // whether their spans were covered only by advancing.
                match (self.store.expansion(), work.next_item()) {
                    (Ok(Some(expansion)), Some(item)) => {
                        let verdict = match self.check_expansion(&run, &expansion, item) {
                            Ok(()) => "The draft validates; `gewu harness next` will accept it.\n"
                                .to_owned(),
                            Err(errors) => format!(
                                "`gewu harness next` would reject the draft:\n\n{}\n",
                                errors
                                    .iter()
                                    .map(|error| format!("- {error}"))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            ),
                        };
                        // Advisory: a trimmed view can strand a citation, but
                        // an anchor to an earlier chunk is not a defect.
                        let cited: Vec<_> = expansion
                            .chunks
                            .iter()
                            .flat_map(|chunk| {
                                gewu_review::audit::invisible_in(chunk, &run.input.changes)
                                    .into_iter()
                                    .map(|citation| format!("- `{}` cites {citation}", chunk.id))
                            })
                            .collect();
                        let cited = if cited.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "\nCited lines no view of the chunk shows; fine for an earlier \
                                 chunk or code outside the diff, otherwise quote them:\n\n{}\n",
                                cited.join("\n")
                            )
                        };
                        let teach = self.explainer_reminder(&expansion.chunks);
                        let names = self.name_reminder(&run, &expansion.chunks);
                        let steps = self.stepper_reminder(&run, &expansion.chunks);
                        Ok(format!(
                            "{}\n{verdict}{cited}{names}{steps}{teach}\nRevise `.gewu/harness/expansion.json` if \
                             needed, measure again, then run `gewu harness next`.\n",
                            prompts::measured(&expansion.chunks, &run.input)
                        ))
                    }
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
            HarnessStage::Proofread => self.prompt_proofread(&run),
            HarnessStage::Polish => match self.store.revision() {
                Ok(Some(revision)) => Ok(format!(
                    "{}\nRewrite `.gewu/harness/revision.json` until every chunk fits, then run \
                     `gewu harness next`.\n",
                    prompts::measured(&revision.chunks, &run.input)
                )),
                _ => self.prompt_polish(&run),
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
            Ok(None) => return Ok(self.investigation_prompt(run)),
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
        if let Err(error) = crate::research_state::validate_checkpoint(
            self.store.root(),
            &run.input,
            &checkpoint,
            run.state.research_required,
        ) {
            return Ok(prompts::checkpoint_retry(
                ".gewu/harness/investigation.json",
                &[error.to_string()],
            ));
        }
        self.advance(&mut run.state, HarnessStage::Plan)?;
        Ok(accepted(
            prompts::plan(&run.input, self.max_chunks),
            "`investigation.json`",
        ))
    }

    fn investigation_prompt(&self, run: &Run) -> String {
        if run.state.research_required {
            crate::research_state::prompt()
        } else {
            prompts::investigate()
        }
    }

    fn finish_plan(&self, run: &mut Run) -> Result<String> {
        let list = match self.store.work_list() {
            Ok(Some(list)) => list,
            Ok(None) => return Ok(prompts::plan(&run.input, self.max_chunks)),
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
        if self.written_tests(run, &list).is_some() {
            return Ok(accepted(self.finish_expansion(run)?, "`worklist.json`"));
        }
        Ok(accepted(self.prompt_expand(run, &list)?, "`worklist.json`"))
    }

    /// Validates the pending expansion and appends its chunks to the spec.
    fn finish_expansion(&self, run: &mut Run) -> Result<String> {
        let mut work = self.work_list()?;
        let Some(item) = work.next_item().cloned() else {
            let spec = self.spec_so_far(run)?;
            artifacts::write(&self.spec_path, &spec)?;
            self.advance(&mut run.state, HarnessStage::Await)?;
            return self.evaluate(run);
        };
        let (expansion, written) = match self.store.expansion() {
            Ok(Some(expansion)) => (expansion, false),
            Ok(None) => match self.written_tests(run, &work) {
                Some(expansion) => (expansion, true),
                None => return self.prompt_expand(run, &work),
            },
            Err(error) => return self.repair(run, &item, &[error.to_string()]),
        };
        if expansion.item_id != item.id {
            let mismatch = format!(
                "expansion names item `{}`, but the next item is `{}`",
                expansion.item_id, item.id
            );
            return self.repair(run, &item, &[mismatch]);
        }
        if let Err(errors) = self.check_expansion(run, &expansion, &item) {
            // The draft exists, so the errors and its measurement are what the
            // agent needs; reprinting the whole stage prompt buried them (found
            // by self-review).
            if run.state.stage != HarnessStage::Repair {
                self.advance(&mut run.state, HarnessStage::Repair)?;
            }
            return Ok(prompts::draft_retry(
                &errors,
                &prompts::measured(&expansion.chunks, &run.input),
            ));
        }
        let mut spec = self.spec_so_far(run)?;
        let batch: Vec<String> = expansion
            .chunks
            .iter()
            .map(|chunk| chunk.id.clone())
            .collect();
        // Each chunk carries the item it was written for, so the reviewer can
        // see which part of the walkthrough they are in.
        spec.chunks
            .extend(expansion.chunks.into_iter().map(|chunk| ReviewChunk {
                section: Some(item.title.clone()),
                ..chunk
            }));
        work.mark_expanded(&item.id, &batch);
        spec = spec.with_remaining_work(work.remaining());
        if let Err(errors) = ValidatedReview::validate_partial(run.input.clone(), spec.clone()) {
            return self.repair(run, &item, &error_strings(errors.0));
        }
        artifacts::write(&self.spec_path, &spec)?;
        self.store.save_work_list(&work)?;
        self.store.discard_expansion()?;
        let chunks = batch.len();
        let note = format!(
            "`expansion.json`: {chunks} chunk{} for `{}`",
            if chunks == 1 { "" } else { "s" },
            item.id
        );
        Ok(accepted(self.after_batch(run, batch, written)?, &note))
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
                if self.written_tests(run, &work).is_some() {
                    return self.finish_expansion(run);
                }
                // Say what the reviewer left, so "no feedback" is not mistaken
                // for "feedback ignored" (found by self-review).
                let spec = self.spec()?;
                let session = self.session(&spec)?;
                let left = session.feedback.len();
                let reading = spec
                    .chunks
                    .iter()
                    .any(|chunk| !session.reviewed.contains(&chunk.id));
                let prompt = self.prompt_expand(run, &work)?;
                let (heading, body) = prompt.split_once("\n\n").unwrap_or((&prompt, ""));
                Ok(format!(
                    "{heading}\n\n{}{body}",
                    prompts::reviewer_read(left, reading)
                ))
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
            &spec,
            &session,
            &work,
            &current,
            &run.input,
            self.force,
            run.state.write_ahead,
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
        replace_chunks(&mut spec, revision.chunks);
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
            .map_err(|errors| Error::message(errors.to_string()))?;
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
        Ok(prompts::revise(&chunks, &session, &run.input))
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
        crate::research_state::archive(self.store.root(), run.state.revision, &run.input)?;
        artifacts::write(&self.input_path, &current)?;
        artifacts::write(&self.spec_path, &spec)?;
        self.store.save_work_list(&work)?;
        self.store.discard_expansion()?;
        run.state.source = current.source.clone();
        run.input = current;
        self.advance(&mut run.state, HarnessStage::Investigate)?;
        if run.state.research_required {
            return Ok(format!(
                "# Gewu harness: the plan is stale\n\n{} stale chunk(s) were removed; {remainder} change(s) are uncovered. Surviving chunks and feedback were kept. Previous research was archived.\n\n{}",
                stale.len(),
                crate::research_state::prompt()
            ));
        }
        Ok(prompts::replan(stale, remainder))
    }

    fn prompt_expand(&self, run: &Run, work: &WorkList) -> Result<String> {
        match work.next_item() {
            Some(item) if self.written_tests(run, work).is_some() => {
                Ok(written::written_prompt(&item.id))
            }
            Some(item) => {
                let investigation = self.store.investigation()?;
                Ok(prompts::expand(
                    item,
                    &run.input,
                    investigation.as_ref(),
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
            &prompts::expand(item, &run.input, investigation.as_ref(), self.max_chunks),
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
                illustration: checkpoint.illustration,
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
            .ok_or_else(|| Error::message("work list is missing; the plan stage has not completed"))
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
        Ok(gewu_git::Repository::new(&self.project)
            .worktree_input(&run.state.origin.base, run.input.max_chunk_lines)?)
    }

    fn run(&self) -> Result<Run> {
        let state = self
            .store
            .state()?
            .ok_or_else(|| Error::message("no harness run; use `gewu harness start`"))?;
        if state.schema_version != HARNESS_SCHEMA_VERSION {
            return Err(Error::message(format!(
                "unsupported harness state version {}",
                state.schema_version
            )));
        }
        let input: ReviewInput = artifacts::read(&self.input_path)?;
        if state.source != input.source {
            return Err(Error::message(
                "harness state does not match review input; reset and restart",
            ));
        }
        Ok(Run { state, input })
    }
}

fn error_strings(errors: Vec<impl ToString>) -> Vec<String> {
    errors.into_iter().map(|error| error.to_string()).collect()
}

/// Puts rewritten chunks in place of the chunks with their ids.
fn replace_chunks(spec: &mut ReviewSpec, mut rewritten: Vec<ReviewChunk>) {
    for chunk in &mut spec.chunks {
        if let Some(position) = rewritten.iter().position(|other| other.id == chunk.id) {
            // A rewrite changes how a chunk reads, not where it belongs.
            let section = chunk.section.take();
            *chunk = rewritten.remove(position);
            chunk.section = section;
        }
    }
}

/// Puts "Accepted …" under the next prompt's heading, so an agent knows its
/// checkpoint went through before it reads what comes next. Found by
/// dogfooding: `harness next` printed the next stage with no word that the
/// last one was taken. The heading stays first, since drivers match on it.
fn accepted(prompt: String, what: &str) -> String {
    match prompt.split_once('\n') {
        Some((heading, rest)) => format!("{heading}\n\nAccepted {what}.\n{rest}"),
        None => format!("{prompt}\n\nAccepted {what}."),
    }
}

#[cfg(test)]
mod ahead_tests;
#[cfg(test)]
mod claims_tests;
mod names;
mod proofread;
mod reminders;
#[cfg(test)]
mod share_tests;
mod steppers;
mod tally;
#[cfg(test)]
mod tests;
mod written;
