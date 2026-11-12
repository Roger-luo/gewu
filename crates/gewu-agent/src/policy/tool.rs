//! Permission requests for an agent's own tools, decided by the same rules
//! as its file and terminal requests.

use std::path::PathBuf;

use agent_client_protocol::v1::schema::{ToolCallUpdateFields, ToolKind};
use serde_json::Value;

use super::Policy;

impl Policy {
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
            // Claude Code's adapter asks to run a shell line with no kind,
            // the command in its input. Found by a live run: every harness
            // command was refused as an unrecognized tool.
            // Its Write and Edit come the same way, the file in the input:
            // a call that carries text to write is checked as a write.
            _ => match call.raw_input.as_ref().and_then(Self::command_line) {
                Some(line) => self.check_shell(&line),
                None if paths.is_empty() => Err(format!("unrecognized tool: {title}")),
                None if call.raw_input.as_ref().is_some_and(Self::writes) => paths
                    .iter()
                    .try_for_each(|path| self.check_write(path).map(drop)),
                None => paths
                    .iter()
                    .try_for_each(|path| self.check_read(path).map(drop)),
            },
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

    /// Whether a tool's input carries text to write into its file.
    fn writes(input: &Value) -> bool {
        ["content", "new_string", "edits", "old_string"]
            .iter()
            .any(|key| input.get(key).is_some())
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
