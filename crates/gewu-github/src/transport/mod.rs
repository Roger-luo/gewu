//! Ways of reaching the GitHub REST API.

mod gh;
mod recorder;

use serde_json::Value;

use crate::GitHubError;

pub use gh::GhCli;
pub use recorder::Recorder;

/// One way of reaching the GitHub REST API.
pub trait Transport: Send + Sync {
    /// Calls `method path` with an optional JSON body and returns the reply.
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, GitHubError>;
}
