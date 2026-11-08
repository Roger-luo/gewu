use std::path::Path;

/// The highlighter language for a quoted file.
///
/// The extension is only a hint. `Ion.lock` is TOML and `.gitignore` has no
/// extension, so a name that does not decide the language is sniffed from the
/// quoted text.
pub(crate) struct SourceLanguage;

impl SourceLanguage {
    pub(crate) fn detect(path: &str, sample: &str) -> Option<String> {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path)
            .to_ascii_lowercase();
        if let Some(language) = Self::by_filename(&file_name) {
            return Some(language.to_owned());
        }
        let extension = Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if Self::ambiguous(&extension) {
            return Self::sniff(sample).map(str::to_owned);
        }
        let language = Self::by_extension(&extension).or_else(|| Self::sniff(sample))?;
        // A quote of only YAML front matter from a Markdown file should color
        // as YAML; the Markdown grammar leaves those keys nearly monochrome.
        if language == "markdown" && Self::yaml_front_matter(sample) {
            return Some("yaml".to_owned());
        }
        Some(language.to_owned())
    }

    /// A sample that is YAML front matter (the `---` block at the top of a
    /// Markdown file), not Markdown prose.
    fn yaml_front_matter(sample: &str) -> bool {
        let text = sample.trim_start();
        if !text.starts_with("---") {
            return false;
        }
        let body = text.trim_start_matches("---").trim_start();
        let closed = body.contains("\n---") || body.starts_with("---");
        let keys = body
            .lines()
            .take_while(|line| *line != "---")
            .filter(|line| line.contains(':'))
            .count();
        closed || keys >= 2
    }

    fn by_filename(name: &str) -> Option<&'static str> {
        match name {
            ".gitignore" | ".dockerignore" | ".npmignore" | ".prettierignore" | ".eslintignore" => {
                Some("gitignore")
            }
            "makefile" | "gnumakefile" => Some("make"),
            "dockerfile" | "containerfile" => Some("dockerfile"),
            "cargo.lock" | "uv.lock" | "poetry.lock" => Some("toml"),
            "composer.lock" => Some("json"),
            _ => None,
        }
    }

    /// Extensions that name a container (`*.lock`) or nothing, not a language.
    fn ambiguous(extension: &str) -> bool {
        matches!(extension, "" | "lock" | "txt" | "conf" | "cfg")
    }

    fn by_extension(extension: &str) -> Option<&'static str> {
        Some(match extension {
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "py" => "python",
            "go" => "go",
            "toml" => "toml",
            "json" => "json",
            "md" | "mdx" => "markdown",
            "yaml" | "yml" => "yaml",
            "sh" | "bash" | "zsh" => "shell",
            "swift" => "swift",
            "css" => "css",
            "html" | "htm" => "html",
            "xml" => "xml",
            "c" => "c",
            "h" | "hh" | "hpp" | "cc" | "cpp" | "cxx" => "cpp",
            "java" => "java",
            "kt" => "kotlin",
            "rb" => "ruby",
            "ini" => "ini",
            "properties" => "properties",
            "env" => "dotenv",
            "mk" => "make",
            "diff" | "patch" => "diff",
            _ => return None,
        })
    }

    /// Guess a language from the quoted text when the path does not name one.
    fn sniff(sample: &str) -> Option<&'static str> {
        let text = sample.trim();
        if text.is_empty() {
            return None;
        }
        if text.starts_with("#!") {
            return Some(if text.contains("python") {
                "python"
            } else {
                "shell"
            });
        }
        if text.starts_with('{') || text.starts_with('[') && Self::json_array(text) {
            return Some("json");
        }
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(40)
            .collect();
        if lines.is_empty() {
            return None;
        }
        let assignments = lines.iter().filter(|line| line.contains(" = ")).count();
        let tables = lines
            .iter()
            .filter(|line| line.starts_with('[') && line.contains(']') && !line.contains("\":"))
            .count();
        if assignments > 0 && assignments + tables >= lines.len().div_ceil(2) {
            return Some("toml");
        }
        let patterns = lines
            .iter()
            .filter(|line| {
                line.starts_with(['#', '!', '*', '/']) || line.ends_with('/') || !line.contains(' ')
            })
            .count();
        if assignments == 0 && patterns >= lines.len().div_ceil(2) {
            return Some("gitignore");
        }
        None
    }

    fn json_array(text: &str) -> bool {
        text.chars()
            .nth(1)
            .is_some_and(|char| matches!(char, '"' | '{' | '[' | ' ' | '\n'))
            && text.contains('"')
            && !text.starts_with("[[")
    }
}

#[cfg(test)]
mod tests {
    use super::SourceLanguage;

    #[test]
    fn a_lock_file_of_toml_is_toml() {
        let sample = "[[skill]] name = \"academic-writing-rubric\" kind = \"local\"";
        assert_eq!(
            SourceLanguage::detect("Ion.lock", sample).as_deref(),
            Some("toml")
        );
    }

    #[test]
    fn gitignore_is_named_even_without_an_extension() {
        assert_eq!(
            SourceLanguage::detect(".gitignore", "/build/\n*.synctex.gz\n").as_deref(),
            Some("gitignore")
        );
    }

    #[test]
    fn a_known_extension_wins_over_the_sample() {
        assert_eq!(
            SourceLanguage::detect("src/lib.rs", "const X: u32 = 1;").as_deref(),
            Some("rust")
        );
    }

    #[test]
    fn a_json_lock_file_is_json() {
        assert_eq!(
            SourceLanguage::detect("deps.lock", "{\n  \"packages\": []\n}").as_deref(),
            Some("json")
        );
    }

    /// Found by judging a real walkthrough: a SKILL.md front-matter quote was
    /// colored as Markdown and looked nearly monochrome.
    #[test]
    fn a_markdown_front_matter_quote_is_yaml() {
        let sample = "---\nname: gewu-dogfood\ndescription: Dogfood the harness.\nallowed-tools: Bash\n---\n";
        assert_eq!(
            SourceLanguage::detect(".agents/skills/gewu-dogfood/SKILL.md", sample).as_deref(),
            Some("yaml")
        );
    }
}
