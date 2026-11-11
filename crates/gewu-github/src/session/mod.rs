//! The reviewer's choice of sign-in: Gewu's OAuth app, or the GitHub CLI's
//! own login for reviewers who already use `gh`.

#[cfg(test)]
mod tests;

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

/// The method in use and what the other one would sign in as.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignInMethods {
    pub method: SignInMethod,
    pub cli: CliState,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    method: SignInMethod,
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
    method: Mutex<SignInMethod>,
    /// Where the chosen method is kept between runs.
    saved: Option<PathBuf>,
}

impl GitHub {
    /// github.com through either sign-in. The method saved at `saved` wins
    /// over `method`.
    #[must_use]
    pub fn live(
        oauth: OAuthSession,
        cli: GhCli,
        method: SignInMethod,
        saved: Option<PathBuf>,
    ) -> Self {
        let method = saved
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Saved>(&bytes).ok())
            .map_or(method, |saved| saved.method);
        Self {
            reach: Reach::Live { oauth, cli },
            method: Mutex::new(method),
            saved,
        }
    }

    /// The recorder, which sends nothing; the sandbox and `desktop-record`.
    #[must_use]
    pub fn recorded(recorder: Recorder) -> Self {
        Self {
            reach: Reach::Recorded(recorder),
            method: Mutex::default(),
            saved: None,
        }
    }

    pub fn method(&self) -> SignInMethod {
        *self
            .method
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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

    /// The method in use, and whether `gh` could sign in instead.
    pub fn methods(&self) -> Result<SignInMethods, GitHubError> {
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
        Ok(SignInMethods {
            method: self.method(),
            cli,
        })
    }

    /// Switches to `method` and remembers it for the next run.
    pub fn use_method(&self, method: SignInMethod) -> Result<SignInMethods, GitHubError> {
        *self
            .method
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = method;
        if let Some(path) = &self.saved {
            let text = serde_json::to_vec_pretty(&Saved { method })
                .map_err(|error| GitHubError::Reply(error.to_string()))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, text)?;
        }
        if let Reach::Recorded(recorder) = &self.reach {
            // The sandbox's two sign-ins are one account: the CLI is always
            // signed in, Gewu's app starts signed out.
            recorder.set_login((method == SignInMethod::Cli).then_some(Recorder::SANDBOX_LOGIN));
        }
        self.methods()
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
