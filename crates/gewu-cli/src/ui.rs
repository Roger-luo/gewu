use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use gewu_http::{ServeOptions, Server};
use gewu_workbench::Home;

#[derive(Debug, Subcommand)]
pub enum UiCommand {
    /// Serve the desktop web UI and its app protocol over loopback HTTP.
    Serve(ServeArgs),
}

impl UiCommand {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Serve(args) => args.run(),
        }
    }
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Serve only the prepared review in this workspace, which holds
    /// `.gewu/review-input.json` and `.gewu/review-spec.json`. Without it the
    /// full workbench is served: repositories, review tasks, and agents, kept
    /// under `$GEWU_HOME` (default `~/.gewu`).
    #[arg(long)]
    root: Option<PathBuf>,
    /// Built web UI to serve, from `pnpm --dir apps/desktop build`.
    #[arg(long = "static", default_value = "apps/desktop/dist")]
    static_dir: PathBuf,
    /// Loopback port; `0` picks a free one.
    #[arg(long, default_value_t = 4317)]
    port: u16,
}

impl ServeArgs {
    fn run(self) -> Result<()> {
        let options = match self.root {
            Some(root) => ServeOptions::new(root, self.static_dir, self.port),
            None => {
                let gewu = std::env::current_exe().context("could not locate the gewu binary")?;
                let home = Home::from_env();
                ServeOptions::workbench(home.root(), gewu, self.static_dir, self.port)
            }
        };
        let server = Server::start(options)?;
        println!("Serving Gewu at http://{}", server.local_addr());
        server.run()?;
        Ok(())
    }
}
