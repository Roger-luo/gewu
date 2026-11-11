//! The reviewer's workbench settings, kept in the home's `settings.json`.

use serde::{Deserialize, Serialize};

use crate::{AgentChoice, WorkbenchError};

/// The most rewrites a fresh agent may make after a batch is written.
pub const MAX_PROOFREAD_ROUNDS: u32 = 5;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Settings {
    /// Fresh-agent rewrites of each batch before the reviewer sees it;
    /// `0` turns proofreading off.
    #[serde(default = "default_rounds")]
    pub proofread_rounds: u32,
    /// Preselected on the new-review screen. `None` picks the first agent
    /// that is installed.
    #[serde(default)]
    pub default_agent: Option<AgentChoice>,
    /// Has the agent walk through tests. Off, the default, Gewu writes test
    /// sections itself from the test code: they are ready at once and cost
    /// no agent time, but carry no explanation beyond the tests' own names and
    /// doc comments.
    #[serde(default)]
    pub explain_tests: bool,
}

fn default_rounds() -> u32 {
    2
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            proofread_rounds: default_rounds(),
            default_agent: None,
            explain_tests: false,
        }
    }
}

impl Settings {
    pub(crate) fn validated(self) -> Result<Self, WorkbenchError> {
        if self.proofread_rounds > MAX_PROOFREAD_ROUNDS {
            return Err(WorkbenchError::invalid(format!(
                "proofread rounds must be between 0 and {MAX_PROOFREAD_ROUNDS}"
            )));
        }
        if let Some(agent) = &self.default_agent
            && agent.preset_id == "custom"
            && agent
                .command
                .as_deref()
                .is_none_or(|command| command.trim().is_empty())
        {
            return Err(WorkbenchError::invalid(
                "a custom default agent needs the command that starts it",
            ));
        }
        Ok(self)
    }
}

/// The settings screen's workbench section: the settings, and where Gewu
/// keeps things, which only environment variables change.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub max_proofread_rounds: u32,
    /// `GEWU_HOME`.
    pub home: String,
    /// `GEWU_BIN`: the binary agents run harness commands with.
    pub gewu: String,
}
