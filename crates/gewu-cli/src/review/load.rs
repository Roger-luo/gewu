//! `gewu review load`: how much a walkthrough asks of its reader, per chunk
//! and as a whole, by the cognitive-load rubric. Advisory: the harness holds
//! app chunks to the rubric's reading budget, but this verdict gates nothing.

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::{Args, ValueEnum};
use gewu_decide::{Decider, KEY_VARIABLE, PROVIDER, Scored, TypesafeDecider, score};
use gewu_review::load::{Rubric, WalkthroughLoad, chunk_state};
use gewu_review::{ReviewInput, ReviewSpec};
use serde_json::json;

use super::ArtifactArgs;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Format {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct LoadArgs {
    #[command(flatten)]
    artifacts: ArtifactArgs,
    /// Ask TypeSafe Jev the cognitive-load questions about every chunk. Sends
    /// each chunk's prose and quoted code to api.typesafe.ai, reading the key
    /// from TYPESAFE_API_KEY. Without it, only reading time is scored.
    #[arg(long)]
    jev: bool,
    /// With `--format json`, include each chunk's kind and the exact text a
    /// decider reads, for labelling chunks to calibrate against.
    #[arg(long)]
    with_state: bool,
    /// Score with this rubric file instead of the one built into Gewu.
    #[arg(long)]
    rubric: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

impl LoadArgs {
    pub fn run(self) -> Result<()> {
        let review = self.artifacts.load_partial()?;
        let rubric = super::rubric::resolve(self.rubric.as_deref())?;
        let jev = self.jev.then(TypesafeDecider::default);
        if jev.is_some() {
            eprintln!("Sending each chunk to {PROVIDER} (key from {KEY_VARIABLE}).");
        }
        let decider = jev.as_ref().map(|jev| jev as &dyn Decider);
        let scored = score(review.spec(), review.input(), &rubric, decider);
        match self.format {
            Format::Text => print!("{}", text(&scored, &rubric)),
            Format::Json => {
                let mut value = as_json(&scored, &rubric);
                if self.with_state {
                    add_states(&mut value, review.spec(), review.input());
                }
                println!("{}", serde_json::to_string_pretty(&value)?);
            }
        }
        Ok(())
    }
}

fn text(scored: &Scored, rubric: &Rubric) -> String {
    let load = &scored.load;
    let mut out = String::new();
    for chunk in &load.chunks {
        let drivers = chunk
            .drivers
            .iter()
            .take(3)
            .map(|(name, share)| format!("{name} {share:.2}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "{:>5.0}%  E[L] {:.2}  {}  {}{}\n",
            chunk.overloaded * 100.0,
            chunk.expected,
            chunk.id,
            chunk.reading,
            if drivers.is_empty() {
                String::new()
            } else {
                format!("; {drivers}")
            }
        ));
    }
    for (id, error) in &scored.failures {
        out.push_str(&format!("unanswered: {id}: {error}\n"));
    }
    out.push_str(&summary(load, rubric));
    out
}

fn summary(load: &WalkthroughLoad, rubric: &Rubric) -> String {
    let unanswered = load.chunks.iter().any(|chunk| chunk.unanswered > 0);
    format!(
        "\n{} chunks; expected overloaded {:.1}; P(more than {}) = {:.2} → {}{}\n\
         (rubric v{}: θ {}, reading budget {} s, α {}; advisory)\n",
        load.chunks.len(),
        load.expected_overloaded(),
        load.allowed,
        load.excess,
        if load.high() {
            "high cognitive overhead"
        } else {
            "within bounds"
        },
        if unanswered {
            " — a lower bound: some questions went unanswered"
        } else {
            ""
        },
        rubric.version,
        rubric.equation.threshold,
        rubric.budget.chunk_seconds,
        rubric.equation.overloaded_share,
    )
}

fn as_json(scored: &Scored, rubric: &Rubric) -> serde_json::Value {
    let load = &scored.load;
    json!({
        "rubric_version": rubric.version,
        "high": load.high(),
        "excess": load.excess,
        "allowed": load.allowed,
        "expected_overloaded": load.expected_overloaded(),
        "chunks": load.chunks.iter().map(|chunk| json!({
            "id": chunk.id,
            "overloaded": chunk.overloaded,
            "expected": chunk.expected,
            "time_load": chunk.time_load,
            "reading_seconds": chunk.reading.total(),
            "reading": {"prose": chunk.reading.prose, "code": chunk.reading.code,
                        "explainers": chunk.reading.explainers, "context": chunk.reading.context,
                        "discount": chunk.reading.discount},
            "answers": chunk.answers,
            "unanswered": chunk.unanswered,
        })).collect::<Vec<_>>(),
        "failures": scored.failures.iter()
            .map(|(id, error)| json!({"id": id, "error": error.to_string()}))
            .collect::<Vec<_>>(),
    })
}

/// Each chunk's kind and decider state, beside its score.
fn add_states(value: &mut serde_json::Value, spec: &ReviewSpec, input: &ReviewInput) {
    let changes: HashMap<_, _> = input.changes.iter().map(|c| (&c.id, c)).collect();
    let Some(chunks) = value["chunks"].as_array_mut() else {
        return;
    };
    for (index, (scored, chunk)) in chunks.iter_mut().zip(&spec.chunks).enumerate() {
        scored["kind"] = json!(chunk.kind);
        scored["section"] = json!(chunk.section);
        scored["state"] = json!(chunk_state(chunk, &changes, &spec.chunks[..index]));
    }
}
