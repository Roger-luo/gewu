//! The reviewer's choice of sign-in: Gewu's OAuth app, or the GitHub CLI's
//! own login for reviewers who already use `gh`.

mod settings;
#[cfg(test)]
mod tests;

pub use settings::{GitHubSettings, Preferences};

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::{
    Account, GhCli, GitHubError, OAuthSession, Recorder, SignInPoll, SignInStart, Transport,
};

/// How Gewu signs in to GitHub.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum SignInMethod {
    /// Gewu's OAuth app: a device code, and a token in the system keychain.
    #[default]
    #[serde(rename = "gewu")]
    Gewu,
    /// The GitHub CLI's login. Gewu never sees the token.
    #[serde(rename = "gh")]
    Cli,
}

/// Where the GitHub CLI stands, so the sign-in dialog can offer it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CliState {
    /// `gh` is not installed.
    Missing,
    SignedOut,
    SignedIn {
        login: String,
    },
}

enum Reach {
    Live {
        oauth: OAuthSession,
        cli: GhCli,
    },
    /// Answers like GitHub and sends nothing. Its "GitHub CLI" is signed in
    /// as [`Recorder::SANDBOX_LOGIN`].
    Recorded(Recorder),
}

/// GitHub as the reviewer signed in to it.
pub struct GitHub {
    reach: Reach,
    preferences: Mutex<Preferences>,
    /// Where the preferences are kept between runs.
    saved: Option<PathBuf>,
    /// The sandbox only: whether Gewu's app is signed in while the CLI is in
    /// use, as a real keychain would keep its token.
    sandbox_app: std::sync::atomic::AtomicBool,
}

impl GitHub {
    /// github.com through either sign-in. Preferences saved at `saved` win
    /// over `defaults`.
    #[must_use]
    pub fn live(
        oauth: OAuthSession,
        cli: GhCli,
        defaults: Preferences,
        saved: Option<PathBuf>,
    ) -> Self {
        let preferences = saved
            .as_deref()
            .and_then(Preferences::load)
            .unwrap_or(defaults)
            .normalized();
        if let Some(client_id) = &preferences.client_id {
            oauth.set_client_id(client_id);
        }
        Self {
            reach: Reach::Live { oauth, cli },
            preferences: Mutex::new(preferences),
            saved,
            sandbox_app: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The recorder, which sends nothing; the sandbox and `desktop-record`.
    #[must_use]
    pub fn recorded(recorder: Recorder) -> Self {
        Self {
            reach: Reach::Recorded(recorder),
            preferences: Mutex::default(),
            saved: None,
            sandbox_app: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Preferences> {
        self.preferences
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn method(&self) -> SignInMethod {
        self.lock().method
    }

    /// Publish should record instead of sending: the reviewer asked to
    /// rehearse, or this is the sandbox.
    pub fn rehearsing(&self) -> bool {
        matches!(self.reach, Reach::Recorded(_)) || self.lock().rehearse
    }

    /// Runs `f` with the API as the signed-in reviewer.
    pub fn with_transport<T>(&self, f: impl FnOnce(&dyn Transport) -> T) -> Result<T, GitHubError> {
        Ok(match (&self.reach, self.method()) {
            (Reach::Live { oauth, .. }, SignInMethod::Gewu) => f(oauth.transport()?.as_ref()),
            (Reach::Live { cli, .. }, SignInMethod::Cli) => f(cli),
            (Reach::Recorded(recorder), _) => f(recorder),
        })
    }

    /// The signed-in reviewer, or `None`.
    pub fn account(&self) -> Result<Option<Account>, GitHubError> {
        self.with_transport(Account::current)?
    }

    /// Starts a sign-in with the current method: a device code for Gewu's
    /// app, a pasted token for the CLI.
    pub fn start(&self) -> Result<SignInStart, GitHubError> {
        match (&self.reach, self.method()) {
            (_, SignInMethod::Cli) => Ok(SignInStart::Token),
            (Reach::Live { oauth, .. }, SignInMethod::Gewu) => oauth.start(),
            (Reach::Recorded(recorder), SignInMethod::Gewu) => Ok(recorder.start_device()),
        }
    }

    pub fn poll(&self) -> Result<SignInPoll, GitHubError> {
        match (&self.reach, self.method()) {
            (_, SignInMethod::Cli) => Ok(SignInPoll::Expired),
            (Reach::Live { oauth, .. }, SignInMethod::Gewu) => oauth.poll(),
            (Reach::Recorded(recorder), SignInMethod::Gewu) => recorder.poll_device(),
        }
    }

    /// Signs the GitHub CLI in with a pasted token.
    pub fn sign_in_with_token(&self, token: &str) -> Result<Option<Account>, GitHubError> {
        match (&self.reach, self.method()) {
            (_, SignInMethod::Gewu) => Err(GitHubError::SignIn(
                "sign in through the browser instead of pasting a token".into(),
            )),
            (Reach::Live { cli, .. }, SignInMethod::Cli) => {
                cli.sign_in(token)?;
                Account::current(cli)
            }
            (Reach::Recorded(recorder), SignInMethod::Cli) => {
                recorder.sign_in(token)?;
                Account::current(recorder)
            }
        }
    }

    /// Forgets Gewu's token. The CLI's login belongs to `gh`, so leaving it
    /// means switching back to Gewu's app.
    pub fn sign_out(&self) -> Result<(), GitHubError> {
        match (&self.reach, self.method()) {
            (Reach::Live { oauth, .. }, SignInMethod::Gewu) => oauth.sign_out(),
            (_, SignInMethod::Cli) => self.use_method(SignInMethod::Gewu).map(drop),
            (Reach::Recorded(recorder), SignInMethod::Gewu) => {
                recorder.set_login(None);
                Ok(())
            }
        }
    }
}
