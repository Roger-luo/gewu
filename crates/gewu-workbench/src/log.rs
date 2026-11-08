use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use gewu_agent::{AgentEvent, EventKind};

use crate::{LogEvent, TaskLog, WorkbenchError, home::now};

/// A task's event log: one JSON object per line, kept in memory as well so
/// polling is cheap.
pub(crate) struct TaskLogFile {
    path: PathBuf,
    events: Vec<LogEvent>,
    /// Agent text not yet ended by a newline. Agents stream words, and a
    /// reviewer reads lines.
    pending: String,
}

impl TaskLogFile {
    pub(crate) fn open(path: PathBuf) -> Self {
        let events = fs::read_to_string(&path)
            .map(|text| {
                text.lines()
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect()
            })
            .unwrap_or_default();
        Self {
            path,
            events,
            pending: String::new(),
        }
    }

    pub(crate) fn record(&mut self, event: AgentEvent) -> Result<(), WorkbenchError> {
        if event.kind == EventKind::Output {
            self.pending.push_str(&event.text);
            while let Some(end) = self.pending.find('\n') {
                let line: String = self.pending.drain(..=end).collect();
                let line = line.trim_end();
                if !line.is_empty() {
                    self.append(EventKind::Output, line.to_owned())?;
                }
            }
            return Ok(());
        }
        self.flush()?;
        self.append(event.kind, event.text)
    }

    /// Ends any partial line of agent text.
    pub(crate) fn flush(&mut self) -> Result<(), WorkbenchError> {
        let text = std::mem::take(&mut self.pending);
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        self.append(EventKind::Output, text.to_owned())
    }

    fn append(&mut self, kind: EventKind, text: String) -> Result<(), WorkbenchError> {
        let event = LogEvent {
            seq: self.events.len(),
            at: now(),
            kind,
            text,
        };
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(&event)?)?;
        self.events.push(event);
        Ok(())
    }

    pub(crate) fn since(&self, after: usize) -> TaskLog {
        TaskLog {
            events: self.events.iter().skip(after).cloned().collect(),
            next: self.events.len(),
        }
    }
}
