//! The reviewer's workbench settings, kept in the home's `settings.json`.

use serde::{Deserialize, Serialize};

use gewu_review::harness::WriteAhead;

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
    /// The preset that proofreads each batch. `None` proofreads with the
    /// agent that wrote it. Found by live runs: Cursor's proofreader passed
    /// 42 of the 43 chunks it read in three pull requests, scoring 3 on 249 of
    /// 259 dimensions, so its rubric caught nothing.
    #[serde(default)]
    pub proofreader: Option<String>,
    /// How far the agent writes ahead of the reviewer. Agents write faster
    /// than people read, so by default it writes on while the reviewer reads,
    /// and the review completes once every batch is read.
    #[serde(default = "default_write_ahead")]
    pub write_ahead: WriteAhead,
}

fn default_write_ahead() -> WriteAhead {
    WriteAhead::Unlimited
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
            proofreader: None,
            write_ahead: default_write_ahead(),
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
        if let WriteAhead::Batches(count) = self.write_ahead
            && count > WriteAhead::MAX_BATCHES
        {
            return Err(WorkbenchError::invalid(format!(
                "write at most {} batches ahead, or choose no limit",
                WriteAhead::MAX_BATCHES
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
        if let Some(id) = &self.proofreader
            && !gewu_agent::Preset::find(id).is_some_and(|preset| {
                preset.kind == gewu_agent::PresetKind::Acp && preset.id != "custom"
            })
        {
            return Err(WorkbenchError::invalid(format!(
                "`{id}` cannot proofread; pick an installed agent, or the writer"
            )));
        }
        Ok(self)
    }

    /// The command that proofreads a batch `writer` wrote.
    pub(crate) fn proofreader_command(&self, writer: Vec<String>) -> Vec<String> {
        self.proofreader
            .as_deref()
            .and_then(gewu_agent::Preset::find)
            .map_or(writer, |preset| preset.command(None))
    }

    /// The backend whose prompts the proofreader gets: its own preset, or
    /// the writer's when the writer proofreads.
    pub(crate) fn proofreader_backend(&self, writer: Option<String>) -> Option<String> {
        self.proofreader.clone().or(writer)
    }
}

/// The settings screen's workbench section: the settings, and where Gewu
/// keeps things, which only environment variables change.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub max_proofread_rounds: u32,
    /// The most batches ahead a limit may allow.
    pub max_write_ahead_batches: u32,
    /// `GEWU_HOME`.
    pub home: String,
    /// `GEWU_BIN`: the binary agents run harness commands with.
    pub gewu: String,
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn the_writer_proofreads_unless_another_agent_is_chosen() {
        let writer = vec!["agent".to_owned(), "acp".to_owned()];
        let settings = Settings::default();
        assert_eq!(settings.proofreader_command(writer.clone()), writer);
        let settings = Settings {
            proofreader: Some("gemini".into()),
            ..Settings::default()
        };
        assert_eq!(settings.proofreader_command(writer), ["gemini", "--acp"]);
    }
}
