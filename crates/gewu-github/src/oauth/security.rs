//! The macOS Keychain through Apple's `/usr/bin/security`, as `gh` keeps its
//! own token.
//!
//! A Keychain item trusts the program that created it. Reading it from
//! Gewu's own binary would ask for the reviewer's password whenever that
//! binary changes, which is every rebuild during development. Going through
//! `security`, a fixed Apple-signed program, means the item it created is
//! read without asking at all.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use super::TokenStore;
use crate::GitHubError;

/// `security`'s exit status for an item that does not exist.
const NOT_FOUND: i32 = 44;

pub struct SecurityCli {
    program: PathBuf,
    service: String,
    account: String,
}

impl SecurityCli {
    /// `service` and `account` are written into a `security -i` command, so
    /// they must not contain quotes or newlines.
    #[must_use]
    pub fn new(program: impl Into<PathBuf>, service: &str, account: &str) -> Self {
        Self {
            program: program.into(),
            service: service.to_string(),
            account: account.to_string(),
        }
    }

    fn run(&self, args: &[&str], input: Option<&str>) -> Result<Output, GitHubError> {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| GitHubError::Keychain(format!("could not run security: {error}")))?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(input.as_bytes())?;
        }
        Ok(child.wait_with_output()?)
    }

    fn item(&self) -> [&str; 4] {
        ["-s", &self.service, "-a", &self.account]
    }
}

fn failed(output: &Output) -> GitHubError {
    GitHubError::Keychain(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

impl TokenStore for SecurityCli {
    fn get(&self) -> Result<Option<String>, GitHubError> {
        let mut args = vec!["find-generic-password"];
        args.extend(self.item());
        args.push("-w");
        let output = self.run(&args, None)?;
        match output.status.code() {
            Some(0) => {
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok((!token.is_empty()).then_some(token))
            }
            Some(NOT_FOUND) => Ok(None),
            _ => Err(failed(&output)),
        }
    }

    fn set(&self, token: &str) -> Result<(), GitHubError> {
        // GitHub tokens are `gho_` and letters and digits. Anything else
        // could break out of the quoted command below.
        if token.is_empty() || !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(GitHubError::Keychain(
                "GitHub returned a token Gewu cannot store".into(),
            ));
        }
        // Sent on stdin to `security -i`, so the token never appears in the
        // process list. `-U` replaces an existing item.
        let command = format!(
            "add-generic-password -U -s \"{}\" -a \"{}\" -w \"{token}\"\n",
            self.service, self.account
        );
        let output = self.run(&["-i"], Some(&command))?;
        // `security -i` exits 0 even when a command fails; it reports on stderr.
        if output.status.success() && output.stderr.is_empty() {
            Ok(())
        } else {
            Err(failed(&output))
        }
    }

    fn delete(&self) -> Result<(), GitHubError> {
        let mut args = vec!["delete-generic-password"];
        args.extend(self.item());
        let output = self.run(&args, None)?;
        match output.status.code() {
            Some(0 | NOT_FOUND) => Ok(()),
            _ => Err(failed(&output)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::SecurityCli;
    use crate::oauth::TokenStore;

    /// A `security` stand-in that keeps the item in a file.
    fn fake(dir: &std::path::Path) -> SecurityCli {
        let script = dir.join("security");
        let item = dir.join("item");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
item='{item}'
case "$1" in
  -i) read -r line; echo "$line" > '{log}'
      printf '%s' "$line" | sed 's/.*-w "\(.*\)"$/\1/' > "$item" ;;
  find-generic-password) [ -f "$item" ] || exit 44; cat "$item" ;;
  delete-generic-password) [ -f "$item" ] || exit 44; rm "$item" ;;
esac
"#,
                item = item.display(),
                log = dir.join("log").display(),
            ),
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        SecurityCli::new(script, "gewu:github.com", "oauth")
    }

    #[test]
    fn keeps_the_token_without_putting_it_on_the_command_line() {
        let dir = tempfile::tempdir().expect("dir");
        let store = fake(dir.path());
        assert_eq!(store.get().expect("get"), None);
        store.set("gho_abc123").expect("set");
        assert_eq!(store.get().expect("get").as_deref(), Some("gho_abc123"));
        let command = std::fs::read_to_string(dir.path().join("log")).expect("log");
        assert_eq!(
            command.trim(),
            r#"add-generic-password -U -s "gewu:github.com" -a "oauth" -w "gho_abc123""#
        );
        store.delete().expect("delete");
        store.delete().expect("deleting nothing is fine");
        assert_eq!(store.get().expect("get"), None);
    }

    #[test]
    fn refuses_a_token_that_could_break_the_command() {
        let dir = tempfile::tempdir().expect("dir");
        assert!(fake(dir.path()).set("gho\" -s other").is_err());
    }

    /// The real Keychain, on a scratch item: `cargo nextest run --run-ignored
    /// only keychain_round_trip`. Asks for nothing, since `security` creates
    /// and reads the item.
    #[test]
    #[ignore = "touches the login keychain"]
    fn keychain_round_trip() {
        let store = SecurityCli::new("/usr/bin/security", "gewu-test:github.com", "oauth");
        store.delete().expect("clean");
        store.set("gho_roundtrip1").expect("set");
        assert_eq!(store.get().expect("get").as_deref(), Some("gho_roundtrip1"));
        store.set("gho_roundtrip2").expect("replace");
        assert_eq!(store.get().expect("get").as_deref(), Some("gho_roundtrip2"));
        store.delete().expect("delete");
        assert_eq!(store.get().expect("get"), None);
    }
}
