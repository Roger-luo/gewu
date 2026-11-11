//! `gewu review rubric`: the cognitive-load rubric in effect, for a person
//! reviewing it or a calibration script reading it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use gewu_review::load::Rubric;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Format {
    #[default]
    Toml,
    Json,
}

#[derive(Debug, Args)]
pub struct RubricArgs {
    /// Check and print this rubric file instead of the one built into Gewu.
    #[arg(long)]
    rubric: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

impl RubricArgs {
    pub fn run(self) -> Result<()> {
        let rubric = resolve(self.rubric.as_deref())?;
        match self.format {
            Format::Toml => print!("{}", toml::to_string(&rubric)?),
            Format::Json => println!("{}", serde_json::to_string_pretty(&rubric)?),
        }
        Ok(())
    }
}

/// The rubric at `path`, checked, or the built-in one.
pub(super) fn resolve(path: Option<&Path>) -> Result<Rubric> {
    let Some(path) = path else {
        return Ok(Rubric::builtin().clone());
    };
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the rubric {}", path.display()))?;
    Rubric::parse(&text).with_context(|| format!("in {}", path.display()))
}
