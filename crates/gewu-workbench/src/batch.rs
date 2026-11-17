//! One batch of a task, run on its own thread: prepare the source on the
//! first batch, then either write the raw-diff review or run the agent to
//! the next reviewer gate.

use std::{path::Path, sync::Arc};

use gewu_agent::{AcpRun, AgentEvent, AgentSession, Cancel, EventKind, EventSink, ProjectState};
use gewu_harness::{DEFAULT_MAX_CHUNKS, HarnessRunner};
use gewu_review::{ReviewSpec, harness::HarnessStage};

use gewu_app::Snapshot;

use crate::{TaskStatus, Workbench, WorkbenchError, home::now, task::Task};

const WAITING: &str = "# Gewu harness: waiting for the reviewer";
const COMPLETE: &str = "# COMPLETE";
const HEADING: &str = "# Gewu harness: ";
const PROOFREADING: [&str; 2] = ["# Gewu harness: proofread", "# Gewu harness: polish"];

/// Where a batch left the task.
struct Ended {
    status: TaskStatus,
    stage: Option<String>,
    message: String,
}

impl Ended {
    fn new(status: TaskStatus, stage: Option<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            stage,
            message: message.into(),
        }
    }

    fn cancelled(stage: Option<String>) -> Self {
        Self::new(
            TaskStatus::Cancelled,
            stage,
            "Cancelled. Continue to resume.",
        )
    }

    /// What the harness prompt says about who acts next, if not the agent.
    fn gate(prompt: &str) -> Option<Self> {
        if prompt.starts_with(COMPLETE) {
            Some(Self::new(
                TaskStatus::Complete,
                Some("complete".into()),
                "The walkthrough is complete.",
            ))
        } else if prompt.starts_with(WAITING) {
            let unreviewed = prompt
                .split_once(" chunk(s) are still unreviewed")
                .and_then(|(head, _)| head.rsplit('\n').next()?.parse::<usize>().ok())
                .map(|count| match count {
                    1 => "1 step is still unreviewed. Marking it reviewed starts the next batch."
                        .to_owned(),
                    _ => format!(
                        "{count} steps are still unreviewed. The next batch starts as you mark them \
                         reviewed."
                    ),
                });
            Some(Self::new(
                TaskStatus::AwaitingReviewer,
                Some("await".into()),
                unreviewed.unwrap_or_else(|| {
                    "A batch is ready. As you mark its steps reviewed, the agent writes the next \
                     one ahead of you."
                        .to_owned()
                }),
            ))
        } else {
            None
        }
    }

    fn stage(prompt: &str) -> Option<String> {
        let heading = prompt.lines().next()?.strip_prefix(HEADING)?;
        Some(heading.split_whitespace().next()?.to_owned())
    }
}

/// How often one batch run starts its author again after it stopped between
/// items with progress made.
const MAX_RESUMES: u32 = 2;

