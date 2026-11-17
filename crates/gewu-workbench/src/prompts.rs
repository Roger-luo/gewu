//! The prompts agents are sent, for the settings screen to show: Gewu keeps
//! nothing about how it instructs agents out of the reviewer's sight.

use gewu_prompts::Prompt;
use serde::{Deserialize, Serialize};

/// One prompt as the reviewer inspects it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptView {
    /// The file stem in `harness/prompts/`, e.g. `expand`.
    pub id: String,
    pub title: String,
    /// When Gewu sends it.
    pub when: String,
    /// What every agent is sent. `{{…}}` marks what Gewu fills in per run.
    pub general: String,
    /// What some backends get after it.
    pub additions: Vec<PromptAddition>,
}

/// One backend's addition to a prompt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptAddition {
    /// The agent preset's id, e.g. `codex`.
    pub backend: String,
    pub text: String,
}

impl PromptView {
    /// Every prompt, in the order a run meets them.
    #[must_use]
    pub fn all() -> Vec<Self> {
        Prompt::ALL.iter().map(|prompt| Self::of(*prompt)).collect()
    }

    fn of(prompt: Prompt) -> Self {
        Self {
            id: prompt.id().to_owned(),
            title: prompt.title().to_owned(),
            when: prompt.when().to_owned(),
            general: prompt.general().to_owned(),
            additions: prompt
                .backends()
                .filter_map(|backend| {
                    prompt.addition(backend).map(|text| PromptAddition {
                        backend: backend.to_owned(),
                        text: text.to_owned(),
                    })
                })
                .collect(),
        }
    }
}
