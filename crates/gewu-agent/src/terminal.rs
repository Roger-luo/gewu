//! Client-side terminals for `terminal/*` requests. Only commands the
//! [`Policy`](crate::Policy) allows are started, always in the project root.

use std::{
    collections::HashMap,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use agent_client_protocol::v1::schema::TerminalExitStatus;
use tokio::sync::watch;

/// Output kept per terminal unless the agent asks for less.
const DEFAULT_LIMIT: usize = 256 * 1024;

struct Terminal {
    output: Arc<Mutex<String>>,
    limit: usize,
    child: Arc<Mutex<Child>>,
    exit: watch::Receiver<Option<TerminalExitStatus>>,
}

#[derive(Clone, Default)]
pub struct Terminals {
    next: Arc<Mutex<u64>>,
    open: Arc<Mutex<HashMap<String, Terminal>>>,
}

impl Terminals {
    pub fn create(
        &self,
        program: &str,
        args: &[String],
        cwd: &Path,
        limit: Option<u64>,
    ) -> std::io::Result<String> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let output = Arc::new(Mutex::new(String::new()));
        for stream in [
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let output = Arc::clone(&output);
            thread::spawn(move || Self::drain(stream, &output));
        }
        let child = Arc::new(Mutex::new(child));
        let (sender, exit) = watch::channel(None);
        {
            let child = Arc::clone(&child);
            thread::spawn(move || {
                loop {
                    let status = child
                        .lock()
                        .ok()
                        .and_then(|mut c| c.try_wait().ok().flatten());
                    if let Some(status) = status {
                        let _ = sender.send(Some(Self::exit_status(status)));
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            });
        }
        let id = {
            let mut next = self.next.lock().map_err(|_| poisoned())?;
            *next += 1;
            format!("term-{next}")
        };
        let terminal = Terminal {
            output,
            limit: limit.map_or(DEFAULT_LIMIT, |limit| limit as usize),
            child,
            exit,
        };
        self.open
            .lock()
            .map_err(|_| poisoned())?
            .insert(id.clone(), terminal);
        Ok(id)
    }

    fn drain(mut stream: Box<dyn Read + Send>, output: &Mutex<String>) {
        let mut buffer = [0_u8; 8192];
        while let Ok(read) = stream.read(&mut buffer) {
            if read == 0 {
                break;
            }
            if let Ok(mut output) = output.lock() {
                output.push_str(&String::from_utf8_lossy(&buffer[..read]));
            }
        }
    }

    fn exit_status(status: std::process::ExitStatus) -> TerminalExitStatus {
        let mut exit = TerminalExitStatus::new();
        exit.exit_code = status.code().and_then(|code| u32::try_from(code).ok());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            exit.signal = status.signal().map(|signal| signal.to_string());
        }
        exit
    }

    /// Current output (trimmed to the limit from the front), whether it was
    /// trimmed, and the exit status once the command has finished.
    pub fn output(&self, id: &str) -> Option<(String, bool, Option<TerminalExitStatus>)> {
        let open = self.open.lock().ok()?;
        let terminal = open.get(id)?;
        let output = terminal.output.lock().ok()?;
        let (text, truncated) = if output.len() > terminal.limit {
            let mut start = output.len() - terminal.limit;
            while !output.is_char_boundary(start) {
                start += 1;
            }
            (output[start..].to_owned(), true)
        } else {
            (output.clone(), false)
        };
        Some((text, truncated, terminal.exit.borrow().clone()))
    }

    /// Resolves when the command exits.
    pub fn exit(&self, id: &str) -> Option<watch::Receiver<Option<TerminalExitStatus>>> {
        Some(self.open.lock().ok()?.get(id)?.exit.clone())
    }

    pub fn kill(&self, id: &str) -> bool {
        let Ok(open) = self.open.lock() else {
            return false;
        };
        open.get(id)
            .and_then(|terminal| terminal.child.lock().ok())
            .is_some_and(|mut child| child.kill().is_ok() || child.try_wait().is_ok())
    }

    pub fn release(&self, id: &str) -> bool {
        self.kill(id);
        self.open
            .lock()
            .is_ok_and(|mut open| open.remove(id).is_some())
    }

    /// Stop every command still running; called when the session ends.
    pub fn release_all(&self) {
        if let Ok(mut open) = self.open.lock() {
            for (_, terminal) in open.drain() {
                if let Ok(mut child) = terminal.child.lock() {
                    let _ = child.kill();
                }
            }
        }
    }
}

fn poisoned() -> std::io::Error {
    std::io::Error::other("terminal registry lock poisoned")
}
