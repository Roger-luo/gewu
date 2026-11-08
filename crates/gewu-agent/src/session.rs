//! One ACP session: start the agent in the project, hand it the harness
//! brief, answer its requests under the [`Policy`], and stream its updates.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Client, LineDirection,
    schema::ProtocolVersion,
    v1::schema::{
        CancelNotification, ClientCapabilities, ContentBlock, CreateTerminalRequest,
        CreateTerminalResponse, FileSystemCapabilities, Implementation, InitializeRequest,
        KillTerminalRequest, KillTerminalResponse, NewSessionRequest, PermissionOptionKind,
        PromptRequest, ReadTextFileRequest, ReadTextFileResponse, ReleaseTerminalRequest,
        ReleaseTerminalResponse, RequestPermissionOutcome, RequestPermissionRequest,
        RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
        StopReason, TerminalId, TerminalOutputRequest, TerminalOutputResponse, TextContent,
        ToolCallStatus, WaitForTerminalExitRequest, WaitForTerminalExitResponse,
        WriteTextFileRequest, WriteTextFileResponse,
    },
};
use tokio::sync::watch;

use crate::{AgentError, AgentEvent, EventKind, EventSink, Policy, Preset, terminal::Terminals};

const BRIEF: &str = include_str!("../../../harness/prompts/agent.md");
/// Agent stderr lines kept for an error message.
const STDERR_TAIL: usize = 40;
/// How long a cancelled agent has to end its turn before it is killed.
const CANCEL_GRACE: Duration = Duration::from_secs(5);

/// Stops a running session from another thread.
#[derive(Clone, Debug)]
pub struct Cancel(Arc<watch::Sender<bool>>);

impl Default for Cancel {
    fn default() -> Self {
        Self(Arc::new(watch::channel(false).0))
    }
}

impl Cancel {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }

    async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

/// How a session ended.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// `end_turn`, `cancelled`, `refusal`, `max_tokens`, or `max_turn_requests`.
    pub stop_reason: String,
    /// Requests refused under the policy, in order.
    pub denials: Vec<String>,
    /// The agent's last message to the reviewer.
    pub message: String,
}

/// An agent command, the project it works in, and the brief it is given.
pub struct AcpRun {
    command: Vec<String>,
    prompt: String,
    policy: Policy,
}

/// Shared between the handlers of one session.
#[derive(Clone)]
struct Context {
    policy: Policy,
    sink: EventSink,
    denials: Arc<Mutex<Vec<String>>>,
    message: Arc<Mutex<String>>,
    terminals: Terminals,
}

impl Context {
    fn emit(&self, kind: EventKind, text: impl Into<String>) {
        (self.sink)(AgentEvent::new(kind, text));
    }

    fn deny(&self, reason: String) -> agent_client_protocol::Error {
        self.emit(EventKind::Denied, &reason);
        if let Ok(mut denials) = self.denials.lock() {
            denials.push(reason.clone());
        }
        agent_client_protocol::Error::new(-32602, reason)
    }

