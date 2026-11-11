//! The GitHub REST API over HTTPS with the reviewer's own OAuth token.

use serde_json::Value;
use ureq::{Agent, http::Request};

use crate::{GitHubError, Transport};

const API: &str = "https://api.github.com";

/// Calls the API as the signed-in reviewer.
pub struct TokenApi {
    agent: Agent,
    base: String,
    token: String,
}

impl TokenApi {
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self::with_base(API, token)
    }

    /// Talks to `base` instead of api.github.com; tests use a loopback server.
    #[must_use]
    pub fn with_base(base: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            agent: agent(),
            base: base.into(),
            token: token.into(),
        }
    }
}

/// An agent that hands back error statuses so GitHub's message can be read.
pub(crate) fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
}

impl Transport for TokenApi {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, GitHubError> {
        let request = Request::builder()
            .method(method)
            .uri(format!("{}{path}", self.base))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "gewu")
            .header("Content-Type", "application/json")
            .body(body.map(Value::to_string).unwrap_or_default())
            .map_err(|error| GitHubError::Network(error.to_string()))?;
        let mut response = self
            .agent
            .run(request)
            .map_err(|error| GitHubError::Network(error.to_string()))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| GitHubError::Network(error.to_string()))?;
        if status == 401 {
            return Err(GitHubError::SignedOut);
        }
        let reply: Value = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).map_err(|error| GitHubError::Reply(error.to_string()))?
        };
        if (200..300).contains(&status) {
            return Ok(reply);
        }
        Err(GitHubError::Refused {
            method: method.to_string(),
            path: path.to_string(),
            message: reply["message"]
                .as_str()
                .map_or_else(|| format!("HTTP {status}"), str::to_string),
        })
    }
}

#[cfg(test)]
mod tests;
