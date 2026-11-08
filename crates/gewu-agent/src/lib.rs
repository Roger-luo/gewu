//! ACP client that runs a coding agent through the Gewu harness.
//!
//! Gewu speaks the [Agent Client Protocol](https://agentclientprotocol.com)
//! through the official SDK. An [`AcpRun`] starts any ACP agent in a
//! project, hands it the harness brief, and answers its file and terminal
//! requests under a [`Policy`]: read the project, write only under
//! `.gewu/harness/`, and run only this binary's harness commands. The
//! walkthrough itself is the files the harness validates, not the agent's
//! messages, so the session needs no structured-output contract.

mod error;
mod event;
mod policy;
mod preset;
mod session;
mod state;
mod terminal;

pub use error::AgentError;
pub use event::{AgentEvent, EventKind, EventSink};
pub use policy::Policy;
pub use preset::{PRESETS, Preset, PresetKind, PresetView};
pub use session::{AcpRun, Cancel, Outcome};
pub use state::ProjectState;
