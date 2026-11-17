//! `gewu review generate`: an agent writes the walkthrough, one batch at a time.
//!
//! Gewu starts or continues the harness, runs the agent the reviewer named,
//! and stops where the harness stops: at the reviewer gate. Running it again
//! after reading a batch continues. The agent may write only harness files;
//! anything else it changes is reported, because a walkthrough of code its
//! author edited is a walkthrough of nothing.

use gewu_prompts::{BACKEND_ENV, Prompt};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use sha2::{Digest, Sha256};

use gewu_git::Repository;

use gewu_harness::{
    DEFAULT_MAX_CHUNKS, HarnessRunner,
    estimate::{self, Estimate, Usage},
};

use super::{StartArgs, watch::Watch};

const WAITING: &str = "# Gewu harness: waiting for the reviewer";
const COMPLETE: &str = "# COMPLETE";
const PROOFREAD: &str = "# Gewu harness: proofread";
const POLISH: &str = "# Gewu harness: polish";

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Agent {
    /// Claude Code, restricted to reading and to writing harness files.
    Claude,
    /// Codex, in its workspace-write sandbox.
    Codex,
}

impl Agent {
    /// The agent preset whose prompt additions this agent gets.
    fn backend(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
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
    /// Review this GitHub pull request: its head is fetched, and its base is
    /// where it branched off its target. Running it again continues.
    #[arg(long, conflicts_with_all = ["base", "working_tree"])]
    pr: Option<u64>,
    /// The remote a pull request is fetched from.
    #[arg(long, default_value = "origin")]
    remote: String,
    #[arg(long)]
    model: Option<String>,
    /// Print how many batches are left and roughly what they will cost,
    /// without starting a review or running the agent.
    #[arg(long)]
    estimate: bool,
    /// How many times a fresh agent may send a batch back for rewriting after
    /// scoring it against the rubric, before the reviewer reads it anyway.
    /// Applies when a review starts; zero skips proofreading.
    #[arg(long, default_value_t = 2)]
    proofread_rounds: u32,
    /// Run this program in place of the agent's CLI, with the same arguments.
    #[arg(long, hide = true)]
    agent_program: Option<PathBuf>,
}

impl GenerateArgs {
    /// `gewu review pr <number>`: the same command with the request named.
    #[must_use]
    pub fn for_pull_request(mut self, number: u64) -> Self {
        self.pr = Some(number);
        self
    }

    pub fn run(self) -> Result<()> {
        let runner =
            HarnessRunner::new(DEFAULT_MAX_CHUNKS, false).with_proofread(self.proofread_rounds);
        let requested = self.requested()?;
        if self.estimate {
            return self.print_estimate(requested);
        }
        if let Some(start) = requested {
            let (review_id, input, origin) = start.begin()?;
            // Asking again for the review in progress continues it: a
            // reviewer re-runs the same command after reading each batch.
            if Self::in_progress(&input, origin.working_tree) {
                println!("Continuing the review already started for this range.");
            } else {
                runner.start(review_id, input, origin)?;
            }
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
        // A batch is every run it took, recorded as one, so the estimate's
        // per-batch average includes proofreading.
        let mut spent = Vec::new();
        let batch = self.batch(&runner, &estimate, &mut spent);
        if !spent.is_empty() {
            let usage = Usage::total(&spent);
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
        let (status, denied, prompt) = batch?;
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

    /// Runs the author, then a fresh agent for each proofreading pass, until
    /// the harness leaves proofreading or the rounds are spent. Returns how
    /// the last run exited, what it was refused, and where the harness is.
    fn batch(
        &self,
        runner: &HarnessRunner,
        estimate: &Estimate,
        spent: &mut Vec<Usage>,
    ) -> Result<(std::process::ExitStatus, Vec<String>, String)> {
        let (mut status, mut denied) = self.agent_run(estimate, spent)?;
        let mut prompt = runner.status()?;
        // The author stops where proofreading begins: a walkthrough reads as
        // complete to whoever wrote it, so each reading is a fresh session.
        // Each rewrite needs one, and the last reading one more.
        for _ in 0..=self.proofread_rounds {
            let proofreading = prompt.starts_with(PROOFREAD) || prompt.starts_with(POLISH);
            if !proofreading || !status.success() || !denied.is_empty() {
                break;
            }
            println!("A fresh agent proofreads the batch against the rubric.");
            (status, denied) = self.agent_run(estimate, spent)?;
            prompt = runner.status()?;
        }
        Ok((status, denied, prompt))
    }

    /// Runs the agent once, adds what it cost to `spent`, and refuses to
    /// continue if it changed anything outside `.gewu/`. Returns how it exited
    /// and the tool calls it was refused.
    fn agent_run(
        &self,
        estimate: &Estimate,
        spent: &mut Vec<Usage>,
    ) -> Result<(std::process::ExitStatus, Vec<String>)> {
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
        spent.extend(usage);
        let after = Self::project_state()?;
        if before != after {
            bail!(
                "the agent changed files outside `.gewu/`; inspect them with `git status` and \
                 `git diff` before continuing"
            );
        }
        Ok((status, denied))
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
    /// The range the reviewer named, if any, with a pull request resolved.
    fn requested(&self) -> Result<Option<StartArgs>> {
        if let Some(number) = self.pr {
            let request = Repository::new(Path::new(".")).pull_request(&self.remote, number)?;
            let title = request
                .title
                .as_deref()
                .map_or_else(String::new, |title| format!(" “{title}”"));
            println!(
                "Pull request #{number}{title}: {} into {}, from merge base {}.",
                &request.head[..request.head.len().min(12)],
                request.target,
                &request.base[..request.base.len().min(12)]
            );
            return Ok(Some(StartArgs::new(
                request.base,
                Some(request.head),
                false,
            )));
        }
        Ok(self
            .base
            .clone()
            .map(|base| StartArgs::new(base, self.head.clone(), self.working_tree)))
    }

    /// Whether the review in `.gewu/` is already of this range. A working-tree
    /// review re-diffs as the reviewer edits, so its base is what matters.
    fn in_progress(input: &gewu_review::ReviewInput, working_tree: bool) -> bool {
        let Ok(bytes) = std::fs::read(".gewu/review-input.json") else {
            return false;
        };
        let Ok(current) = serde_json::from_slice::<gewu_review::ReviewInput>(&bytes) else {
            return false;
        };
        let state_working_tree = std::fs::read(".gewu/harness/state.json")
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|state| state["origin"]["working_tree"] == true);
        if working_tree {
            state_working_tree && current.source.base == input.source.base
        } else {
            !state_working_tree && current.source == input.source
        }
    }

    fn print_estimate(&self, requested: Option<StartArgs>) -> Result<()> {
        let estimate = if let Some(start) = requested {
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
        let backend = self.agent.backend();
        let brief = Prompt::Agent
            .text_for(Some(backend))
            .replace("{{gewu}}", &gewu);
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
        // The harness prompts the agent fetches carry the same backend's additions.
        command.args(args).env(BACKEND_ENV, backend);
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
