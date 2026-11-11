//! Shapes shared by the HTTP and Tauri transports. JSON is `snake_case`;
//! enums are tagged by `type`.

use gewu_agent::{EventKind, PresetView};
use serde::{Deserialize, Serialize};

/// What a review covers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Source {
    /// The commits after `base` up to and including `head`.
    CommitRange { base: String, head: String },
    /// Uncommitted edits in the repository itself, against `base`.
    WorkingTree { base: String },
    /// A GitHub pull request, from its merge base to its head.
    PullRequest { number: u64, remote: String },
    /// Everything inside `path` at `ref`, read as if newly added.
    Directory {
        path: String,
        #[serde(rename = "ref")]
        reference: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentChoice {
    pub preset_id: String,
    /// The command line for the `custom` preset.
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkbenchAction {
    AddRepository {
        path: String,
    },
    RemoveRepository {
        repository_id: String,
    },
    CreateTask {
        repository_id: String,
        source: Source,
        agent: AgentChoice,
        #[serde(default)]
        title: Option<String>,
    },
    ContinueTask {
        task_id: String,
    },
    CancelTask {
        task_id: String,
    },
    DeleteTask {
        task_id: String,
    },
    UpdateSettings {
        settings: crate::Settings,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepositoryView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub dirty: bool,
    /// Why the repository cannot be read now, e.g. it was moved.
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Generating,
    AwaitingReviewer,
    Complete,
    Failed,
    Cancelled,
}

/// How far the reviewer has got through what exists so far.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewProgress {
    pub reviewed: usize,
    pub total: usize,
    pub open_issues: usize,
    /// Work items the agent has not written yet; `None` without a plan.
    pub remaining_work_items: Option<usize>,
    /// The reviewer's recorded decision, if any.
    #[serde(default)]
    pub decision: Option<gewu_review::Decision>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskView {
    pub id: String,
    pub repository_id: String,
    pub repository_name: String,
    pub title: String,
    pub source: Source,
    pub agent: AgentChoice,
    pub agent_label: String,
    pub status: TaskStatus,
    /// The harness stage, e.g. `investigate`, `plan`, `expand`.
    pub stage: Option<String>,
    /// What happened last, or why the task failed.
    pub message: Option<String>,
    /// `None` until a walkthrough exists to open.
    pub review: Option<ReviewProgress>,
    /// Unix milliseconds.
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkbenchSnapshot {
    pub repositories: Vec<RepositoryView>,
    /// Newest first.
    pub tasks: Vec<TaskView>,
    pub agents: Vec<PresetView>,
    pub settings: crate::SettingsView,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LogEvent {
    pub seq: usize,
    /// Unix milliseconds.
    pub at: u64,
    pub kind: EventKind,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskLog {
    pub events: Vec<LogEvent>,
    /// Pass as `after` to fetch only newer events.
    pub next: usize,
}
