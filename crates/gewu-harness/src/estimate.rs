//! What a walkthrough will cost before an agent is spawned, and what each
//! batch did cost after.
//!
//! A reviewer pays for every agent run, so `gewu review generate` says how
//! many batches are left and roughly what they will spend. The estimate uses
//! this review's own recorded runs once there is one, and otherwise defaults
//! measured on Gewu's self-review corpora.

use std::{fmt, fs, path::Path};

use crate::Result;
use gewu_review::ReviewInput;
use serde_json::{Value, json};

/// Recorded per agent run, one JSON object per line.
const USAGE: &str = ".gewu/harness/usage.jsonl";

/// Measured with Claude Code's default model on Gewu's self-review corpora:
/// the mean of seven batches over two reviews, a 68-line TUI feature (three
/// batches, $1.38) and a 248-line verdict fix (four batches, $2.24). A first
/// batch, which also investigates and plans, runs about half again as long.
const DEFAULT_TOKENS: f64 = 480_000.0;
const DEFAULT_COST_USD: f64 = 0.52;
const DEFAULT_SECONDS: f64 = 85.0;

/// One agent run's reported use.
#[derive(Clone, Copy, Debug)]
pub struct Usage {
    pub tokens: f64,
    pub cost_usd: f64,
    /// Wall time of the run.
    pub seconds: f64,
}

impl Usage {
    /// Reads Claude's JSON report: every input, cache, and output token.
    pub fn from_report(report: &Value) -> Option<Self> {
        let usage = &report["usage"];
        let count = |key: &str| usage[key].as_f64().unwrap_or(0.0);
        let tokens = count("input_tokens")
            + count("output_tokens")
            + count("cache_creation_input_tokens")
            + count("cache_read_input_tokens");
        let cost_usd = report["total_cost_usd"].as_f64()?;
        let seconds = report["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0;
        Some(Self {
            tokens,
            cost_usd,
            seconds,
        })
    }

    /// Appends this run to the review's record.
    pub fn record(self, review_id: &str) -> Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(USAGE)?;
        let line = json!({ "review_id": review_id, "tokens": self.tokens, "cost_usd": self.cost_usd,
                    "seconds": self.seconds });
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Every run recorded for this review.
    pub fn recorded(review_id: &str) -> Vec<Self> {
        fs::read_to_string(USAGE)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|entry| entry["review_id"] == review_id)
            .map(|entry| Self {
                tokens: entry["tokens"].as_f64().unwrap_or(0.0),
                cost_usd: entry["cost_usd"].as_f64().unwrap_or(0.0),
                seconds: entry["seconds"].as_f64().unwrap_or(0.0),
            })
            .collect()
    }

    pub fn total(runs: &[Self]) -> Self {
        Self {
            tokens: runs.iter().map(|run| run.tokens).sum(),
            cost_usd: runs.iter().map(|run| run.cost_usd).sum(),
            seconds: runs.iter().map(|run| run.seconds).sum(),
        }
    }
}

/// Batches left, and what they are likely to cost.
#[derive(Debug)]
pub struct Estimate {
    low: usize,
    high: usize,
    per_batch: Usage,
    measured: bool,
    priced: bool,
}

impl Estimate {
    /// `remaining` is the plan's unexpanded items once there is a plan;
    /// before that the batch count is inferred from the diff's size. On
    /// Gewu's corpora a plan held about two items plus one per 120 lines.
    pub fn new(
        input: &ReviewInput,
        remaining: Option<usize>,
        recorded: &[Usage],
        priced: bool,
    ) -> Self {
        let (low, high) = match remaining {
            Some(items) => (items, items),
            None => {
                let lines = input
                    .changes
                    .iter()
                    .map(|change| {
                        (1..=change.body_lines())
                            .filter(|line| change.needs_review(*line))
                            .count()
                    })
                    .sum::<usize>();
                let guess = 2 + lines.div_ceil(120);
                (guess.saturating_sub(1).max(1), guess + 2)
            }
        };
        let per_batch = if recorded.is_empty() {
            Usage {
                tokens: DEFAULT_TOKENS,
                cost_usd: DEFAULT_COST_USD,
                seconds: DEFAULT_SECONDS,
            }
        } else {
            let total = Usage::total(recorded);
            let runs = recorded.len() as f64;
            Usage {
                tokens: total.tokens / runs,
                cost_usd: total.cost_usd / runs,
                seconds: total.seconds / runs,
            }
        };
        Self {
            low,
            high,
            per_batch,
            measured: !recorded.is_empty(),
            priced,
        }
    }

    /// A typical batch's wall time, for the progress line's time left.
    pub fn seconds_per_batch(&self) -> f64 {
        self.per_batch.seconds
    }

    /// Reads the plan, if there is one, for the items still to write.
    pub fn remaining_items(root: &Path) -> Option<usize> {
        let list: Value =
            serde_json::from_slice(&fs::read(root.join(".gewu/harness/worklist.json")).ok()?)
                .ok()?;
        Some(
            list["items"]
                .as_array()?
                .iter()
                .filter(|item| item["expanded"] != true)
                .count(),
        )
    }
}

impl fmt::Display for Estimate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let batches = if self.low == self.high {
            format!("{}", self.low)
        } else {
            format!("about {}–{}", self.low, self.high)
        };
        write!(
            formatter,
            "Estimate: {batches} batch(es) left, one agent run each, about {} a batch.",
            duration(self.per_batch.seconds)
        )?;
        if !self.priced {
            return write!(formatter, " This agent does not report token use or cost.");
        }
        let basis = if self.measured {
            "this review's batches so far"
        } else {
            "Gewu's own measured runs; real use depends on the code and the model"
        };
        let total = |batches: usize| {
            (
                tokens(self.per_batch.tokens * batches as f64),
                self.per_batch.cost_usd * batches as f64,
            )
        };
        let ((low_tokens, low_cost), (high_tokens, high_cost)) =
            (total(self.low), total(self.high));
        let range = if self.low == self.high {
            format!("{high_tokens} and ${high_cost:.2}")
        } else {
            let low_tokens = low_tokens.trim_end_matches(" tokens");
            format!("{low_tokens}–{high_tokens} and ${low_cost:.2}–${high_cost:.2}")
        };
        write!(
            formatter,
            " Roughly {range} in total, at about {} and ${:.2} a batch ({basis}).",
            tokens(self.per_batch.tokens),
            self.per_batch.cost_usd,
        )
    }
}

/// `4m 10s`, `45s`.
pub fn duration(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// `2.6M tokens`, `340k tokens`.
pub fn tokens(count: f64) -> String {
    if count >= 1_000_000.0 {
        format!("{:.1}M tokens", count / 1_000_000.0)
    } else {
        format!("{:.0}k tokens", count / 1_000.0)
    }
}
