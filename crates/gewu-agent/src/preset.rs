use std::{env, ffi::OsStr, path::PathBuf};

use serde::{Deserialize, Serialize};

/// How a preset produces a walkthrough.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetKind {
    /// No agent: a file-ordered raw diff, written instantly.
    None,
    /// An Agent Client Protocol server started as a subprocess.
    Acp,
}

/// A named way to run an agent. `custom` takes its command from the reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: PresetKind,
    command: &'static [&'static str],
}

pub const PRESETS: [Preset; 7] = [
    Preset {
        id: "none",
        label: "No agent (raw diff, file order)",
        kind: PresetKind::None,
        command: &[],
    },
    Preset {
        id: "claude",
        label: "Claude Code",
        kind: PresetKind::Acp,
        command: &["npx", "-y", "@zed-industries/claude-code-acp"],
    },
    Preset {
        id: "codex",
        label: "Codex",
        kind: PresetKind::Acp,
        command: &["npx", "-y", "@zed-industries/codex-acp"],
    },
    Preset {
        id: "cursor",
        label: "Cursor Agent",
        kind: PresetKind::Acp,
        command: &["agent", "acp"],
    },
    Preset {
        id: "gemini",
        label: "Gemini CLI",
        kind: PresetKind::Acp,
        command: &["gemini", "--acp"],
    },
    Preset {
        id: "opencode",
        label: "OpenCode",
        kind: PresetKind::Acp,
        command: &["opencode", "acp"],
    },
    Preset {
        id: "custom",
        label: "Custom ACP command",
        kind: PresetKind::Acp,
        command: &[],
    },
];

/// What the reviewer sees about a preset before choosing it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresetView {
    pub id: String,
    pub label: String,
    pub kind: PresetKind,
    pub command: String,
    pub available: bool,
    pub detail: Option<String>,
}

impl Preset {
    #[must_use]
    pub fn find(id: &str) -> Option<&'static Self> {
        PRESETS.iter().find(|preset| preset.id == id)
    }

    /// The command line to start, with `custom` taking the reviewer's.
    #[must_use]
    pub fn command(&self, custom: Option<&str>) -> Vec<String> {
        if self.id == "custom" {
            return custom
                .map(|line| line.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default();
        }
        self.command.iter().map(|word| (*word).to_owned()).collect()
    }

    #[must_use]
    pub fn view(&self) -> PresetView {
        let (available, detail) = match (self.kind, self.command.first()) {
            (PresetKind::None, _) => (true, None),
            (PresetKind::Acp, None) => (true, Some("Enter any ACP agent command".to_owned())),
            (PresetKind::Acp, Some(program)) => match Self::which(program) {
                Some(_) => (true, None),
                None => (false, Some(format!("`{program}` not found on PATH"))),
            },
        };
        PresetView {
            id: self.id.to_owned(),
            label: self.label.to_owned(),
            kind: self.kind,
            command: self.command.join(" "),
            available,
            detail,
        }
    }

    /// Every preset, with availability checked now.
    #[must_use]
    pub fn views() -> Vec<PresetView> {
        PRESETS.iter().map(Self::view).collect()
    }

    /// `program` resolved on the agent search path, as a shell would.
    #[must_use]
    pub fn which(program: &str) -> Option<PathBuf> {
        Self::which_in(program, &crate::AcpRun::search_path())
    }

    #[must_use]
    pub fn which_in(program: &str, path: &OsStr) -> Option<PathBuf> {
        if program.contains('/') {
            let path = PathBuf::from(program);
            return path.is_file().then_some(path);
        }
        env::split_paths(path)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    }
}
