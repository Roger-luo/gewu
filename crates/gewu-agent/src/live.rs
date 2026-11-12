//! An agent kept running across turns. Starting a fresh agent for every
//! batch cost a live run of ppvm #276 about half its writing time: each new
//! author re-read the description, the manifest, and the code for 1.4 to 6.4
//! minutes before its first write. One session keeps what it has read.

use std::thread::JoinHandle;

use tokio::sync::{mpsc, oneshot};

use crate::{
    AcpRun, AgentError, AgentEvent, Cancel, EventKind, EventSink, Outcome,
    session::{HANDSHAKE_ATTEMPTS, Turn},
};

/// Times an agent that stopped after a refusal is told to carry on.
pub const REFUSAL_RETRIES: usize = 3;

/// One agent process and its session, opened on the first turn and again
/// whenever it has stopped. Dropping it ends the agent.
pub struct AgentSession {
    run: AcpRun,
    sink: EventSink,
    live: Option<Live>,
}

struct Live {
    turns: mpsc::UnboundedSender<Turn>,
    thread: JoinHandle<Result<(), AgentError>>,
}

impl AgentSession {
    #[must_use]
    pub fn new(run: AcpRun, sink: EventSink) -> Self {
        Self {
            run,
            sink,
            live: None,
        }
    }

    /// Runs one turn to its end, blocking this thread. A new agent gets the
    /// brief; a running one is told to carry on. An agent that stopped since
    /// its last turn, or hangs before opening a session, is started again.
    ///
    /// An agent that ends its turn after a refused request is told what was
    /// refused and what it may do, and carries on, up to [`REFUSAL_RETRIES`]
    /// times. Found by running Claude Code: it takes a refusal for the
    /// reviewer saying no, and stops.
    pub fn turn(&mut self, cancel: &Cancel) -> Result<Outcome, AgentError> {
        let mut outcome = self.send(None, cancel)?;
        let mut retries = 0;
        while !outcome.denials.is_empty()
            && outcome.stop_reason != "cancelled"
            && !cancel.is_cancelled()
            && retries < REFUSAL_RETRIES
        {
            retries += 1;
            self.status(format!(
                "the agent stopped after a refused request; telling it what it may do \
                 (attempt {retries} of {REFUSAL_RETRIES})"
            ));
            let note = self
                .run
                .refused
                .replace("{{refused}}", &outcome.denials.join("\n- "));
            outcome = self.send(Some(note), cancel)?;
        }
        Ok(outcome)
    }

    /// One prompt to the agent: the brief when it is new, else `note`, else
    /// the follow-up.
    fn send(&mut self, note: Option<String>, cancel: &Cancel) -> Result<Outcome, AgentError> {
        let mut starts = 0;
        let mut reopened = false;
        loop {
            let fresh = self.live.is_none();
            if fresh {
                starts += 1;
                self.live = Some(Live::open(self.run.clone(), self.sink.clone()));
            }
            let prompt = match (&note, fresh) {
                (_, true) => self.run.prompt.clone(),
                (Some(note), false) => note.clone(),
                (None, false) => self.run.follow_up.clone(),
            };
            let (reply, outcome) = oneshot::channel();
            let sent = self.live.as_ref().is_some_and(|live| {
                live.turns
                    .send(Turn {
                        prompt,
                        cancel: cancel.clone(),
                        reply,
                    })
                    .is_ok()
            });
            if sent && let Ok(result) = outcome.blocking_recv() {
                return result;
            }
            let error = self.close();
            if cancel.is_cancelled() {
                return Err(error);
            }
            match error {
                AgentError::Unresponsive { .. } if starts < HANDSHAKE_ATTEMPTS => {
                    self.status(format!(
                        "the agent did not open a session within {} s; starting it again \
                         (attempt {} of {HANDSHAKE_ATTEMPTS})",
                        self.run.handshake_limit().as_secs(),
                        starts + 1
                    ));
                }
                // A session that served earlier turns may end while idle;
                // a new agent with the brief takes over, once.
                AgentError::Stopped { .. } if !fresh && !reopened => {
                    reopened = true;
                    self.status("the agent's session ended; starting a new one".to_owned());
                }
                error => return Err(error),
            }
        }
    }

    fn status(&self, text: String) {
        (self.sink)(AgentEvent::new(EventKind::Status, text));
    }

    /// Ends the agent and returns why it stopped.
    fn close(&mut self) -> AgentError {
        let Some(live) = self.live.take() else {
            return AgentError::Stopped { reason: None };
        };
        drop(live.turns);
        match live.thread.join() {
            Ok(Err(error)) => error,
            Ok(Ok(())) => AgentError::Stopped { reason: None },
            Err(_) => AgentError::Stopped {
                reason: Some("the session thread panicked".to_owned()),
            },
        }
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

impl Live {
    fn open(run: AcpRun, sink: EventSink) -> Self {
        let (turns, receiver) = mpsc::unbounded_channel();
        let thread = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(run.serve(sink, receiver))
        });
        Self { turns, thread }
    }
}
