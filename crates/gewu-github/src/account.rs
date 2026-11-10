use serde::{Deserialize, Serialize};

use crate::{GitHubError, Transport};

/// The signed-in GitHub user, as the account chip shows it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Account {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
}

impl Account {
    /// The signed-in user, or `None` when nobody is signed in.
    pub fn current(transport: &dyn Transport) -> Result<Option<Self>, GitHubError> {
        match transport.call("GET", "/user", None) {
            Ok(reply) => serde_json::from_value(reply)
                .map(Some)
                .map_err(|error| GitHubError::Reply(error.to_string())),
            Err(GitHubError::SignedOut) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
