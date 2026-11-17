//! One ACP session: start the agent in the project, hand it the harness
//! brief, answer its requests under the [`Policy`], and stream its updates.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Client, ErrorCode, LineDirection,
    schema::ProtocolVersion,
    v1::schema::{
        AuthMethod, AuthenticateRequest, CancelNotification, ClientCapabilities, ContentBlock,
        CreateTerminalRequest, CreateTerminalResponse, FileSystemCapabilities, Implementation,
        InitializeRequest, KillTerminalRequest, KillTerminalResponse, NewSessionRequest,
        PermissionOptionKind, PromptRequest, ReadTextFileRequest, ReadTextFileResponse,
        ReleaseTerminalRequest, ReleaseTerminalResponse, RequestPermissionOutcome,
        RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
        SessionNotification, SessionUpdate, StopReason, TerminalId, TerminalOutputRequest,
        TerminalOutputResponse, TextContent, ToolCallStatus, WaitForTerminalExitRequest,
        WaitForTerminalExitResponse, WriteTextFileRequest, WriteTextFileResponse,
    },
};
use tokio::sync::watch;

use gewu_prompts::{BACKEND_ENV, Prompt};

use crate::{AgentError, AgentEvent, EventKind, EventSink, Policy, Preset, terminal::Terminals};

/// Agent stderr lines kept for an error message.
const STDERR_TAIL: usize = 40;
/// How long a cancelled agent has to end its turn before it is killed.
const CANCEL_GRACE: Duration = Duration::from_secs(5);
/// How long the agent may take to start, sign in, and open a session before
/// it is started again. Healthy Cursor handshakes in a live run of ppvm #276
/// took seconds; four hung ones went 25 minutes each and the CLI then exited
/// silently, having never opened a session. Nothing is sent before the
/// session opens, so starting again loses no work.
const HANDSHAKE_LIMIT: Duration = Duration::from_secs(120);
/// Starts allowed when the handshake hangs.
pub(crate) const HANDSHAKE_ATTEMPTS: u32 = 3;

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
#[derive(Clone)]
pub struct AcpRun {
    command: Vec<String>,
    pub(crate) prompt: String,
    /// What a session that already holds the brief is sent on a later turn.
    pub(crate) follow_up: String,
    /// What an agent that stopped after a refusal is told; `{{refused}}`
    /// takes the requests.
    pub(crate) refused: String,
    policy: Policy,
    handshake_limit: Duration,
    /// The agent preset this run is, so prompts carry its additions.
    backend: Option<String>,
    gewu: PathBuf,
}

/// Shared between the handlers of one session.
#[derive(Clone)]
struct Context {
    policy: Policy,
    sink: EventSink,
    denials: Arc<Mutex<Vec<String>>>,
    message: Arc<Mutex<String>>,
    terminals: Terminals,
    /// Set when the handshake ran past its limit.
    stalled: Arc<std::sync::atomic::AtomicBool>,
}

/// One prompt for a running session, and where its outcome goes.
pub(crate) struct Turn {
    pub(crate) prompt: String,
    pub(crate) cancel: Cancel,
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<Outcome, AgentError>>,
}

impl Context {
    /// Denials and the last message are per turn.
    fn begin_turn(&self) {
        if let Ok(mut denials) = self.denials.lock() {
            denials.clear();
        }
        if let Ok(mut message) = self.message.lock() {
            message.clear();
        }
    }

