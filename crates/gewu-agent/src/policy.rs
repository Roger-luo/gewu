//! What an agent may do while it writes a walkthrough: read the project,
//! write harness checkpoints, and run this Gewu binary's harness commands.

use std::path::{Component, Path, PathBuf};

use agent_client_protocol::v1::schema::{ToolCallUpdateFields, ToolKind};
use serde_json::Value;

/// `review` subcommands that only read. `generate`, `pr`, and `open` would
/// start another agent or a terminal UI; `resolve` and `retarget` are the
/// reviewer's decisions; `prepare` and `fallback` overwrite the review.
const READ_ONLY_REVIEW: [&str; 5] = ["guide", "validate", "audit", "frames", "handover"];

/// Characters that would let one allowed command smuggle in another.
const SHELL_META: [char; 9] = [';', '|', '&', '$', '`', '>', '<', '\n', '('];

#[derive(Clone, Debug)]
pub struct Policy {
    root: PathBuf,
    harness: PathBuf,
    spellings: Vec<String>,
}

impl Policy {
    /// `root` is the checkout under review; `gewu` is the binary the brief
    /// names. Both must be absolute.
    #[must_use]
    pub fn new(root: &Path, gewu: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let gewu = gewu.to_string_lossy().into_owned();
        let mut spellings = vec![gewu.clone(), "gewu".to_owned()];
        if let Ok(relative) = Path::new(&gewu).strip_prefix(&root) {
            let relative = relative.to_string_lossy();
            spellings.push(relative.to_string());
            spellings.push(format!("./{relative}"));
        }
        Self {
            harness: root.join(".gewu").join("harness"),
            root,
            spellings,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `text` with the project root and the Gewu binary's path elided, as a
    /// reviewer reads a tool call's title.
    #[must_use]
    pub fn shorten(&self, text: &str) -> String {
        let root = format!("{}/", self.root.display());
        text.replace(&self.spellings[0], "gewu").replace(&root, "")
    }

    /// The absolute, normalized form of `path`, resolved against the root.
    fn resolve(&self, path: &Path) -> PathBuf {
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let mut normal = PathBuf::new();
        for component in joined.components() {
            match component {
                Component::ParentDir => {
                    normal.pop();
                }
                Component::CurDir => {}
                other => normal.push(other),
            }
        }
        // Symlinks are resolved through the deepest part that exists, so a
        // file not yet written compares equal to its real directory.
        let mut existing = normal.as_path();
        let mut rest = Vec::new();
        while !existing.exists() {
            let (Some(parent), Some(name)) = (existing.parent(), existing.file_name()) else {
                return normal;
            };
            rest.push(name.to_owned());
            existing = parent;
        }
        let mut resolved = existing
            .canonicalize()
            .unwrap_or_else(|_| existing.to_path_buf());
        resolved.extend(rest.iter().rev());
        resolved
    }

    pub fn check_read(&self, path: &Path) -> Result<PathBuf, String> {
        let resolved = self.resolve(path);
        if resolved.starts_with(&self.root) {
            Ok(resolved)
        } else {
            Err(format!("read outside the project: {}", path.display()))
        }
    }

    pub fn check_write(&self, path: &Path) -> Result<PathBuf, String> {
        let resolved = self.resolve(path);
        if resolved.starts_with(&self.harness) {
            Ok(resolved)
        } else {
            Err(format!("write outside .gewu/harness/: {}", path.display()))
        }
    }

    /// Whether `program args…` is this binary's harness command, or a
    /// read-only review command.
    pub fn check_command(&self, program: &str, args: &[String]) -> Result<(), String> {
        let line = || format!("{program} {}", args.join(" "));
        if !self.spellings.iter().any(|spelling| spelling == program) {
            return Err(format!("command is not the Gewu binary: {}", line()));
        }
        match args.first().map(String::as_str) {
            Some("harness") => Ok(()),
            Some("review")
                if args
                    .get(1)
                    .is_some_and(|sub| READ_ONLY_REVIEW.contains(&sub.as_str())) =>
            {
                Ok(())
            }
            _ => Err(format!("command is not a Gewu harness command: {}", line())),
        }
    }

    /// A shell line the agent wants to run. An optional leading
    /// `cd <project> &&` is accepted, because agents add one habitually.
    pub fn check_shell(&self, line: &str) -> Result<(), String> {
        let mut rest = line.trim();
        if let Some(after) = rest.strip_prefix("cd ")
            && let Some((dir, command)) = after.split_once("&&")
        {
            let dir = dir.trim().trim_matches(['"', '\'']);
            if self.resolve(Path::new(dir)) != self.root {
                return Err(format!("command leaves the project: {line}"));
            }
            rest = command.trim();
        }
        if rest.contains(SHELL_META) {
            return Err(format!("command chains shell operators: {line}"));
        }
        let mut words = rest
            .split_whitespace()
            .map(|word| word.trim_matches(['"', '\'']).to_owned());
        let program = words.next().unwrap_or_default();
        self.check_command(&program, &words.collect::<Vec<_>>())
    }

    /// Decide a `session/request_permission` for a tool call.
    pub fn check_tool(&self, call: &ToolCallUpdateFields) -> Result<(), String> {
        let title = call.title.as_deref().unwrap_or("tool call");
        let paths = Self::paths(call);
        match call.kind.unwrap_or_default() {
            ToolKind::Think => Ok(()),
            ToolKind::Read | ToolKind::Search => paths
                .iter()
                .try_for_each(|path| self.check_read(path).map(drop)),
            ToolKind::Edit | ToolKind::Delete | ToolKind::Move => {
                if paths.is_empty() {
                    return Err(format!("edit names no file: {title}"));
                }
                paths
                    .iter()
                    .try_for_each(|path| self.check_write(path).map(drop))
            }
            ToolKind::Execute => {
                let line = call
                    .raw_input
                    .as_ref()
                    .and_then(Self::command_line)
                    .unwrap_or_else(|| title.to_owned());
                self.check_shell(&line)
            }
            ToolKind::Fetch => Err(format!("network access is not allowed: {title}")),
            ToolKind::SwitchMode => Err(format!("mode switches are not allowed: {title}")),
            _ => Err(format!("unrecognized tool: {title}")),
        }
    }

    /// Paths a tool call names, from its locations or common input keys.
    fn paths(call: &ToolCallUpdateFields) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = call
            .locations
            .iter()
            .flatten()
            .map(|location| location.path.clone())
            .collect();
        if let Some(Value::Object(input)) = &call.raw_input {
            for key in ["file_path", "filePath", "path", "notebook_path"] {
                if let Some(Value::String(path)) = input.get(key) {
                    paths.push(PathBuf::from(path));
                }
            }
        }
        paths
    }

    fn command_line(input: &Value) -> Option<String> {
        match input.get("command")? {
            Value::String(line) => Some(line.clone()),
            Value::Array(words) => Some(
                words
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