/// Chunks the walkthrough in `root` holds, or 0 before one is written.
fn written(root: &Path) -> usize {
    std::fs::read_to_string(root.join(".gewu/review-spec.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<ReviewSpec>(&text).ok())
        .map_or(0, |spec| spec.chunks.len())
}

impl Workbench {
    /// A failure the reviewer can act on, without CLI flags or exit dumps.
    fn failure_message(error: &WorkbenchError) -> String {
        match error {
            WorkbenchError::Git(gewu_git::GitError::Empty {
                base,
                working_tree: true,
                ..
            }) => format!(
                "Nothing to review from {base}: the working tree matches that commit. \
                 There are no uncommitted changes."
            ),
            WorkbenchError::Git(gewu_git::GitError::Empty {
                base, dirty: true, ..
            }) => format!(
                "Nothing to review from {base}: that range has no commits. Your edits are \
                 uncommitted — start a new review of Uncommitted changes instead."
            ),
            WorkbenchError::Git(gewu_git::GitError::Empty { base, .. }) => {
                format!("Nothing to review from {base}: that range has no changes.")
            }
            WorkbenchError::Agent(gewu_agent::AgentError::Spawn { program, reason }) => {
                format!(
                    "Could not start `{program}`: {reason}. Install it, or start a new review \
                     with a different agent."
                )
            }
            other => {
                let message = other.to_string();
                let mut letters = message.chars();
                letters.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(letters).collect()
                })
            }
        }
    }

    pub(crate) fn run_batch(&self, task_id: &str, cancel: &Cancel) {
        // Stopping the agent breaks its pipes, so a cancelled run's error is expected noise.
        let ended = self.batch(task_id, cancel).unwrap_or_else(|error| {
            if cancel.is_cancelled() {
                Ended::cancelled(None)
            } else {
                Ended::new(TaskStatus::Failed, None, Self::failure_message(&error))
            }
        });
        let Ok(mut state) = self.state() else {
            return;
        };
        // A task deleted while it ran has nothing left to update.
        let Ok(task) = state.task(task_id) else {
            return;
        };
        let stage = ended.stage.or_else(|| task.record.stage.clone());
        let kind = match ended.status {
            TaskStatus::Failed => EventKind::Error,
            _ => EventKind::Status,
        };
        // The agent often logged this failure already, in lower case; once is
        // enough. Found by QA: the log showed the same error twice.
        let repeated = task
            .log
            .since(0)
            .events
            .last()
            .is_some_and(|last| last.text.eq_ignore_ascii_case(&ended.message));
        if !repeated {
            let _ = task.log.record(AgentEvent::new(kind, &ended.message));
        }
        let _ = task.log.flush();
        let awaiting = ended.status == TaskStatus::AwaitingReviewer;
        task.record.set(ended.status, stage, Some(ended.message));
        task.cancel = None;
        task.reload();
        let _ = self.save_task(&task.record);
        // The reviewer finished a batch while this run was ending; act on it
        // now rather than leave them at an open gate. Found by a live run of
        // #276: marks made during a run's wind-down started nothing.
        let wants_next = std::mem::take(&mut task.wants_next);
        let root = task.record.root.clone();
        drop(state);
        if !(awaiting && wants_next) {
            return;
        }
        let open = root.is_some_and(|root| {
            HarnessRunner::in_project(&root, DEFAULT_MAX_CHUNKS, false)
                .would_expand()
                .unwrap_or(false)
        });
        if open {
            let _ = self.start_batch(task_id);
        } else if let Ok(mut state) = self.state()
            && let Ok(task) = state.task(task_id)
            && let Ok(Some(review)) = task.review()
        {
            // Or they read the last step of a finished walkthrough.
            let snapshot = review.snapshot();
            let _ = self.finish_if_written(task, &snapshot);
        }
    }

    /// Close a task once the reviewer has read every step of a walkthrough
    /// the agent has finished, rather than asking them to continue into nothing.
    pub(crate) fn finish_if_written(
        &self,
        task: &mut Task,
        snapshot: &Snapshot,
    ) -> Result<(), WorkbenchError> {
        let read =
            snapshot.generation.complete && snapshot.summary.reviewed == snapshot.summary.total;
        let (TaskStatus::AwaitingReviewer, Some(root), true) =
            (task.record.status, task.record.root.as_deref(), read)
        else {
            return Ok(());
        };
        let runner = HarnessRunner::in_project(root, DEFAULT_MAX_CHUNKS, false);
        if !runner.status()?.starts_with(WAITING) {
            return Ok(());
        }
        let Some(ended) =
            Ended::gate(&runner.next()?).filter(|ended| ended.status == TaskStatus::Complete)
        else {
            return Ok(());
        };
        let _ = task
            .log
            .record(AgentEvent::new(EventKind::Status, &ended.message));
        let _ = task.log.flush();
        task.record
            .set(ended.status, ended.stage, Some(ended.message));
        task.reload();
        self.save_task(&task.record)
    }

    fn emit(&self, task_id: &str, kind: EventKind, text: impl Into<String>) {
        if let Ok(mut state) = self.state()
            && let Ok(task) = state.task(task_id)
        {
            let _ = task.log.record(AgentEvent::new(kind, text));
        }
    }

    fn update(
        &self,
        task_id: &str,
        stage: Option<String>,
        message: &str,
    ) -> Result<(), WorkbenchError> {
        let mut state = self.state()?;
        let task = state.task(task_id)?;
        task.record.stage = stage.or_else(|| task.record.stage.clone());
        task.record.message = Some(message.to_owned());
        task.record.updated_at = now();
        task.reload();
        self.save_task(&task.record)
    }

    fn batch(&self, task_id: &str, cancel: &Cancel) -> Result<Ended, WorkbenchError> {
        let (record, repository) = {
            let mut state = self.state()?;
            let record = state.task(task_id)?.record.clone();
            let repository = state.repository(&record.repository_id)?.clone();
            (record, repository)
        };
        let root = match &record.root {
            Some(root) => root.clone(),
            None => {
                self.emit(
                    task_id,
                    EventKind::Status,
                    format!("Preparing {}", record.source.title()),
                );
                let checkout = self.inner.home.task(task_id).join("checkout");
                let prepared = record.source.prepare(&repository, &checkout)?;
                let files = prepared.input.changes.len();
                self.emit(
                    task_id,
                    EventKind::Status,
                    format!("{files} change(s) to review"),
                );
                if record.uses_agent() {
                    // The app keeps the next batch ready while the reviewer
                    // reads, so finishing one opens another without a wait.
                    HarnessRunner::in_project(&prepared.root, DEFAULT_MAX_CHUNKS, false)
                        .with_proofread(self.settings().proofread_rounds)
                        .with_write_ahead(true)
                        .with_explain_tests(self.settings().explain_tests)
                        .start(
                            format!("review-{:x}", now()),
                            // The app scrolls, so chunks are held to reading
                            // time rather than the terminal's rows.
                            prepared.input.with_max_chunk_seconds(Some(
                                gewu_review::load::Rubric::builtin().budget.chunk_seconds,
                            )),
                            prepared.origin,
                        )?;
                } else {
                    Self::fallback(&prepared.root, &prepared.input)?;
                }
                let mut state = self.state()?;
                let title = prepared
                    .title
                    .map(|title| state.unique_title(&record.repository_id, task_id, title));
                let task = state.task(task_id)?;
                task.record.root = Some(prepared.root.clone());
                task.record.isolated = prepared.isolated;
                task.record.prepared = crate::task::preparation();
                if let (false, Some(title)) = (task.record.titled, title) {
                    task.record.title = title;
                }
                self.save_task(&task.record)?;
                prepared.root
            }
        };
        if !record.uses_agent() {
            return Ok(Ended::new(
                TaskStatus::Complete,
                Some("complete".into()),
                "Raw diff in file order; no agent wrote a walkthrough.",
            ));
        }
        self.agent_batch(task_id, &record, &root, cancel)
    }

    fn fallback(root: &Path, input: &gewu_review::ReviewInput) -> Result<(), WorkbenchError> {
        let state = root.join(".gewu");
        std::fs::create_dir_all(&state)?;
        std::fs::write(
            state.join("review-input.json"),
            serde_json::to_vec_pretty(input)?,
        )?;
        let spec = ReviewSpec::fallback(input);
        std::fs::write(
            state.join("review-spec.json"),
            serde_json::to_vec_pretty(&spec)?,
        )?;
        Ok(())
    }

    fn agent_batch(
        &self,
        task_id: &str,
        record: &crate::task::TaskRecord,
        root: &Path,
        cancel: &Cancel,
    ) -> Result<Ended, WorkbenchError> {
        let runner = HarnessRunner::in_project(root, DEFAULT_MAX_CHUNKS, false);
        let mut prompt = runner.status()?;
        // The reviewer may have read the batch since, or the next item may be
        // one Gewu writes itself; the harness decides, before any agent starts.
        if prompt.starts_with(WAITING) || runner.writes_next()? {
            prompt = runner.next()?;
        }
        if let Some(ended) = Ended::gate(&prompt) {
            return Ok(ended);
        }
        let mut stage = Ended::stage(&prompt);
        let command = record.preset()?.command(record.agent.command.as_deref());
        // Prompts carry the backend's additions; a custom command is no known
        // backend, so it gets the general prompts.
        let backend = (record.agent.preset_id != "custom").then(|| record.agent.preset_id.clone());
        // The author stops where proofreading begins, and a fresh agent reads
        // the batch: a walkthrough reads as complete to whoever wrote it. Each
        // rewrite takes one more run, and the last reading one more.
        // Rewrites a batch may get after a fresh agent scores it.
        let rounds = self.settings().proofread_rounds;
        let proofreader = self.settings().proofreader_command(command.clone());
        let proofreader_backend = self.settings().proofreader_backend(backend.clone());
        let mut fresh = 0;
        // Times an author that stopped between items, having written one, is
        // started again: the gate lets it write one batch ahead, and an agent
        // may take one accepted batch for the end of its task.
        //
        // Counted only while no progress is made: one run writes many batches
        // when the reviewer keeps up, each ending in a hand-off. Found by a
        // live run of #276 with proofreading on: a run gave up at an open
        // gate after two sections.
        let mut resumed = 0;
        let mut furthest = written(root);
        let sink: EventSink = {
            let workbench = self.clone();
            let id = task_id.to_owned();
            Arc::new(move |event: AgentEvent| workbench.emit(&id, event.kind, event.text))
        };
        // One author for the whole loop: a fresh one per batch re-read the
        // description, manifest, and code before writing, about half the
        // writing time in a live run of #276. Proofreaders stay fresh agents.
        let mut author = AgentSession::new(
            AcpRun::new(root, command.clone(), &self.inner.gewu).with_backend(backend.clone()),
            Arc::clone(&sink),
        );
        let outcome = loop {
            let written_before = written(root);
            let (message, start) = if fresh == 0 {
                ("The agent is writing the next batch", "Starting the agent")
            } else {
                (
                    "A fresh agent is proofreading the batch",
                    "Starting a fresh agent to proofread the batch against the rubric",
                )
            };
            self.update(task_id, stage.clone(), message)?;
            self.emit(task_id, EventKind::Status, start);
            let before = ProjectState::capture(root)?;
            let outcome = if fresh == 0 {
                author.turn(cancel)
            } else {
                AcpRun::new(root, proofreader.clone(), &self.inner.gewu)
                    .with_backend(proofreader_backend.clone())
                    .run(Arc::clone(&sink), cancel)
            };
            if ProjectState::capture(root)? != before {
                return Ok(Ended::new(
                    TaskStatus::Failed,
                    stage,
                    format!(
                        "The agent changed files outside .gewu/. Inspect them with `git status` \
                         in {} before continuing.",
                        root.display()
                    ),
                ));
            }
            let outcome = outcome?;
            if written(root) > furthest {
                furthest = written(root);
                resumed = 0;
            }
            let cancelled = cancel.is_cancelled() || outcome.stop_reason == "cancelled";
            let mut prompt = runner.status()?;
            // An agent may stop where an item of tests begins; Gewu writes it.
            if !cancelled && runner.writes_next()? {
                prompt = runner.next()?;
            }
            // The run ended waiting, but the reviewer may have finished the
            // older batch while the agent wound down, or a proofreader passed
            // the batch it read. Then the gate is already open: an author
            // writes the next batch instead of the reviewer pressing Continue.
            // Found by reviewing the write-ahead change.
            if prompt.starts_with(WAITING)
                && !cancelled
                && outcome.denials.is_empty()
                && resumed < MAX_RESUMES
                && runner.would_expand()?
            {
                prompt = runner.next()?;
                // Gewu may have written the next item itself and reached the
                // gate again, with nothing left for an agent to do.
                if let Some(ended) = Ended::gate(&prompt) {
                    return Ok(ended);
                }
                stage = Ended::stage(&prompt).or(stage);
                resumed += 1;
                fresh = 0;
                continue;
            }
            if let Some(ended) = Ended::gate(&prompt) {
                return Ok(ended);
            }
            stage = Ended::stage(&prompt).or(stage);
            if cancelled {
                return Ok(Ended::cancelled(stage));
            }
            // Requests the reviewer left while the agent wrote ahead hold the
            // gate; they reach the agent when the reviewer continues, not as a
            // failure. Found by reviewing the write-ahead change.
            if runner.stage()? == HarnessStage::Await {
                return Ok(Ended::new(
                    TaskStatus::AwaitingReviewer,
                    Some("await".into()),
                    "Your feedback is waiting for the agent. Continue sends it.",
                ));
            }
            let proofreading = PROOFREADING
                .iter()
                .any(|heading| prompt.starts_with(heading));
            // An author that wrote a batch may stop where the next item
            // begins; it is started again to write it.
            let progressed = written(root) > written_before;
            if !proofreading
                && progressed
                && outcome.denials.is_empty()
                && stage.as_deref() == Some("expand")
                && resumed < MAX_RESUMES
            {
                resumed += 1;
                fresh = 0;
                continue;
            }
            if !proofreading || !outcome.denials.is_empty() || fresh > rounds {
                break outcome;
            }
            fresh += 1;
        };
        let at = stage.as_deref().unwrap_or("an unknown stage");
        let mut message = format!("The agent stopped at {at} before the reviewer gate.");
        if let Some(denied) = outcome.denials.last() {
            message.push_str(&format!(" It was refused: {denied}."));
        } else if !outcome.message.is_empty() {
            message.push_str(&format!(" It said: {}", outcome.message));
        }
        Ok(Ended::new(TaskStatus::Failed, stage, message))
    }
}