    fn outcome(&self, stop_reason: String) -> Outcome {
        Outcome {
            stop_reason,
            denials: self.denials.lock().map(|d| d.clone()).unwrap_or_default(),
            message: self
                .message
                .lock()
                .map(|m| m.trim().to_owned())
                .unwrap_or_default(),
        }
    }

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
        // Claude Code's adapter sends a whole shell line as the command,
        // with no arguments. Found by a live run: every harness command was
        // refused as not the Gewu binary.
        let (program, args) =
            if request.args.is_empty() && request.command.contains(char::is_whitespace) {
                self.policy.shell_command(&request.command)
            } else {
                self.policy
                    .check_command(&request.command, &request.args)
                    .map(|()| (request.command.clone(), request.args.clone()))
            }
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
            .create(&program, &args, &cwd, request.output_byte_limit)
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
        let mut run = Self {
            command,
            prompt: String::new(),
            follow_up: String::new(),
            refused: String::new(),
            policy: Policy::new(root, gewu),
            handshake_limit: HANDSHAKE_LIMIT,
            backend: None,
            gewu: gewu.to_path_buf(),
        };
        run.render_prompts();
        run
    }

    /// Names the agent preset this run is (`claude`, `codex`, …). Its
    /// prompts carry that backend's additions, and so do the harness prompts
    /// the agent fetches: the preset is set in [`BACKEND_ENV`] for the agent
    /// and every command it runs.
    #[must_use]
    pub fn with_backend(mut self, backend: Option<String>) -> Self {
        self.backend = backend;
        self.render_prompts();
        self
    }

    fn render_prompts(&mut self) {
        let gewu = self.gewu.to_string_lossy();
        let backend = self.backend.as_deref();
        let render = |prompt: Prompt| prompt.text_for(backend).replace("{{gewu}}", &gewu);
        self.prompt = render(Prompt::Agent);
        self.follow_up = render(Prompt::Continue);
        self.refused = render(Prompt::Refused);
    }

    fn backend_env(&self) -> Vec<(String, String)> {
        self.backend
            .iter()
            .map(|backend| (BACKEND_ENV.to_owned(), backend.clone()))
            .collect()
    }

    /// How long the handshake may take before the agent is started again.
    #[must_use]
    pub fn with_handshake_limit(mut self, limit: Duration) -> Self {
        self.handshake_limit = limit;
        self
    }

    /// Run one batch to the end of the agent's turn, blocking this thread.
    pub fn run(self, sink: EventSink, cancel: &Cancel) -> Result<Outcome, AgentError> {
        crate::AgentSession::new(self, sink).turn(cancel)
    }

    pub(crate) fn handshake_limit(&self) -> Duration {
        self.handshake_limit
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

    /// Starts the agent and serves `turns` on one session until the sender
    /// is dropped or the agent stops. A turn's reply is dropped unsent when
    /// the agent fails during it; the error is this function's result.
    pub(crate) async fn serve(
        self,
        sink: EventSink,
        turns: tokio::sync::mpsc::UnboundedReceiver<Turn>,
    ) -> Result<(), AgentError> {
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
        let env = self.backend_env();
        let config = env
            .iter()
            .fold(config, |config, (key, value)| config.env(key, value));
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
            terminals: Terminals::with_env(env),
            stalled: Arc::default(),
        };
        let limit = self.handshake_limit;
        let result = Self::connect(agent, context.clone(), root, limit, turns).await;
        if context.stalled.load(std::sync::atomic::Ordering::SeqCst) {
            context.terminals.release_all();
            let stalled = AgentError::Unresponsive {
                waited: limit.as_secs(),
            };
            context.emit(EventKind::Error, stalled.to_string());
            return Err(stalled);
        }
        context.terminals.release_all();
        result.map_err(|error| match error {
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
        })
    }

    async fn connect(
        agent: AcpAgent,
        context: Context,
        root: PathBuf,
        handshake_limit: Duration,
        mut turns: tokio::sync::mpsc::UnboundedReceiver<Turn>,
    ) -> Result<(), AgentError> {
        let on_update = context.clone();
        let on_permission = context.clone();
        let on_read = context.clone();
        let on_write = context.clone();
        let on_create = context.clone();
        let on_output = context.terminals.clone();
        let on_wait = context.terminals.clone();
        let on_kill = context.terminals.clone();
        let on_release = context.terminals.clone();
        Client
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
                let mut opened: Option<agent_client_protocol::v1::schema::SessionId> = None;
                while let Some(turn) = turns.recv().await {
                    context.begin_turn();
                    let session_id = if let Some(id) = &opened {
                        id.clone()
                    } else {
                        let id = Self::handshake(&cx, &context, &root, handshake_limit).await?;
                        opened = Some(id.clone());
                        id
                    };
                    let prompt = cx
                        .send_request(PromptRequest::new(
                            session_id.clone(),
                            vec![ContentBlock::Text(TextContent::new(turn.prompt))],
                        ))
                        .block_task();
                    tokio::pin!(prompt);
                    let (stop, abandoned) = tokio::select! {
                        response = &mut prompt => (response?.stop_reason, false),
                        () = turn.cancel.cancelled() => {
                            context.emit(EventKind::Status, "cancelling the agent");
                            cx.send_notification(CancelNotification::new(session_id))?;
                            match tokio::time::timeout(CANCEL_GRACE, &mut prompt).await {
                                Ok(response) => (response?.stop_reason, false),
                                Err(_) => (StopReason::Cancelled, true),
                            }
                        }
                    };
                    let _ = turn.reply.send(Ok(context.outcome(Self::stop_name(stop))));
                    // An agent that ignored the cancel is not given another turn.
                    if abandoned {
                        break;
                    }
                }
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Initialise, sign in if the agent asks, and open a session, within
    /// `limit`: a CLI that hangs here never opens a session, and is started
    /// again rather than waited on for its own timeout.
    async fn handshake(
        cx: &agent_client_protocol::v1::ConnectionTo<agent_client_protocol::Agent>,
        context: &Context,
        root: &Path,
        limit: Duration,
    ) -> Result<agent_client_protocol::v1::schema::SessionId, agent_client_protocol::Error> {
        let began = std::time::Instant::now();
        let handshake = async {
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
            let new_session = || {
                cx.send_request(NewSessionRequest::new(root.to_path_buf()))
                    .block_task()
            };
            // Some agents (Cursor's) refuse a session until the client
            // calls `authenticate` with a method they handle themselves,
            // even when their CLI is signed in. Sign in once and retry.
            let opened = match new_session().await {
                Err(error) if error.code == ErrorCode::AUTH_REQUIRED => {
                    let Some(method) = init
                        .auth_methods
                        .iter()
                        .find(|method| matches!(method, AuthMethod::Agent(_)))
                    else {
                        return Err(error);
                    };
                    context.emit(
                        EventKind::Status,
                        format!("signing in to the agent with {}", method.name()),
                    );
                    cx.send_request(AuthenticateRequest::new(method.id().clone()))
                        .block_task()
                        .await?;
                    new_session().await?
                }
                other => other?,
            };
            Ok::<_, agent_client_protocol::Error>(opened)
        };
        let Ok(session) = tokio::time::timeout(limit, handshake).await else {
            context
                .stalled
                .store(true, std::sync::atomic::Ordering::SeqCst);
            return Err(agent_client_protocol::Error::internal_error());
        };
        let session = session?;
        // Logged so a slow start can be told apart from a slow model.
        context.emit(
            EventKind::Status,
            format!("session open after {} s", began.elapsed().as_secs()),
        );
        Ok(session.session_id)
    }

    fn stop_name(reason: StopReason) -> String {
        serde_json::to_value(reason)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned())
    }
}
