//! `gewu review generate`: an agent writes the walkthrough, one batch at a time.
//!
//! Gewu starts or continues the harness, runs the agent the reviewer named,
//! and stops where the harness stops: at the reviewer gate. Running it again
//! after reading a batch continues. The agent may write only harness files;
//! anything else it changes is reported, because a walkthrough of code its
//! author edited is a walkthrough of nothing.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use sha2::{Digest, Sha256};

use super::{
    DEFAULT_MAX_CHUNKS, StartArgs,
    estimate::{self, Estimate, Usage},
    runner::HarnessRunner,
    watch::Watch,
};

const BRIEF: &str = include_str!("../../../../harness/prompts/agent.md");
const WAITING: &str = "# Gewu harness: waiting for the reviewer";
const COMPLETE: &str = "# COMPLETE";

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Agent {
    /// Claude Code, restricted to reading and to writing harness files.
    Claude,
    /// Codex, in its workspace-write sandbox.
    Codex,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    /// Which agent writes the walkthrough. Named explicitly because it sends
    /// the code under review to that provider.
    #[arg(long, value_enum)]
    agent: Agent,
    /// Start a new review from this base instead of continuing the current one.
    #[arg(long)]
    base: Option<String>,
    #[arg(long, requires = "base")]
    head: Option<String>,
    #[arg(long, requires = "base")]
    working_tree: bool,
    #[arg(long)]
    model: Option<String>,
    /// Print how many batches are left and roughly what they will cost,
    /// without starting a review or running the agent.
    #[arg(long)]
    estimate: bool,
    /// Run this program in place of the agent's CLI, with the same arguments.
    #[arg(long, hide = true)]
    agent_program: Option<PathBuf>,
}

impl GenerateArgs {
    pub fn run(self) -> Result<()> {
        let runner = HarnessRunner::new(DEFAULT_MAX_CHUNKS, false);
        if self.estimate {
            return self.print_estimate();
        }
        if let Some(base) = &self.base {
            let start = StartArgs::new(base.clone(), self.head.clone(), self.working_tree);
            let (review_id, input, origin) = start.begin()?;
            runner.start(review_id, input, origin)?;
        }
        let mut prompt = runner
            .status()
            .context("no review is started; pass `--base` (and `--head` or `--working-tree`)")?;
        if prompt.starts_with(WAITING) {
            // The reviewer may have read the batch since; `next` decides.
            prompt = runner.next()?;
        }
        if let Some(stop) = Self::stopped(&prompt) {
            println!("{stop}");
            return Ok(());
        }
        let review_id = Self::review_id().unwrap_or_default();
        let estimate = self.estimate_for(&review_id)?;
        println!("{estimate}");
        let before = Self::project_state()?;
        let mut command = self.agent_command()?;
        let (status, stdout) = self.run_watched(&mut command, estimate.seconds_per_batch())?;
        let (denied, usage) = match self.agent {
            // Claude reports in JSON, so a refused tool call can be named
            // instead of reading as an agent that simply stopped, and what
            // the batch cost can be recorded.
            Agent::Claude => Self::report(&stdout),
            Agent::Codex => {
                let text = String::from_utf8_lossy(&stdout);
                let tail: String = text
                    .chars()
                    .rev()
                    .take(4000)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                println!("{tail}");
                (Vec::new(), None)
            }
        };
        if let Some(usage) = usage {
            usage.record(&review_id)?;
            let total = Usage::total(&Usage::recorded(&review_id));
            println!(
                "This batch took {} and used {} (${:.2}); this review so far: {} (${:.2}).\n",
                estimate::duration(usage.seconds),
                estimate::tokens(usage.tokens),
                usage.cost_usd,
                estimate::tokens(total.tokens),
                total.cost_usd
            );
        }
        let after = Self::project_state()?;
        if before != after {
            bail!(
                "the agent changed files outside `.gewu/`; inspect them with `git status` and \
                 `git diff` before continuing"
            );
        }
        let prompt = runner.status()?;
        let refused = denied.join("\n- ");
        // A refused call the agent worked around is worth knowing; one that
        // stopped it is the reason it stopped.
        if !denied.is_empty() && Self::stopped(&prompt).is_some() {
            eprintln!("note: the agent was refused, and worked around:\n- {refused}");
        }
        match Self::stopped(&prompt) {
            Some(stop) => println!("{stop}"),
            None if !denied.is_empty() => bail!(
                "the agent stopped before the reviewer gate after being refused:\n- {refused}\nfix \
                 the permissions or the brief, then run this again"
            ),
            None if status.success() => println!(
                "The agent stopped before the reviewer gate, at:\n{}\nRun the same command again \
                 to continue.",
                prompt.lines().next().unwrap_or_default()
            ),
            None => bail!("the agent exited with {status} before the reviewer gate"),
        }
        Ok(())
    }

