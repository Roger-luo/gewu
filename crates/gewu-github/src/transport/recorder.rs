use std::sync::Mutex;

use serde_json::{Value, json};

use super::Transport;
use crate::GitHubError;

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
                "avatar_url": format!("https://avatars.githubusercontent.com/{login}"),
                "html_url": format!("https://github.com/{login}"),
            }),
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
