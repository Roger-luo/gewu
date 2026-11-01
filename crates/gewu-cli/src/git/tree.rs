use std::process::Command;

use anyhow::{Context, Result, bail};

use super::{EXCLUDE_STATE, Repository};

impl Repository<'_> {
    /// Paths the diff between two tree-ishes touches within `pathspecs`, so an
    /// audit can check each reached the manifest.
    pub fn changed_paths(
        &self,
        base: &str,
        head: &str,
        pathspecs: &[String],
    ) -> Result<Vec<String>> {
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--name-only",
            "-z",
            base,
            head,
            "--",
        ];
        args.extend(pathspecs.iter().map(String::as_str));
        if pathspecs.is_empty() {
            args.push(".");
        }
        args.push(EXCLUDE_STATE);
        Ok(Self::nul_separated(&self.git_bytes(&args, false)?))
    }

    /// Every tracked path in a tree-ish.
    pub fn tree_paths(&self, head: &str) -> Result<Vec<String>> {
        let output = self.git_bytes(&["ls-tree", "-r", "-z", "--name-only", head], false)?;
        Ok(Self::nul_separated(&output))
    }

    /// Every identifier in a tree-ish's text files, for resolving references
    /// to code the diff leaves unchanged.
    pub fn tree_words(&self, head: &str) -> Result<Vec<String>> {
        let pattern = "[A-Za-z_][A-Za-z0-9_]*";
        let args = [
            "grep",
            "-h",
            "-o",
            "-I",
            "-E",
            pattern,
            head,
            "--",
            ".",
            EXCLUDE_STATE,
        ];
        let output = self.git_bytes(&args, true)?;
        Ok(String::from_utf8_lossy(&output)
            .lines()
            .map(str::to_owned)
            .collect())
    }

    /// Whether any text file of a tree-ish contains `text` verbatim.
    pub fn tree_contains(&self, head: &str, text: &str) -> Result<bool> {
        let args = [
            "grep",
            "-q",
            "-I",
            "-F",
            "-e",
            text,
            head,
            "--",
            ".",
            EXCLUDE_STATE,
        ];
        Ok(self.git_status(&args)? == Some(0))
    }

    fn git_status(&self, args: &[&str]) -> Result<Option<i32>> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.root)
            .output()
            .context("could not run git")?;
        if !output.status.success() && output.status.code() != Some(1) {
            bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(output.status.code())
    }

    /// `git grep` exits 1 when nothing matches, which is not a failure here.
    fn git_bytes(&self, args: &[&str], no_match_ok: bool) -> Result<Vec<u8>> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.root)
            .output()
            .context("could not run git")?;
        let empty = no_match_ok && output.status.code() == Some(1) && output.stderr.is_empty();
        if !output.status.success() && !empty {
            bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(output.stdout)
    }

    fn nul_separated(output: &[u8]) -> Vec<String> {
        output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect()
    }
}
