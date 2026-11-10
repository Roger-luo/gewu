use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

use super::Transport;
use crate::GitHubError;

/// The GitHub CLI. It holds the token, so Gewu never sees or stores one.
#[derive(Clone, Debug)]
pub struct GhCli {
    program: PathBuf,
}

impl GhCli {
    /// Uses `program`, usually `gh` resolved on the agent search path.
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// Signs the GitHub CLI in to github.com with a token the reviewer
    /// pasted. The CLI keeps the token in its own credential store.
    pub fn sign_in(&self, token: &str) -> Result<(), GitHubError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(GitHubError::SignedOut);
        }
        let output = self.run(
            &["auth", "login", "--hostname", "github.com", "--with-token"],
            Some(token.as_bytes()),
        )?;
        if output.status.success() {
            return Ok(());
        }
        Err(GitHubError::Refused {
            method: "sign in".into(),
            path: "github.com".into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }

    fn run(
        &self,
        args: &[&str],
        input: Option<&[u8]>,
    ) -> Result<std::process::Output, GitHubError> {
        let mut child = Command::new(&self.program)
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => GitHubError::MissingCli,
                _ => GitHubError::Io(error),
            })?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(input)?;
        }
        Ok(child.wait_with_output()?)
    }
}

impl Transport for GhCli {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, GitHubError> {
        let mut args = vec!["api", "--method", method, path];
        let input = body.map(Value::to_string);
        if input.is_some() {
            args.extend(["--input", "-"]);
        }
        let output = self.run(&args, input.as_deref().map(str::as_bytes))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            return serde_json::from_str(&stdout)
                .map_err(|error| GitHubError::Reply(error.to_string()));
        }
        if stderr.contains("gh auth login") || stderr.contains("HTTP 401") {
            return Err(GitHubError::SignedOut);
        }
        let message = serde_json::from_str::<Value>(&stdout)
            .ok()
            .and_then(|reply| reply["message"].as_str().map(str::to_string))
            .unwrap_or_else(|| stderr.trim().to_string());
        Err(GitHubError::Refused {
            method: method.to_string(),
            path: path.to_string(),
            message,
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use serde_json::json;

    use super::{GhCli, Transport};
    use crate::{Account, GitHubError};

    /// A `gh` stand-in that logs its arguments and stdin, then prints `reply`.
    fn fake_gh(dir: &std::path::Path, reply: &str, status: i32, stderr: &str) -> GhCli {
        let script = dir.join("gh");
        let log = dir.join("log");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$@\" > '{log}'\ncat >> '{log}'\nprintf '%s' '{reply}'\nprintf '%s' '{stderr}' >&2\nexit {status}\n",
                log = log.display()
            ),
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        GhCli::new(script)
    }

    #[test]
    fn posts_the_body_on_stdin() {
        let dir = tempfile::tempdir().expect("dir");
        let gh = fake_gh(dir.path(), r#"{"number":7}"#, 0, "");
        let reply = gh
            .call("POST", "/repos/a/b/issues", Some(&json!({"title": "T"})))
            .expect("reply");
        assert_eq!(reply["number"], 7);
        let log = std::fs::read_to_string(dir.path().join("log")).expect("log");
        assert!(
            log.starts_with("api --method POST /repos/a/b/issues --input -"),
            "{log}"
        );
        assert!(log.contains(r#"{"title":"T"}"#), "{log}");
    }

    #[test]
    fn a_cli_without_a_login_reads_as_signed_out() {
        let dir = tempfile::tempdir().expect("dir");
        let gh = fake_gh(
            dir.path(),
            "",
            4,
            "To get started with GitHub CLI, please run:  gh auth login",
        );
        assert_eq!(Account::current(&gh).expect("account"), None);
    }

    #[test]
    fn a_refusal_keeps_github_s_message() {
        let dir = tempfile::tempdir().expect("dir");
        let gh = fake_gh(
            dir.path(),
            r#"{"message":"Not Found"}"#,
            1,
            "gh: Not Found (HTTP 404)",
        );
        let error = gh.call("GET", "/repos/a/b", None).expect_err("refused");
        assert!(
            matches!(error, GitHubError::Refused { ref message, .. } if message == "Not Found")
        );
    }

    #[test]
    fn a_missing_cli_says_so() {
        let gh = GhCli::new("/nonexistent/gh");
        assert!(matches!(
            gh.call("GET", "/user", None),
            Err(GitHubError::MissingCli)
        ));
    }
}
