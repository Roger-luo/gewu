use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("no agent command was given")]
    NoCommand,
    #[error("could not start `{program}`: {reason}")]
    Spawn { program: String, reason: String },
    #[error("the agent failed: {0}")]
    Protocol(String),
    /// The agent's process ended mid-session; `reason` is its last word.
    #[error(
        "the agent stopped unexpectedly{}",
        .reason.as_ref().map(|reason| format!(": {reason}")).unwrap_or_default()
    )]
    Stopped { reason: Option<String> },
    /// The agent wants its own CLI signed in, e.g. `agent login` for Cursor.
    #[error("the agent needs you to sign in to it first: {0}")]
    SignInRequired(String),
    #[error("git: {0}")]
    Git(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl AgentError {
    /// A protocol failure told plainly: the agent's last line on stderr, or
    /// its exit status, rather than the transport's error dump.
    pub(crate) fn stopped(message: &str, stderr: &str) -> Self {
        let last_line = stderr
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_owned);
        let exit_status = || {
            let (_, rest) = message.split_once("exit status: ")?;
            let code: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (!code.is_empty()).then(|| format!("exit status {code}"))
        };
        Self::Stopped {
            reason: last_line.or_else(exit_status),
        }
    }
}

impl From<agent_client_protocol::Error> for AgentError {
    fn from(error: agent_client_protocol::Error) -> Self {
        if error.code == agent_client_protocol::ErrorCode::AUTH_REQUIRED {
            let detail = error
                .data
                .as_ref()
                .and_then(|data| data["message"].as_str())
                .unwrap_or(&error.message);
            return Self::SignInRequired(detail.to_owned());
        }
        Self::Protocol(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::AgentError;

    #[test]
    fn stopped_prefers_the_last_stderr_line() {
        let error = AgentError::stopped(
            "Internal error: \"Process exited with exit status: 3\"",
            "starting\nfake agent: simulated failure\n\n",
        );
        assert_eq!(
            error.to_string(),
            "the agent stopped unexpectedly: fake agent: simulated failure"
        );
    }

    #[test]
    fn stopped_falls_back_to_the_exit_status() {
        let error = AgentError::stopped("Process exited with exit status: 3: oops", "");
        assert_eq!(
            error.to_string(),
            "the agent stopped unexpectedly: exit status 3"
        );
        let silent = AgentError::stopped("connection closed", "");
        assert_eq!(silent.to_string(), "the agent stopped unexpectedly");
    }
}
