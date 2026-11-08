use std::{fs, path::Path, process::Command};

use sha2::{Digest, Sha256};

use crate::AgentError;

/// Everything outside `.gewu/` that an agent could change, as one digest:
/// the status list, the diff, and the contents of untracked files. Equal
/// digests before and after a run mean the agent left the project alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectState(String);

impl ProjectState {
    pub fn capture(root: &Path) -> Result<Self, AgentError> {
        let git = |args: &[&str]| -> Result<Vec<u8>, AgentError> {
            let output = Command::new("git")
                .args(args)
                .current_dir(root)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .output()
                .map_err(|error| AgentError::Git(format!("could not run git: {error}")))?;
            if !output.status.success() {
                return Err(AgentError::Git(
                    String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                ));
            }
            Ok(output.stdout)
        };
        let outside = ["--", ".", ":(exclude,top).gewu"];
        let mut digest = Sha256::new();
        digest.update(git(
            &[&["status", "--porcelain", "-z"][..], &outside].concat()
        )?);
        digest.update(git(&[&["diff", "--binary"][..], &outside].concat())?);
        let untracked = git(&[
            &["ls-files", "-o", "--exclude-standard", "-z"][..],
            &outside,
        ]
        .concat())?;
        for name in untracked
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            digest.update(name);
            let path = root.join(String::from_utf8_lossy(name).as_ref());
            if path.is_file() {
                digest.update(fs::read(path)?);
            }
        }
        Ok(Self(hex::encode(digest.finalize())))
    }
}
