//! A progress line while the agent writes a batch.
//!
//! An agent run takes minutes and prints nothing until it ends, so a reviewer
//! could not tell whether it was working or how long was left. This reads the
//! harness files the agent writes as it goes and redraws one line: where the
//! plan stands, what the agent is on, how long it has run, and roughly how
//! long remains at the pace batches have taken so far.

use std::{
    fs,
    io::{IsTerminal, Write},
    time::{Duration, Instant},
};

use serde_json::Value;

use gewu_harness::estimate::duration;

/// How often a non-terminal reader gets a plain line instead of a redraw.
const PLAIN_EVERY: Duration = Duration::from_secs(30);
const BAR: usize = 12;

pub(super) struct Watch {
    started: Instant,
    seconds_per_batch: f64,
    terminal: bool,
    printed: Option<Instant>,
}

/// What the harness files say right now.
struct Snapshot {
    stage: String,
    items: Option<(usize, usize)>,
    current: Option<String>,
    drafted: usize,
}

impl Watch {
    pub(super) fn new(seconds_per_batch: f64) -> Self {
        Self {
            started: Instant::now(),
            seconds_per_batch: seconds_per_batch.max(1.0),
            terminal: std::io::stderr().is_terminal(),
            printed: None,
        }
    }

    pub(super) fn tick(&mut self) {
        let due = self.terminal
            || self
                .printed
                .is_none_or(|printed| printed.elapsed() >= PLAIN_EVERY);
        if !due {
            return;
        }
        let line = self.line(&Snapshot::read());
        let mut stderr = std::io::stderr();
        if self.terminal {
            let _ = write!(stderr, "\r\x1b[2K{line}");
        } else {
            let _ = writeln!(stderr, "{line}");
        }
        let _ = stderr.flush();
        self.printed = Some(Instant::now());
    }

    /// Clears the line so the agent's report starts on a clean row.
    pub(super) fn finish(&self) {
        if self.terminal {
            let _ = write!(std::io::stderr(), "\r\x1b[2K");
        }
    }

    fn line(&self, snapshot: &Snapshot) -> String {
        let elapsed = self.started.elapsed().as_secs_f64();
        // This run's share of one batch, never shown as done before it is.
        let share = (elapsed / self.seconds_per_batch).min(0.95);
        let (bar, place) = match snapshot.items {
            Some((done, total)) if total > 0 => {
                let fraction = ((done as f64 + share) / total as f64).min(1.0);
                let filled = (fraction * BAR as f64).round() as usize;
                (
                    format!("[{}{}] ", "█".repeat(filled), "░".repeat(BAR - filled)),
                    format!("batch {} of {total}", (done + 1).min(total)),
                )
            }
            _ => (String::new(), "first batch".to_owned()),
        };
        let doing = match (snapshot.stage.as_str(), &snapshot.current) {
            ("investigate", _) => "investigating the change".to_owned(),
            ("plan", _) => "planning the walkthrough".to_owned(),
            ("expand" | "repair", Some(title)) => format!("writing “{title}”"),
            (stage, _) => stage.to_owned(),
        };
        let drafted = match snapshot.drafted {
            0 => String::new(),
            1 => " · 1 chunk drafted".to_owned(),
            count => format!(" · {count} chunks drafted"),
        };
        let left = self.seconds_per_batch - elapsed;
        let left = if left > 0.0 {
            format!("about {} left", duration(left))
        } else {
            "taking longer than usual".to_owned()
        };
        format!(
            "{bar}{place} · {doing}{drafted} · {} · {left}",
            duration(elapsed)
        )
    }
}

impl Snapshot {
    fn read() -> Self {
        let json =
            |path: &str| -> Option<Value> { serde_json::from_slice(&fs::read(path).ok()?).ok() };
        let stage = json(".gewu/harness/state.json")
            .and_then(|state| state["stage"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "starting".to_owned());
        let list = json(".gewu/harness/worklist.json");
        let items = list.as_ref().and_then(|list| {
            let items = list["items"].as_array()?;
            let done = items.iter().filter(|item| item["expanded"] == true).count();
            Some((done, items.len()))
        });
        let current = list.as_ref().and_then(|list| {
            list["items"]
                .as_array()?
                .iter()
                .find(|item| item["expanded"] != true)
                .and_then(|item| item["title"].as_str().map(str::to_owned))
        });
        let drafted = json(".gewu/harness/expansion.json")
            .and_then(|draft| draft["chunks"].as_array().map(Vec::len))
            .unwrap_or(0);
        Self {
            stage,
            items,
            current,
            drafted,
        }
    }
}