    fn update(&self, update: SessionUpdate) {
        match update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                if let ContentBlock::Text(text) = chunk.content {
                    if let Ok(mut message) = self.message.lock() {
                        message.push_str(&text.text);
                    }
                    self.emit(EventKind::Output, text.text);
                }
            }
            SessionUpdate::ToolCall(call) => {
                // A new tool call ends the message before it.
                if let Ok(mut message) = self.message.lock() {
                    message.clear();
                }
                self.emit(EventKind::Tool, self.policy.shorten(&call.title));
            }
            SessionUpdate::ToolCallUpdate(update)
                if update.fields.status == Some(ToolCallStatus::Failed) =>
            {
                let title = update
                    .fields
                    .title
                    .unwrap_or_else(|| "tool call".to_owned());
                self.emit(
                    EventKind::Error,
                    format!("failed: {}", self.policy.shorten(&title)),
                );
            }
            _ => {}
        }
    }

    fn permission(&self, request: &RequestPermissionRequest) -> RequestPermissionResponse {
        let wanted = match self.policy.check_tool(&request.tool_call.fields) {
            Ok(()) => [
                PermissionOptionKind::AllowOnce,
                PermissionOptionKind::AllowAlways,
            ],
            Err(reason) => {
                self.deny(reason);
                [
                    PermissionOptionKind::RejectOnce,
                    PermissionOptionKind::RejectAlways,
                ]
            }
        };
        let chosen = wanted.iter().find_map(|kind| {
            request
                .options
                .iter()
                .find(|option| option.kind == *kind)
                .map(|option| option.option_id.clone())
        });
        RequestPermissionResponse::new(chosen.map_or(RequestPermissionOutcome::Cancelled, |id| {
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(id))
        }))
    }

    fn read(
        &self,
        request: &ReadTextFileRequest,
    ) -> Result<ReadTextFileResponse, agent_client_protocol::Error> {
        let path = self
            .policy
            .check_read(&request.path)
            .map_err(|reason| self.deny(reason))?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            agent_client_protocol::Error::new(-32603, format!("{}: {error}", path.display()))
        })?;
        let start = request
            .line
            .map_or(0, |line| line.saturating_sub(1) as usize);
        let text = match (start, request.limit) {
            (0, None) => text,
            (start, limit) => text
                .lines()
                .skip(start)
                .take(limit.map_or(usize::MAX, |limit| limit as usize))
                .collect::<Vec<_>>()
                .join("\n"),
        };
        Ok(ReadTextFileResponse::new(text))
    }

    fn write(
        &self,
        request: &WriteTextFileRequest,
    ) -> Result<WriteTextFileResponse, agent_client_protocol::Error> {
        let path = self
            .policy
            .check_write(&request.path)
            .map_err(|reason| self.deny(reason))?;
        let internal = |error: std::io::Error| {
            agent_client_protocol::Error::new(-32603, format!("{}: {error}", path.display()))
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(internal)?;
        }
        std::fs::write(&path, &request.content).map_err(internal)?;
        Ok(WriteTextFileResponse::new())
    }

    fn terminal(
        &self,
        request: &CreateTerminalRequest,
    ) -> Result<CreateTerminalResponse, agent_client_protocol::Error> {
        self.policy
            .check_command(&request.command, &request.args)
            .map_err(|reason| self.deny(reason))?;
        let root = self.policy.root().to_path_buf();
        let cwd = match &request.cwd {
            Some(cwd) => self
                .policy
                .check_read(cwd)
                .map_err(|reason| self.deny(reason))?,
            None => root,
        };
        let id = self
            .terminals
            .create(
                &request.command,
                &request.args,
                &cwd,
                request.output_byte_limit,
            )
            .map_err(|error| agent_client_protocol::Error::new(-32603, error.to_string()))?;
        Ok(CreateTerminalResponse::new(TerminalId::new(id)))
    }
}

fn unknown_terminal() -> agent_client_protocol::Error {
    agent_client_protocol::Error::new(-32602, "unknown terminal")
}

impl AcpRun {
    /// `command` starts the agent; `gewu` is the binary the agent will run
    /// for harness commands.
    pub fn new(root: &Path, command: Vec<String>, gewu: &Path) -> Self {
        let prompt = BRIEF.replace("{{gewu}}", &gewu.to_string_lossy());
        Self {
            command,
            prompt,
            policy: Policy::new(root, gewu),
        }
    }

