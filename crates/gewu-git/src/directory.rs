use gewu_review::ReviewInput;

use super::{GitError, Repository};

impl Repository<'_> {
    /// Everything inside `directory` at `reference`, presented as added, so
    /// existing code can be walked through the same way as a change.
    pub fn directory_input(
        &self,
        reference: &str,
        directory: &str,
        max_chunk_lines: usize,
    ) -> Result<ReviewInput, GitError> {
        let head = self.git_text(["rev-parse", &format!("{reference}^{{commit}}")])?;
        let directory = directory.trim_matches('/');
        let pathspec = if directory.is_empty() || directory == "." {
            "."
        } else {
            let listed = self.git_text(["ls-tree", "-d", "--name-only", &head, directory])?;
            if listed.is_empty() {
                return Err(GitError::message(format!(
                    "there is no directory `{directory}` at {reference}"
                )));
            }
            directory
        };
        let empty = self.git_text(["hash-object", "-t", "tree", "/dev/null"])?;
        self.diff_input(&empty, &head, pathspec, max_chunk_lines)
    }
}
