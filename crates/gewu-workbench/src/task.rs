use std::path::PathBuf;

use gewu_agent::{Cancel, Preset, PresetKind};
use gewu_app::ReviewWorkspace;
use serde::{Deserialize, Serialize};

use crate::{
    AgentChoice, ReviewProgress, Source, TaskStatus, TaskView, WorkbenchError, home::now,
    log::TaskLogFile,
};

/// What is persisted about a task, in `<home>/tasks/<id>/task.json`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct TaskRecord {
    pub(crate) id: String,
    pub(crate) repository_id: String,
    pub(crate) title: String,
    /// The reviewer named it, so a pull request's title does not replace it.
    pub(crate) titled: bool,
    pub(crate) source: Source,
    pub(crate) agent: AgentChoice,
    pub(crate) status: TaskStatus,
    pub(crate) stage: Option<String>,
    pub(crate) message: Option<String>,
    /// Where `.gewu/` lives; `None` until the first batch prepares it.
    pub(crate) root: Option<PathBuf>,
    /// The root is a checkout of the task's own, removed with the task.
    pub(crate) isolated: bool,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

impl TaskRecord {
    pub(crate) fn preset(&self) -> Result<&'static Preset, WorkbenchError> {
        Preset::find(&self.agent.preset_id).ok_or_else(|| {
            WorkbenchError::invalid(format!("unknown agent `{}`", self.agent.preset_id))
        })
    }

    pub(crate) fn uses_agent(&self) -> bool {
        self.preset()
            .is_ok_and(|preset| preset.kind == PresetKind::Acp)
    }

    pub(crate) fn set(
        &mut self,
        status: TaskStatus,
        stage: Option<String>,
        message: Option<String>,
    ) {
        self.status = status;
        self.stage = stage;
        self.message = message;
        self.updated_at = now();
    }
}

/// A task in memory: its record, its log, and what is running for it.
pub(crate) struct Task {
    pub(crate) record: TaskRecord,
    pub(crate) log: TaskLogFile,
    pub(crate) cancel: Option<Cancel>,
    pub(crate) review: Option<ReviewWorkspace>,
    /// When the walkthrough the cached review was opened from was written.
    written: Option<std::time::SystemTime>,
    pub(crate) progress: Option<ReviewProgress>,
}

impl Task {
    pub(crate) fn new(record: TaskRecord, log: TaskLogFile) -> Self {
        Self {
            record,
            log,
            cancel: None,
            review: None,
            written: None,
            progress: None,
        }
    }

    /// The open review, loaded on first use. `Ok(None)` while no walkthrough
    /// has been written yet.
    ///
    /// Reopened when the walkthrough file changes: an agent writing ahead
    /// accepts a batch and goes on to the next in one run, and the reviewer
    /// must see the accepted batch without waiting for the run to end.
    /// Reopening loses nothing, since every action is already in the log.
    pub(crate) fn review(&mut self) -> Result<Option<&mut ReviewWorkspace>, WorkbenchError> {
        let Some(root) = &self.record.root else {
            return Ok(None);
        };
        let spec = root.join(".gewu/review-spec.json");
        let written = std::fs::metadata(&spec)
            .and_then(|meta| meta.modified())
            .ok();
        if written.is_none() {
            return Ok(None);
        }
        if self.review.is_none() || written != self.written {
            self.review = Some(ReviewWorkspace::open(root)?);
            self.written = written;
            self.progress = None;
        }
        Ok(self.review.as_mut())
    }

    /// Drop the cached review so the next read sees a new batch.
    pub(crate) fn reload(&mut self) {
        self.review = None;
        self.progress = None;
    }

    pub(crate) fn progress(&mut self) -> Option<ReviewProgress> {
        // Always re-read: the reviewer marks steps through the review API
        // without going through reload().
        let snapshot = self.review().ok().flatten()?.snapshot();
        let progress = ReviewProgress {
            reviewed: snapshot.summary.reviewed,
            total: snapshot.summary.total,
            open_issues: snapshot.summary.unresolved_issues.len(),
            remaining_work_items: snapshot.generation.remaining_work_items,
            decision: snapshot.summary.decision,
        };
        self.progress = Some(progress.clone());
        Some(progress)
    }

    pub(crate) fn view(&mut self, repository_name: String) -> TaskView {
        let review = self.progress();
        let record = &self.record;
        TaskView {
            id: record.id.clone(),
            repository_id: record.repository_id.clone(),
            repository_name,
            title: record.title.clone(),
            source: record.source.clone(),
            agent: record.agent.clone(),
            agent_label: record.preset().map_or_else(
                |_| record.agent.preset_id.clone(),
                |preset| preset.label.to_owned(),
            ),
            status: record.status,
            stage: record.stage.clone(),
            message: record.message.clone(),
            review,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}
