//! Stepper code checked against the file it says it shows: a stepper is a
//! faithful excerpt, so every line it shows must be in the file, in order,
//! from `start_line` on.

use gewu_review::{ChunkBlock, Explainer, ReviewChunk};

use super::{HarnessRunner, Run};

/// The first line of `code` that `file` does not hold, in order, from line
/// `start` on. Indentation and `// …` elisions are not compared.
fn first_unfaithful<'a>(code: &'a str, file: &str, start: usize) -> Option<&'a str> {
    let mut lines = file.lines().skip(start.saturating_sub(1)).map(str::trim);
    code.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains('…') && !line.contains("..."))
        .find(|line| !lines.any(|file_line| file_line == *line))
}

impl HarnessRunner {
    /// A reminder naming stepper steps whose code is not the file as written.
    ///
    /// Found by QA of the full #276 walkthrough: a stepper put a method chain
    /// on one line, and its highlights landed on the wrong pass of a loop.
    pub(super) fn stepper_reminder(&self, run: &Run, chunks: &[ReviewChunk]) -> String {
        let repository = gewu_git::Repository::new(&self.project);
        let mut notes = Vec::new();
        for chunk in chunks {
            for block in &chunk.content {
                let ChunkBlock::Explainer(Explainer::Stepper {
                    title,
                    source: Some(source),
                    start_line,
                    steps,
                    ..
                }) = block
                else {
                    continue;
                };
                let path = source
                    .split(|c: char| c == ':' || c.is_whitespace())
                    .next()
                    .unwrap_or_default();
                // A directory or an empty path names no file to compare with.
                if path.is_empty() || path.ends_with('/') || !path.contains('.') {
                    continue;
                }
                let file = if run.state.origin.working_tree {
                    std::fs::read_to_string(self.project.join(path)).ok()
                } else {
                    repository.file_at(&run.input.source.head, path).ok()
                };
                let Some(file) = file.filter(|file| !file.is_empty()) else {
                    continue;
                };
                for (index, step) in steps.iter().enumerate() {
                    let Some(code) = &step.code else { continue };
                    if let Some(line) = first_unfaithful(code, &file, start_line.unwrap_or(1)) {
                        notes.push(format!(
                            "- `{}`, \"{title}\", step {}: `{line}` is not in `{path}` as written \
                             from line {}",
                            chunk.id,
                            index + 1,
                            start_line.unwrap_or(1)
                        ));
                    }
                }
            }
        }
        if notes.is_empty() {
            return String::new();
        }
        format!(
            "\nStepper code must be the file as written. Copy these lines from the file, set \
             `start_line` to where they start, and number `highlight` the same way:\n\n{}\n",
            notes.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::first_unfaithful;

    const FILE: &str = "fn a() {\n    let x = values\n        .iter()\n        .sum();\n}\n";

    /// Found by QA of the full #276 walkthrough: a chain put on one line.
    #[test]
    fn code_must_be_the_file_as_written_in_order() {
        assert_eq!(
            first_unfaithful("let x = values\n    .iter()", FILE, 1),
            None
        );
        assert_eq!(
            first_unfaithful("let x = values\n// …\n.sum();", FILE, 2),
            None
        );
        assert_eq!(
            first_unfaithful("let x = values.iter().sum();", FILE, 1),
            Some("let x = values.iter().sum();")
        );
        assert_eq!(
            first_unfaithful(".sum();\n.iter()", FILE, 1),
            Some(".iter()")
        );
        assert_eq!(first_unfaithful("fn a() {", FILE, 2), Some("fn a() {"));
    }
}