    /// Run one batch to the end of the agent's turn, blocking this thread.
    pub fn run(self, sink: EventSink, cancel: &Cancel) -> Result<Outcome, AgentError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(self.session(sink, cancel))
    }

    /// `PATH` with the places agent CLIs are usually installed, because a
    /// desktop app started from the Dock inherits almost no `PATH`.
    #[must_use]
    pub fn search_path() -> std::ffi::OsString {
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for extra in [
                ".local/bin",
                ".cargo/bin",
                ".local/share/mise/shims",
                ".bun/bin",
            ] {
                dirs.push(home.join(extra));
            }
        }
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
        std::env::join_paths(dirs).unwrap_or_default()
    }

    async fn session(self, sink: EventSink, cancel: &Cancel) -> Result<Outcome, AgentError> {
        let (program, args) = self.command.split_first().ok_or(AgentError::NoCommand)?;
        let path = Self::search_path();
        // Resolved here so a missing CLI is a clear error rather than a
        // shell's "command not found" on stderr.
        let resolved = Preset::which_in(program, &path).ok_or_else(|| AgentError::Spawn {
            program: program.clone(),
            reason: "not found on PATH".to_owned(),
        })?;
        let root = self.policy.root().to_path_buf();
        // The SDK starts agents in this process's directory; the shell moves
        // each one into its own project first.
        let config = AcpAgentConfig::new("/bin/sh")
            .args(
                [
                    "-c".to_owned(),
                    "cd \"$0\" && exec \"$@\"".to_owned(),
                    root.to_string_lossy().into_owned(),
                    resolved.to_string_lossy().into_owned(),
                ]
                .into_iter()
                .chain(args.iter().cloned()),
            )
            .env("PATH", path.to_string_lossy());
        let stderr = Arc::new(Mutex::new(VecDeque::new()));
        let agent = AcpAgent::new(config).with_debug({
            let stderr = Arc::clone(&stderr);
            move |line, direction| {
                if matches!(direction, LineDirection::Stderr)
                    && let Ok(mut tail) = stderr.lock()
                {
                    if tail.len() == STDERR_TAIL {
                        tail.pop_front();
                    }
                    tail.push_back(line.to_owned());
                }
            }
        });
        let context = Context {
            policy: self.policy,
            sink,
            denials: Arc::default(),
            message: Arc::default(),
            terminals: Terminals::default(),
        };
        let result = Self::connect(agent, context.clone(), root, self.prompt, cancel).await;
        context.terminals.release_all();
        let stop_reason = result.map_err(|error| match error {
            AgentError::Protocol(message) => {
                let tail = stderr
                    .lock()
                    .map(|tail| tail.iter().cloned().collect::<Vec<_>>().join("\n"))
                    .unwrap_or_default();
                let stopped = AgentError::stopped(&message, &tail);
                context.emit(EventKind::Error, stopped.to_string());
                stopped
            }
            other => other,
        })?;
        Ok(Outcome {
            stop_reason,
            denials: context
                .denials
                .lock()
                .map(|d| d.clone())
                .unwrap_or_default(),
            message: context
                .message
                .lock()
                .map(|m| m.trim().to_owned())
                .unwrap_or_default(),
        })
    }

    async fn connect(
        agent: AcpAgent,
        context: Context,
        root: PathBuf,
        prompt: String,
        cancel: &Cancel,
    ) -> Result<String, AgentError> {
        let on_update = context.clone();
        let on_permission = context.clone();
        let on_read = context.clone();
        let on_write = context.clone();
        let on_create = context.clone();
        let on_output = context.terminals.clone();
        let on_wait = context.terminals.clone();
        let on_kill = context.terminals.clone();
        let on_release = context.terminals.clone();
        let reason = Client
            .v1()
            .name("gewu")
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    on_update.update(notification.update);
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: RequestPermissionRequest, responder, _cx| {
                    responder.respond(on_permission.permission(&request))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: ReadTextFileRequest, responder, _cx| {
                    responder.respond_with_result(on_read.read(&request))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: WriteTextFileRequest, responder, _cx| {
                    responder.respond_with_result(on_write.write(&request))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: CreateTerminalRequest, responder, _cx| {
                    responder.respond_with_result(on_create.terminal(&request))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: TerminalOutputRequest, responder, _cx| {
                    responder.respond_with_result(
                        on_output
                            .output(&request.terminal_id.0)
                            .map(|(output, truncated, exit)| {
                                TerminalOutputResponse::new(output, truncated).exit_status(exit)
                            })
                            .ok_or_else(unknown_terminal),
                    )
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: WaitForTerminalExitRequest, responder, cx| {
                    let Some(mut exit) = on_wait.exit(&request.terminal_id.0) else {
                        return responder.respond_with_error(unknown_terminal());
                    };
                    // Waiting here would hold every other message, so the
                    // wait runs beside the dispatch loop.
                    cx.spawn(async move {
                        let status = exit
                            .wait_for(Option::is_some)
                            .await
                            .ok()
                            .and_then(|status| status.clone());
                        match status {
                            Some(status) => {
                                responder.respond(WaitForTerminalExitResponse::new(status))
                            }
                            None => responder.respond_with_error(unknown_terminal()),
                        }
                    })
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: KillTerminalRequest, responder, _cx| {
                    on_kill.kill(&request.terminal_id.0);
                    responder.respond(KillTerminalResponse::new())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: ReleaseTerminalRequest, responder, _cx| {
                    on_release.release(&request.terminal_id.0);
                    responder.respond(ReleaseTerminalResponse::new())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, async |cx| {
                let capabilities = ClientCapabilities::new()
                    .fs(FileSystemCapabilities::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true);
                let init = cx
                    .send_request(
                        InitializeRequest::new(ProtocolVersion::V1)
                            .client_capabilities(capabilities)
                            .client_info(Implementation::new("gewu", env!("CARGO_PKG_VERSION"))),
                    )
                    .block_task()
                    .await?;
                if let Some(info) = &init.agent_info {
                    context.emit(
                        EventKind::Status,
                        format!("connected to {} {}", info.name, info.version),
                    );
                }
                let session = cx
                    .send_request(NewSessionRequest::new(root))
                    .block_task()
                    .await?;
                let session_id = session.session_id;
                let prompt = cx
                    .send_request(PromptRequest::new(
                        session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new(prompt))],
                    ))
                    .block_task();
                tokio::pin!(prompt);
                let response = tokio::select! {
                    response = &mut prompt => response?,
                    () = cancel.cancelled() => {
                        context.emit(EventKind::Status, "cancelling the agent");
                        cx.send_notification(CancelNotification::new(session_id))?;
                        match tokio::time::timeout(CANCEL_GRACE, &mut prompt).await {
                            Ok(response) => response?,
                            Err(_) => return Ok(StopReason::Cancelled),
                        }
                    }
                };
                Ok(response.stop_reason)
            })
            .await?;
        Ok(Self::stop_name(reason))
    }

    fn stop_name(reason: StopReason) -> String {
        serde_json::to_value(reason)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned())
    }
}
