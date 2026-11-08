use serde::{Deserialize, Serialize};

/// One line of what an agent did, for the reviewer's task log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Text the agent wrote to the reviewer.
    Output,
    /// A tool the agent called.
    Tool,
    /// Something Gewu did or decided.
    Status,
    /// A request Gewu refused under the harness policy.
    Denied,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentEvent {
    pub kind: EventKind,
    pub text: String,
}

impl AgentEvent {
    #[must_use]
    pub fn new(kind: EventKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

/// Receives events as they happen. Called from the session's thread.
pub type EventSink = std::sync::Arc<dyn Fn(AgentEvent) + Send + Sync>;
