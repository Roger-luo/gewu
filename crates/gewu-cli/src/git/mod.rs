use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

/// Gewu's own bookkeeping is never part of the change under review, and a
/// harness write must not read as the reviewer editing their code.
const EXCLUDE_STATE: &str = ":(exclude,top).gewu";
use gewu_review::{ReviewInput, SourceIdentity};

mod diff;
mod tree;

use diff::DiffParser;

#[derive(Clone, Debug)]
pub struct Repository<'a> {
    root: &'a Path,
}

impl<'a> Repository<'a> {
    #[must_use]
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    pub fn review_input(
        &self,
        base: &str,
        head: &str,
        max_chunk_lines: usize,
    ) -> Result<ReviewInput> {
        let base = self.git_text(["rev-parse", base])?;
        let head = self.git_text(["rev-parse", head])?;
        let output = Command::new("git")
            .args([
                "diff",
                "--no-ext-diff",
                "--no-color",
                "--unified=3",
                "--src-prefix=a/",
                "--dst-prefix=b/",
            ])
            .arg(&base)
            .arg(&head)
            .args(["--", ".", EXCLUDE_STATE])
            .current_dir(self.root)
            .output()
            .context("could not run git diff")?;
        if !output.status.success() {
            bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let source = SourceIdentity::new(base, head, &output.stdout);
        let diff = String::from_utf8(output.stdout).context("git diff was not UTF-8")?;
        Ok(DiffParser::new(source, &diff, max_chunk_lines).parse())
    }

    pub fn worktree_input(&self, base: &str, max_chunk_lines: usize) -> Result<ReviewInput> {
        let base = self.git_text(["rev-parse", base])?;
        let git_dir = PathBuf::from(self.git_text(["rev-parse", "--git-dir"])?);
        let git_dir = if git_dir.is_absolute() {
            git_dir
        } else {
            self.root.join(git_dir)
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let index =
            TemporaryIndex::new(git_dir.join(format!("gewu-index-{}-{nonce}", std::process::id())));
        self.git_with_index(&index.path, ["read-tree", &base])?;
        // Naming `.gewu` in an `add` pathspec is an error when it is ignored,
        // even to exclude it, so the state directory is filtered at the diff
        // instead. Staging it here is harmless: nothing reads this tree except
        // that diff.
        self.git_with_index(&index.path, ["add", "-A", "--", "."])?;
        let tree = self.git_text_with_index(&index.path, ["write-tree"])?;
        self.review_input(&base, &tree, max_chunk_lines)
    }

    /// Refuses a review with nothing in it. A range whose ends are the same
    /// commit opens as an empty walkthrough that looks broken; this happened
    /// with `--base <HEAD's commit> --head HEAD` while all work was uncommitted.
    pub fn ensure_changes(&self, input: &ReviewInput, working_tree: bool) -> Result<()> {
        if !input.changes.is_empty() {
            return Ok(());
        }
        let base = &input.source.base[..input.source.base.len().min(12)];
        let hint = if working_tree {
            "the working tree matches the base; there are no uncommitted changes".to_owned()
        } else {
            let dirty = !self.git_text(["status", "--porcelain"])?.is_empty();
            format!(
                "the range has no changes{}",
                if dirty {
                    "; your edits are uncommitted, so review them with `--base HEAD --working-tree`"
                } else {
                    ""
                }
            )
        };
        bail!("nothing to review from {base}: {hint}")
    }

    fn git_text<const N: usize>(&self, args: [&str; N]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.root)
            .output()
            .context("could not run git")?;
        if !output.status.success() {
            bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }

    fn git_with_index<const N: usize>(&self, index: &Path, args: [&str; N]) -> Result<()> {
        self.git_text_with_index(index, args).map(|_| ())
    }

    fn git_text_with_index<const N: usize>(&self, index: &Path, args: [&str; N]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .env("GIT_INDEX_FILE", index)
            .current_dir(self.root)
            .output()
            .context("could not run git with isolated index")?;
        if !output.status.success() {
            bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }
}

struct TemporaryIndex {
    path: PathBuf,
}

impl TemporaryIndex {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TemporaryIndex {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(self.path.with_extension("lock"));
    }
}

#[cfg(test)]
mod tests;
