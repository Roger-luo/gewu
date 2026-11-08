use std::{collections::HashSet, path::Path};

use anyhow::{Result, bail};
use clap::{Args, ValueEnum};
use gewu_review::audit::{Audit, AuditReport, DEFAULT_ANCHOR_DISTANCE, SymbolIndex};

use super::ArtifactArgs;
use gewu_git::Repository;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Format {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct AuditArgs {
    #[command(flatten)]
    artifacts: ArtifactArgs,
    /// Pathspec the reviewer asked to review; every path the diff touches
    /// inside it must reach the manifest. Repeatable. Defaults to the whole
    /// repository.
    #[arg(long)]
    scope: Vec<String>,
    /// Chunks after which leaning on a reviewed definition, without showing
    /// it again, is reported.
    #[arg(long, default_value_t = DEFAULT_ANCHOR_DISTANCE)]
    anchor_distance: usize,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

impl AuditArgs {
    pub fn run(self) -> Result<()> {
        let review = self.artifacts.load()?;
        let repository = Repository::new(Path::new("."));
        let source = &review.spec().source;
        let symbols = SymbolIndex::from_review(&review)
            .with_words(repository.tree_words(&source.head)?)
            .with_paths(repository.tree_paths(&source.head)?);
        let scope = repository.changed_paths(&source.base, &source.head, &self.scope)?;
        let audit = |symbols| {
            Audit::new(&review, symbols)
                .with_scope(scope.clone())
                .with_anchor_distance(self.anchor_distance)
                .run()
        };
        let mut report = audit(symbols.clone());
        // A run-time path such as `events.jsonl` is often written only in code
        // the diff leaves alone; search the reviewed tree before calling it
        // unresolved. Only the few spans still unresolved are searched.
        let mut found = Vec::new();
        for unresolved in &report.references.unresolved {
            let reference = &unresolved.reference;
            if reference.contains(['/', '.'])
                && repository.tree_contains(&source.head, reference)?
            {
                found.push(reference.clone());
            }
        }
        if !found.is_empty() {
            report = audit(symbols.with_literals(found));
        }
        match self.format {
            Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            Format::Text => print!("{}", Self::text(&report)),
        }
        if !report.passes() {
            bail!("audit failed");
        }
        Ok(())
    }

    fn text(report: &AuditReport) -> String {
        let coverage = &report.coverage;
        let sum = |count: fn(&gewu_review::audit::ChangeCoverage) -> usize| {
            coverage.changes.iter().map(count).sum::<usize>()
        };
        let files: HashSet<_> = coverage.changes.iter().map(|c| &c.path).collect();
        let references = &report.references;
        let mut out = format!(
            "coverage: {} files, {} missing from the manifest · {} of {} lines explained, {} summarized\n",
            files.len(),
            coverage.missing_paths.len(),
            sum(|c| c.explained_lines),
            sum(|c| c.reviewable_lines),
            sum(|c| c.summarized_lines),
        );
        out += &format!(
            "references: {} unresolved · {} invisible · {} forward · {} distant\n",
            references.unresolved.len(),
            references.invisible.len(),
            references.forward.len(),
            references.distant.len(),
        );
        for path in &coverage.missing_paths {
            out += &format!("- missing: {path}\n");
        }
        for change in &coverage.changes {
            for range in &change.unexplained {
                out += &format!(
                    "- unexplained: {} ({}) lines {}-{}\n",
                    change.path, change.change_id, range.start, range.end
                );
            }
            if change.summarized_lines > 0 {
                out += &format!(
                    "- summarized: {} ({} lines)\n",
                    change.path, change.summarized_lines
                );
            }
        }
        for unresolved in &references.unresolved {
            out += &format!(
                "- unresolved: `{}` in {}\n",
                unresolved.reference, unresolved.location
            );
        }
        for invisible in &references.invisible {
            out += &format!(
                "- invisible: `{}` in {} cites lines no view shows\n",
                invisible.citation, invisible.chunk_id
            );
        }
        for dangling in &report.dangling_titles {
            out += &format!(
                "- dangling title: {} refers to “{}”, which no chunk is titled\n",
                dangling.chunk_id, dangling.title
            );
        }
        for echo in &report.echoes {
            out += &format!(
                "- echo: {} repeats {}: {}\n",
                echo.chunk_id, echo.repeats, echo.sentence
            );
        }
        for split in &report.split_sentences {
            out += &format!(
                "- split sentence: {} ends {} at body line {} mid-sentence\n",
                split.chunk_id, split.path, split.line
            );
        }
        for forward in &references.forward {
            out += &format!(
                "- forward: `{}` in {} is defined in {}\n",
                forward.symbol, forward.chunk_id, forward.defined_in
            );
        }
        for distant in &references.distant {
            out += &format!(
                "- distant: `{}` in {} was last shown in {}, {} chunks back\n",
                distant.symbol, distant.chunk_id, distant.last_shown, distant.distance
            );
        }
        out
    }
}
