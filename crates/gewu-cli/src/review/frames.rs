use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;

use super::ArtifactArgs;
use gewu_tui::ReviewTui;

#[derive(Debug, Args)]
pub struct FramesArgs {
    #[command(flatten)]
    artifacts: ArtifactArgs,
    #[arg(long, default_value_t = 80)]
    width: usize,
    #[arg(long, default_value_t = 24)]
    height: usize,
    /// Also write every page as plain text, one file per page, for a person
    /// or a judge to read.
    #[arg(long)]
    pages: Option<PathBuf>,
}

impl FramesArgs {
    /// Renders headlessly against a throwaway session, so checking the UI
    /// never records a visit, a start, or anything else in the reviewer's own.
    pub fn run(self) -> Result<()> {
        let review = self.artifacts.load_partial()?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let session =
            std::env::temp_dir().join(format!("gewu-frames-{}-{nonce}.jsonl", std::process::id()));
        let report = ReviewTui::new(review, session.clone())?.frames(self.width, self.height);
        let _ = fs::remove_file(&session);
        if let Some(directory) = &self.pages {
            Self::write_pages(directory, &report)?;
        }
        println!("{}", serde_json::to_string_pretty(&report)?);
        if report["passes"] != true {
            bail!("frames failed: clipped, unreachable, or uncued pages");
        }
        Ok(())
    }

    fn write_pages(directory: &PathBuf, report: &serde_json::Value) -> Result<()> {
        fs::create_dir_all(directory)
            .with_context(|| format!("could not create {}", directory.display()))?;
        let stages = report["stages"].as_array().into_iter().flatten();
        for (index, stage) in stages.enumerate() {
            let name = stage["stage"]
                .as_str()
                .unwrap_or_default()
                .replace([':', '/'], "_");
            let pages = stage["pages"].as_array().into_iter().flatten();
            for (page, frame) in pages.enumerate() {
                let rows: Vec<_> = frame["screen"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .collect();
                let path = directory.join(format!("{index:03}-{name}-p{}.txt", page + 1));
                fs::write(&path, rows.join("\n") + "\n")
                    .with_context(|| format!("could not write {}", path.display()))?;
            }
        }
        Ok(())
    }
}
