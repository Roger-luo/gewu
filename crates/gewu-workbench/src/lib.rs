//! Repositories, review tasks, and agent orchestration for the Gewu app.
//!
//! A [`Workbench`] lets a reviewer add repositories and start reviews of a
//! commit range, uncommitted changes, a pull request, or a directory. Each
//! review is a task with its own checkout under [`Home`], so several can
//! be generated and read at once; an ACP agent writes each batch on a
//! background thread while the UI polls [`Workbench::snapshot`].

mod batch;
mod error;
mod home;
mod log;
mod prompts;
mod protocol;
mod repository;
mod settings;
mod source;
mod task;
mod workbench;

pub use error::WorkbenchError;
pub use gewu_git::{CommitSummary, PullRequestSummary, RepositoryRefs};
pub use gewu_github::{
    Account, CliState, Contributor, GEWU_CLIENT_ID, GhCli, GitHub, GitHubError, GitHubSettings,
    OAuthSession, Posted, Preferences, Receipt, Recorder, SignInMethod, SignInPoll, SignInStart,
    Transport,
};
pub use home::Home;
pub use prompts::{PromptAddition, PromptReport, PromptView};
pub use protocol::{
    AgentChoice, LogEvent, RepositoryView, ReviewProgress, Source, TaskLog, TaskStatus, TaskView,
    WorkbenchAction, WorkbenchSnapshot,
};
pub use settings::{MAX_PROOFREAD_ROUNDS, Settings, SettingsView};
pub use workbench::{Publication, Workbench};
