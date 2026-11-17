//! The prompts Gewu sends generating agents.
//!
//! Every prompt ships inside the binary. The general prompt is the same for
//! every agent; a backend may add to it (see [`Prompt::text_for`]), because
//! models differ in what they need spelled out. The workbench names the
//! backend in [`BACKEND_ENV`] on the agent's process, so a `gewu harness`
//! command the agent runs renders that backend's prompt.

mod addenda;
mod catalog;

use std::borrow::Cow;

pub use catalog::Prompt;

/// The environment variable that names the agent preset a prompt is for.
pub const BACKEND_ENV: &str = "GEWU_AGENT";

/// The preset named in [`BACKEND_ENV`], if any.
#[must_use]
pub fn backend() -> Option<String> {
    std::env::var(BACKEND_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

impl Prompt {
    /// The prompt for the backend in [`BACKEND_ENV`].
    #[must_use]
    pub fn text(self) -> Cow<'static, str> {
        self.text_for(backend().as_deref())
    }

    /// The general prompt, followed by `backend`'s addition when it has one.
    #[must_use]
    pub fn text_for(self, backend: Option<&str>) -> Cow<'static, str> {
        match backend.and_then(|backend| self.addition(backend)) {
            Some(addition) => Cow::Owned(compose(self.general(), addition)),
            None => Cow::Borrowed(self.general()),
        }
    }

    /// What `backend` adds to the general prompt, if anything.
    #[must_use]
    pub fn addition(self, backend: &str) -> Option<&'static str> {
        addenda::ADDENDA
            .iter()
            .find(|(preset, prompt, _)| *preset == backend && *prompt == self)
            .map(|(_, _, text)| *text)
    }

    /// The presets that add to this prompt.
    pub fn backends(self) -> impl Iterator<Item = &'static str> {
        addenda::ADDENDA
            .iter()
            .filter(move |(_, prompt, _)| *prompt == self)
            .map(|(preset, _, _)| *preset)
    }
}

fn compose(general: &str, addition: &str) -> String {
    format!("{}\n\n{}\n", general.trim_end(), addition.trim())
}

#[cfg(test)]
mod tests;
