use std::sync::Mutex;

use serde_json::{Value, json};

use super::Transport;
use crate::{Account, GitHubError, SignInPoll, SignInStart};

/// Answers like GitHub without contacting it, and remembers every call.
///
/// Created issues get numbers from 1 and review posts get an id, so a
/// publish run reads the same as a real one. `signed_in` decides whether
/// `GET /user` answers or reports that nobody is signed in.
#[derive(Debug, Default)]
pub struct Recorder {
    state: Mutex<RecorderState>,
}

#[derive(Debug, Default)]
struct RecorderState {
    login: Option<String>,
    calls: Vec<(String, String, Option<Value>)>,
    refuse: Option<String>,
}

impl Recorder {
    /// A recorder signed in as `login`.
    #[must_use]
    pub fn signed_in(login: &str) -> Self {
        let recorder = Self::default();
        recorder.lock().login = Some(login.to_string());
        recorder
    }

    /// Makes the next call to `path` fail, as GitHub refusing it would.
    pub fn refuse(&self, path: &str) {
        self.lock().refuse = Some(path.to_string());
    }

    /// The login [`Recorder::sign_in`] signs in as.
    pub const SANDBOX_LOGIN: &'static str = "gewu-sandbox";

    /// The sandbox's sign-in: any non-empty token signs in as
    /// [`Recorder::SANDBOX_LOGIN`], and nothing is sent anywhere.
    pub fn sign_in(&self, token: &str) -> Result<(), GitHubError> {
        if token.trim().is_empty() {
            return Err(GitHubError::SignedOut);
        }
        self.set_login(Some(Self::SANDBOX_LOGIN));
        Ok(())
    }

    /// The device flow, minus GitHub: a fixed code that is approved on the
    /// first poll, so the sandbox shows the same screens as a real sign-in.
    #[must_use]
    pub fn start_device(&self) -> SignInStart {
        SignInStart::Device {
            user_code: "GEWU-0000".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            interval: 1,
            expires_in: 900,
        }
    }

    pub fn poll_device(&self) -> Result<SignInPoll, GitHubError> {
        self.set_login(Some(Self::SANDBOX_LOGIN));
        Ok(match Account::current(self)? {
            Some(account) => SignInPoll::SignedIn { account },
            None => SignInPoll::Denied,
        })
    }

    /// Signs in or out.
    pub fn set_login(&self, login: Option<&str>) {
        self.lock().login = login.map(str::to_string);
    }

    /// Every call so far: method, path, and body.
    #[must_use]
    pub fn calls(&self) -> Vec<(String, String, Option<Value>)> {
        self.lock().calls.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RecorderState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Transport for Recorder {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, GitHubError> {
        let mut state = self.lock();
        let Some(login) = state.login.clone() else {
            return Err(GitHubError::SignedOut);
        };
        if state.refuse.as_deref() == Some(path) {
            state.refuse = None;
            return Err(GitHubError::Refused {
                method: method.to_string(),
                path: path.to_string(),
                message: "Validation Failed".to_string(),
            });
        }
        state
            .calls
            .push((method.to_string(), path.to_string(), body.cloned()));
        let count = state.calls.len();
        let repository = path
            .strip_prefix("/repos/")
            .map(|rest| rest.splitn(3, '/').take(2).collect::<Vec<_>>().join("/"))
            .unwrap_or_default();
        Ok(match (method, path) {
            ("GET", "/user") => json!({
                "login": login,
                "name": null,
                "avatar_url": avatar(&login),
                "html_url": format!("https://github.com/{login}"),
            }),
            ("GET", path) if path.contains("/contributors") => json!([
                { "login": login, "avatar_url": avatar(&login), "type": "User" },
                { "login": "grace", "avatar_url": avatar("grace"), "type": "User" },
                { "login": "dependabot[bot]", "avatar_url": avatar("dependabot"), "type": "Bot" },
                { "login": "linus", "avatar_url": avatar("linus"), "type": "User" },
            ]),
            (_, path) if path.ends_with("/issues") => json!({
                "number": count,
                "html_url": format!("https://github.com/{repository}/issues/{count}"),
            }),
            (_, path) if path.ends_with("/reviews") => {
                let pull = path.trim_end_matches("/reviews");
                json!({
                    "id": count,
                    "html_url": format!(
                        "https://github.com{}#pullrequestreview-{count}",
                        pull.trim_start_matches("/repos").replacen("/pulls/", "/pull/", 1)
                    ),
                })
            }
            _ => json!({}),
        })
    }
}

fn avatar(login: &str) -> String {
    format!("https://avatars.githubusercontent.com/{login}")
}

#[cfg(test)]
mod tests {
    use super::Recorder;
    use crate::Account;

    #[test]
    fn any_token_signs_in_as_the_sandbox_and_nothing_is_sent() {
        let github = Recorder::default();
        assert_eq!(Account::current(&github).expect("account"), None);
        assert!(github.sign_in("  ").is_err());
        github.sign_in("ghp_example").expect("sign in");
        let account = Account::current(&github)
            .expect("account")
            .expect("signed in");
        assert_eq!(account.login, Recorder::SANDBOX_LOGIN);
    }
}
