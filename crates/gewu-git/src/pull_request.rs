use std::process::Command;

use super::{GitError, Repository};

/// The commit range a pull request asks to merge.
#[derive(Debug)]
pub struct PullRequest {
    /// Where the request branched off its target: the merge base, so commits
    /// the target gained since are not read as part of the request.
    pub base: String,
    pub head: String,
    pub target: String,
    pub title: Option<String>,
}

impl Repository<'_> {
    /// Resolves a GitHub pull request without checking it out. Its head is
    /// fetched from `pull/<number>/head`, which GitHub publishes for every
    /// request; its target branch comes from `gh` when that is installed,
    /// and is otherwise the remote's default branch.
    pub fn pull_request(&self, remote: &str, number: u64) -> Result<PullRequest, GitError> {
        let head = self
            .fetched(remote, &format!("pull/{number}/head"))
            .map_err(|error| {
                GitError::message(format!(
                    "could not fetch pull request #{number} from `{remote}`; is it a GitHub remote?: {error}"
                ))
            })?;
        let (target, title) = match self.described(number) {
            Some((target, title)) => (target, Some(title)),
            None => (self.default_branch(remote)?, None),
        };
        let tip = self.fetched(remote, &target)?;
        let base = self.git_text(["merge-base", &tip, &head])?;
        Ok(PullRequest {
            base,
            head,
            target,
            title,
        })
    }

    /// Fetches one ref and returns the commit it names.
    fn fetched(&self, remote: &str, reference: &str) -> Result<String, GitError> {
        let output = self
            .command()
            .args(["fetch", "--quiet", "--no-tags", remote, reference])
            .output()
            .map_err(|error| GitError::message(format!("could not run git fetch: {error}")))?;
        if !output.status.success() {
            return Err(GitError::message(format!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        self.git_text(["rev-parse", "FETCH_HEAD^{commit}"])
    }

    /// The request's target branch and title, from `gh`, if it can answer.
    fn described(&self, number: u64) -> Option<(String, String)> {
        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                &number.to_string(),
                "--json",
                "baseRefName,title",
            ])
            .current_dir(self.root)
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        let view: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        Some((
            view["baseRefName"].as_str()?.to_owned(),
            view["title"].as_str()?.to_owned(),
        ))
    }

    fn default_branch(&self, remote: &str) -> Result<String, GitError> {
        let head = self
            .git_text([
                "symbolic-ref",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ])
            .map_err(|_| {
                GitError::message(format!(
                    "could not tell `{remote}`'s default branch; install `gh`, or run `git remote \
                     set-head {remote} --auto`"
                ))
            })?;
        Ok(head
            .strip_prefix(&format!("{remote}/"))
            .unwrap_or(&head)
            .to_owned())
    }
}
