//! GitHub's device authorization endpoints on github.com.

use serde::Deserialize;
use serde_json::Value;
use ureq::Agent;

use crate::GitHubError;

/// Where the device flow posts its forms. [`GitHubLogin`] is github.com;
/// tests answer with a fake.
pub trait LoginEndpoint: Send + Sync {
    fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, GitHubError>;
}

/// github.com's `/login/device/code` and `/login/oauth/access_token`.
pub struct GitHubLogin {
    agent: Agent,
}

impl Default for GitHubLogin {
    fn default() -> Self {
        Self {
            agent: crate::api::agent(),
        }
    }
}

impl LoginEndpoint for GitHubLogin {
    fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, GitHubError> {
        let mut response = self
            .agent
            .post(format!("https://github.com{path}"))
            .header("Accept", "application/json")
            .header("User-Agent", "gewu")
            .send_form(form.iter().copied())
            .map_err(|error| GitHubError::Network(error.to_string()))?;
        response
            .body_mut()
            .read_json()
            .map_err(|error| GitHubError::Reply(error.to_string()))
    }
}

/// What `/login/device/code` returns: the code the reviewer types, and the
/// one Gewu polls with.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct DeviceCode {
    pub(crate) device_code: String,
    pub(crate) user_code: String,
    pub(crate) verification_uri: String,
    pub(crate) expires_in: u64,
    pub(crate) interval: u64,
}
