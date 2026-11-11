//! What the reviewer chose about GitHub, kept in the home's `github.json`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{CliState, GitHub, Reach, SignInMethod};
use crate::{Account, GitHubError, Recorder};

/// The reviewer's GitHub choices.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Preferences {
    #[serde(default)]
    pub method: SignInMethod,
    /// Another OAuth app's client ID; `None` is Gewu's own.
    #[serde(default)]
    pub client_id: Option<String>,
    /// Publish records what it would send and sends nothing.
    #[serde(default)]
    pub rehearse: bool,
}

impl Preferences {
    pub(super) fn load(path: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    pub(super) fn save(&self, path: &Path) -> Result<(), GitHubError> {
        let text = serde_json::to_vec_pretty(self)
            .map_err(|error| GitHubError::Reply(error.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    /// A blank client ID means Gewu's own app.
    pub(super) fn normalized(mut self) -> Self {
        self.client_id = self
            .client_id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty());
        self
    }
}

/// The settings screen's GitHub section.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GitHubSettings {
    #[serde(flatten)]
    pub preferences: Preferences,
    /// Whether `gh` could post instead.
    pub cli: CliState,
    /// Gewu's own OAuth app, used when `client_id` is `None`.
    pub default_client_id: String,
    /// The sandbox's recorder: nothing is ever sent, whatever `rehearse` says.
    pub sandbox: bool,
}

impl GitHub {
    /// Every GitHub setting, and whether `gh` could sign in instead.
    pub fn settings(&self) -> Result<GitHubSettings, GitHubError> {
        let cli = match &self.reach {
            Reach::Live { cli, .. } => match Account::current(cli) {
                Ok(Some(account)) => CliState::SignedIn {
                    login: account.login,
                },
                Ok(None) => CliState::SignedOut,
                Err(GitHubError::MissingCli) => CliState::Missing,
                Err(error) => return Err(error),
            },
            Reach::Recorded(_) => CliState::SignedIn {
                login: Recorder::SANDBOX_LOGIN.to_string(),
            },
        };
        Ok(GitHubSettings {
            preferences: self.lock().clone(),
            cli,
            default_client_id: crate::GEWU_CLIENT_ID.to_string(),
            sandbox: matches!(self.reach, Reach::Recorded(_)),
        })
    }

    /// Switches sign-in method only; the account chip's Switch.
    pub fn use_method(&self, method: SignInMethod) -> Result<GitHubSettings, GitHubError> {
        let preferences = Preferences {
            method,
            ..self.lock().clone()
        };
        self.set_preferences(preferences)
    }

    /// Applies and saves every GitHub setting.
    pub fn set_preferences(&self, preferences: Preferences) -> Result<GitHubSettings, GitHubError> {
        let preferences = preferences.normalized();
        let switched = preferences.method != self.method();
        if let Some(path) = &self.saved {
            preferences.save(path)?;
        }
        match &self.reach {
            Reach::Live { oauth, .. } => oauth.set_client_id(
                preferences
                    .client_id
                    .as_deref()
                    .unwrap_or(crate::GEWU_CLIENT_ID),
            ),
            // The sandbox's two sign-ins share one recorder: the CLI is always
            // signed in, and Gewu's app keeps whatever it had before.
            Reach::Recorded(recorder) if switched => {
                let app = &self.sandbox_app;
                if preferences.method == SignInMethod::Cli {
                    let signed_in =
                        Account::current(recorder).is_ok_and(|account| account.is_some());
                    app.store(signed_in, std::sync::atomic::Ordering::SeqCst);
                    recorder.set_login(Some(Recorder::SANDBOX_LOGIN));
                } else {
                    let signed_in = app.load(std::sync::atomic::Ordering::SeqCst);
                    recorder.set_login(signed_in.then_some(Recorder::SANDBOX_LOGIN));
                }
            }
            Reach::Recorded(_) => {}
        }
        *self.lock() = preferences;
        self.settings()
    }
}