    fn missing(&self) -> String {
        format!(
            "could not run the {:?} agent; is its CLI installed?",
            self.agent
        )
    }

    /// Runs the agent with its output captured, redrawing a progress line
    /// until it exits. Output is read on threads so a full pipe cannot stall
    /// the agent.
    fn run_watched(
        &self,
        command: &mut Command,
        seconds_per_batch: f64,
    ) -> Result<(std::process::ExitStatus, Vec<u8>)> {
        use std::io::Read;
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().with_context(|| self.missing())?;
        let drain = |stream: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                if let Some(mut stream) = stream {
                    let _ = stream.read_to_end(&mut bytes);
                }
                bytes
            })
        };
        let stdout = drain(
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
        );
        let stderr = drain(
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
        );
        let mut watch = Watch::new(seconds_per_batch);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            watch.tick();
            std::thread::sleep(std::time::Duration::from_secs(1));
        };
        watch.finish();
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        if !status.success() {
            eprint!("{}", String::from_utf8_lossy(&stderr));
        }
        Ok((status, stdout))
    }

    /// The estimate before a run: for a range not yet started, from its diff
    /// alone; for the current review, from its plan and recorded batches.
    fn print_estimate(&self) -> Result<()> {
        let estimate = if let Some(base) = &self.base {
            let start = StartArgs::new(base.clone(), self.head.clone(), self.working_tree);
            let (_, input, _) = start.begin()?;
            Estimate::new(&input, None, &[], self.priced())
        } else {
            let review_id = Self::review_id()
                .context("no review is started; pass `--base` to estimate a new one")?;
            self.estimate_for(&review_id)?
        };
        println!("{estimate}");
        Ok(())
    }

    fn estimate_for(&self, review_id: &str) -> Result<Estimate> {
        let input: gewu_review::ReviewInput = serde_json::from_slice(
            &std::fs::read(".gewu/review-input.json").context("no review manifest")?,
        )?;
        Ok(Estimate::new(
            &input,
            Estimate::remaining_items(Path::new(".")),
            &Usage::recorded(review_id),
            self.priced(),
        ))
    }

    /// Only Claude reports what a run cost.
    fn priced(&self) -> bool {
        matches!(self.agent, Agent::Claude)
    }

    fn review_id() -> Option<String> {
        let state: serde_json::Value =
            serde_json::from_slice(&std::fs::read(".gewu/harness/state.json").ok()?).ok()?;
        state["review_id"].as_str().map(str::to_owned)
    }

    /// Prints Claude's final message; returns the tool calls it was denied
    /// and what the run used.
    fn report(stdout: &[u8]) -> (Vec<String>, Option<Usage>) {
        let Ok(report) = serde_json::from_slice::<serde_json::Value>(stdout) else {
            print!("{}", String::from_utf8_lossy(stdout));
            return (Vec::new(), None);
        };
        if let Some(result) = report["result"].as_str() {
            println!("{result}\n");
        }
        let denied = report["permission_denials"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|denial| {
                let tool = denial["tool_name"].as_str().unwrap_or("tool");
                let input = denial["tool_input"]["command"]
                    .as_str()
                    .map_or_else(|| denial["tool_input"].to_string(), str::to_owned);
                format!("{tool}: {input}")
            })
            .collect();
        (denied, Usage::from_report(&report))
    }

    /// What to tell the reviewer when no agent work is due.
    fn stopped(prompt: &str) -> Option<String> {
        if prompt.starts_with(COMPLETE) {
            Some("The walkthrough is complete. `gewu review open` shows it.".to_owned())
        } else if prompt.starts_with(WAITING) {
            Some(
                "A batch is ready for you: read it with `gewu review open`, then run this command \
                 again for the next one."
                    .to_owned(),
            )
        } else {
            None
        }
    }

    fn agent_command(&self) -> Result<Command> {
        let gewu = std::env::current_exe().context("could not locate the gewu binary")?;
        let gewu = gewu.to_string_lossy();
        let brief = BRIEF.replace("{{gewu}}", &gewu);
        let (program, mut args): (&str, Vec<String>) = match self.agent {
            Agent::Claude => (
                "claude",
                vec![
                    "-p".into(),
                    brief,
                    "--output-format".into(),
                    "json".into(),
                    "--permission-mode".into(),
                    "dontAsk".into(),
                    "--allowedTools".into(),
                    "Read".into(),
                    "Grep".into(),
                    "Glob".into(),
                    "Write(.gewu/harness/**)".into(),
                    "Edit(.gewu/harness/**)".into(),
                ]
                .into_iter()
                .chain(Self::spellings(&gewu).into_iter().flat_map(|binary| {
                    [
                        format!("Bash({binary} harness:*)"),
                        format!("Bash({binary} review:*)"),
                    ]
                }))
                .collect(),
            ),
            Agent::Codex => (
                "codex",
                vec![
                    "exec".into(),
                    "--sandbox".into(),
                    "workspace-write".into(),
                    brief,
                ],
            ),
        };
        if let Some(model) = &self.model {
            args.splice(1..1, ["--model".to_owned(), model.clone()]);
        }
        let mut command = Command::new(self.agent_program.as_deref().unwrap_or(Path::new(program)));
        command.args(args);
        Ok(command)
    }

    /// Ways an agent writes the binary's path. Self-review found agents
    /// shortening the absolute path they were given to `./.gewu/…/gewu` or a
    /// bare `gewu`, which a rule naming only the absolute path then denied.
    fn spellings(gewu: &str) -> Vec<String> {
        let mut spellings = vec![gewu.to_owned(), "gewu".to_owned()];
        if let Ok(root) = std::env::current_dir()
            && let Ok(relative) = Path::new(gewu).strip_prefix(&root)
        {
            let relative = relative.to_string_lossy();
            spellings.push(relative.to_string());
            spellings.push(format!("./{relative}"));
        }
        spellings
    }

    /// Everything outside `.gewu/` that an agent could change, as one digest:
    /// the status list, the diff, and the contents of untracked files.
    fn project_state() -> Result<String> {
        let git = |args: &[&str]| -> Result<Vec<u8>> {
            let output = Command::new("git")
                .args(args)
                .output()
                .context("could not run git")?;
            if !output.status.success() {
                bail!("git failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Ok(output.stdout)
        };
        let outside = ["--", ".", ":(exclude,top).gewu"];
        let mut digest = Sha256::new();
        digest.update(git(
            &[&["status", "--porcelain", "-z"][..], &outside].concat()
        )?);
        digest.update(git(&[&["diff", "--binary"][..], &outside].concat())?);
        let untracked = git(&[
            &["ls-files", "-o", "--exclude-standard", "-z"][..],
            &outside,
        ]
        .concat())?;
        for name in untracked
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let path = Path::new(std::str::from_utf8(name)?);
            digest.update(name);
            if path.is_file() {
                digest.update(std::fs::read(path)?);
            }
        }
        Ok(hex::encode(digest.finalize()))
    }
}
