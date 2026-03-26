use agent_client_protocol::{
    self as acp, Agent as _, ContentBlock, ContentChunk, RequestPermissionOutcome,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
};
use models::{
    AcpConnectionInfo, AcpEvent, AcpLaunchRequest, AcpMessageKind, AcpPermissionOption,
    AcpPermissionRequest,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};
use tokio::{
    io::AsyncReadExt,
    process::Child,
    sync::{Mutex as AsyncMutex, mpsc as tokio_mpsc, oneshot},
};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

struct AcpRuntimeHandle {
    command_tx: tokio_mpsc::UnboundedSender<AcpCommand>,
    event_rx: mpsc::Receiver<AcpEvent>,
    permission_registry: PermissionRegistry,
    terminal_registry: TerminalRegistry,
    _thread: thread::JoinHandle<()>,
}

enum AcpCommand {
    Prompt(String),
    Cancel,
    Disconnect,
}

struct BridgeClient {
    event_tx: mpsc::Sender<AcpEvent>,
    permission_registry: PermissionRegistry,
    terminal_registry: TerminalRegistry,
    workspace_root: PathBuf,
}

type PermissionRegistry = Arc<PermissionRegistryInner>;
type TerminalRegistry = Arc<TerminalRegistryInner>;

struct PermissionRegistryInner {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<RequestPermissionResponse>>>,
}

struct TerminalRegistryInner {
    next_id: AtomicU64,
    terminals: Mutex<HashMap<String, Arc<TerminalState>>>,
}

struct TerminalState {
    child: AsyncMutex<Child>,
    output: Mutex<String>,
    output_limit: Option<usize>,
    truncated: Mutex<bool>,
    exit_status: Mutex<Option<acp::TerminalExitStatus>>,
}

impl PermissionRegistryInner {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    fn insert(&self, sender: oneshot::Sender<RequestPermissionResponse>) -> Result<u64, String> {
        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.pending
            .lock()
            .map_err(|_| "ACP permission registry lock poisoned".to_string())?
            .insert(request_id, sender);
        Ok(request_id)
    }

    fn take(
        &self,
        request_id: u64,
    ) -> Result<Option<oneshot::Sender<RequestPermissionResponse>>, String> {
        self.pending
            .lock()
            .map_err(|_| "ACP permission registry lock poisoned".to_string())
            .map(|mut pending| pending.remove(&request_id))
    }
}

impl TerminalRegistryInner {
    fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            terminals: Mutex::new(HashMap::new()),
        }
    }

    fn next_terminal_id(&self) -> String {
        format!("term-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    fn insert(&self, terminal_id: String, terminal: Arc<TerminalState>) -> Result<(), String> {
        self.terminals
            .lock()
            .map_err(|_| "ACP terminal registry lock poisoned".to_string())?
            .insert(terminal_id, terminal);
        Ok(())
    }

    fn get(&self, terminal_id: &str) -> Result<Option<Arc<TerminalState>>, String> {
        self.terminals
            .lock()
            .map_err(|_| "ACP terminal registry lock poisoned".to_string())
            .map(|terminals| terminals.get(terminal_id).cloned())
    }

    fn take(&self, terminal_id: &str) -> Result<Option<Arc<TerminalState>>, String> {
        self.terminals
            .lock()
            .map_err(|_| "ACP terminal registry lock poisoned".to_string())
            .map(|mut terminals| terminals.remove(terminal_id))
    }

    fn ids(&self) -> Result<Vec<String>, String> {
        self.terminals
            .lock()
            .map_err(|_| "ACP terminal registry lock poisoned".to_string())
            .map(|terminals| terminals.keys().cloned().collect())
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for BridgeClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<RequestPermissionResponse> {
        let (response_tx, response_rx) = oneshot::channel();
        let request_id = match self.permission_registry.insert(response_tx) {
            Ok(request_id) => request_id,
            Err(err) => {
                let _ = self.event_tx.send(AcpEvent::Error(err));
                return Ok(RequestPermissionResponse::new(
                    RequestPermissionOutcome::Cancelled,
                ));
            }
        };

        let permission_request = AcpPermissionRequest {
            request_id,
            tool_summary: format!("{:?}", args.tool_call),
            options: args
                .options
                .iter()
                .map(|option| AcpPermissionOption {
                    option_id: option.option_id.to_string(),
                    label: option.name.clone(),
                    kind: format!("{:?}", option.kind),
                })
                .collect(),
        };

        let _ = self
            .event_tx
            .send(AcpEvent::PermissionRequested(permission_request));

        match response_rx.await {
            Ok(response) => Ok(response),
            Err(_) => Ok(RequestPermissionResponse::new(
                RequestPermissionOutcome::Cancelled,
            )),
        }
    }

    async fn session_notification(&self, args: SessionNotification) -> acp::Result<()> {
        match args.update {
            SessionUpdate::UserMessageChunk(chunk) => {
                send_chunk(&self.event_tx, AcpMessageKind::User, chunk);
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                send_chunk(&self.event_tx, AcpMessageKind::Agent, chunk);
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                send_chunk(&self.event_tx, AcpMessageKind::Thought, chunk);
            }
            SessionUpdate::ToolCall(tool_call) => {
                let _ = self.event_tx.send(AcpEvent::Message {
                    kind: AcpMessageKind::Tool,
                    text: format!("Tool call: {tool_call:?}"),
                });
            }
            SessionUpdate::ToolCallUpdate(tool_update) => {
                let _ = self.event_tx.send(AcpEvent::Message {
                    kind: AcpMessageKind::Tool,
                    text: format!("Tool update: {tool_update:?}"),
                });
            }
            SessionUpdate::Plan(plan) => {
                let _ = self.event_tx.send(AcpEvent::Message {
                    kind: AcpMessageKind::System,
                    text: format!("Plan: {plan:?}"),
                });
            }
            SessionUpdate::SessionInfoUpdate(_) => {}
            SessionUpdate::AvailableCommandsUpdate(_)
            | SessionUpdate::CurrentModeUpdate(_)
            | SessionUpdate::ConfigOptionUpdate(_) => {}
            _ => {}
        }

        Ok(())
    }

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        let path = resolve_workspace_path(&self.workspace_root, &args.path, true)
            .map_err(|err| acp::Error::invalid_params().data(err))?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(acp::Error::into_internal_error)?;
        }

        tokio::fs::write(&path, args.content)
            .await
            .map_err(acp::Error::into_internal_error)?;

        let _ = self.event_tx.send(AcpEvent::Message {
            kind: AcpMessageKind::Tool,
            text: format!("ACP wrote file {}", path.display()),
        });

        Ok(acp::WriteTextFileResponse::new())
    }

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        let path = resolve_workspace_path(&self.workspace_root, &args.path, false)
            .map_err(|err| acp::Error::invalid_params().data(err))?;
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(acp::Error::into_internal_error)?;
        let content = apply_read_window(content, args.line, args.limit);

        let _ = self.event_tx.send(AcpEvent::Message {
            kind: AcpMessageKind::Tool,
            text: format!("ACP read file {}", path.display()),
        });

        Ok(acp::ReadTextFileResponse::new(content))
    }

    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        let cwd = match &args.cwd {
            Some(cwd) => resolve_workspace_path(&self.workspace_root, cwd, false)
                .map_err(|err| acp::Error::invalid_params().data(err))?,
            None => self.workspace_root.clone(),
        };
        if !cwd.is_dir() {
            return Err(acp::Error::invalid_params().data(format!(
                "ACP terminal cwd must be a directory: {}",
                cwd.display()
            )));
        }

        let mut command = tokio::process::Command::new(&args.command);
        command
            .args(&args.args)
            .current_dir(&cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for env_var in args.env {
            command.env(env_var.name, env_var.value);
        }

        let mut child = command.spawn().map_err(acp::Error::into_internal_error)?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let terminal_id = self.terminal_registry.next_terminal_id();
        let terminal = Arc::new(TerminalState {
            child: AsyncMutex::new(child),
            output: Mutex::new(String::new()),
            output_limit: args.output_byte_limit.map(|limit| limit as usize),
            truncated: Mutex::new(false),
            exit_status: Mutex::new(None),
        });

        self.terminal_registry
            .insert(terminal_id.clone(), Arc::clone(&terminal))
            .map_err(|err| acp::Error::internal_error().data(err))?;

        if let Some(stdout) = stdout {
            tokio::task::spawn_local(read_terminal_stream(
                Arc::clone(&terminal),
                stdout,
                self.event_tx.clone(),
                terminal_id.clone(),
                "stdout",
            ));
        }
        if let Some(stderr) = stderr {
            tokio::task::spawn_local(read_terminal_stream(
                Arc::clone(&terminal),
                stderr,
                self.event_tx.clone(),
                terminal_id.clone(),
                "stderr",
            ));
        }

        let _ = self.event_tx.send(AcpEvent::Message {
            kind: AcpMessageKind::Tool,
            text: format!(
                "ACP created terminal {terminal_id} in {} for `{}`",
                cwd.display(),
                args.command
            ),
        });

        Ok(acp::CreateTerminalResponse::new(terminal_id))
    }

    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        let terminal = self
            .terminal_registry
            .get(&args.terminal_id.to_string())
            .map_err(|err| acp::Error::internal_error().data(err))?
            .ok_or_else(|| acp::Error::invalid_params().data("Unknown ACP terminal".to_string()))?;

        update_terminal_exit_status(&terminal)
            .await
            .map_err(acp::Error::into_internal_error)?;

        let output = terminal
            .output
            .lock()
            .map_err(|_| acp::Error::internal_error().data("ACP terminal output lock poisoned"))?
            .clone();
        let truncated = *terminal.truncated.lock().map_err(|_| {
            acp::Error::internal_error().data("ACP terminal truncation lock poisoned")
        })?;
        let exit_status = terminal
            .exit_status
            .lock()
            .map_err(|_| {
                acp::Error::internal_error().data("ACP terminal exit status lock poisoned")
            })?
            .clone();

        Ok(acp::TerminalOutputResponse::new(output, truncated).exit_status(exit_status))
    }

    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        let Some(terminal) = self
            .terminal_registry
            .take(&args.terminal_id.to_string())
            .map_err(|err| acp::Error::internal_error().data(err))?
        else {
            return Ok(acp::ReleaseTerminalResponse::new());
        };

        terminate_terminal(&terminal)
            .await
            .map_err(acp::Error::into_internal_error)?;

        let _ = self.event_tx.send(AcpEvent::Message {
            kind: AcpMessageKind::Tool,
            text: format!("ACP released terminal {}", args.terminal_id),
        });

        Ok(acp::ReleaseTerminalResponse::new())
    }

    async fn wait_for_terminal_exit(
        &self,
        args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        let terminal = self
            .terminal_registry
            .get(&args.terminal_id.to_string())
            .map_err(|err| acp::Error::internal_error().data(err))?
            .ok_or_else(|| acp::Error::invalid_params().data("Unknown ACP terminal".to_string()))?;

        let exit_status = wait_for_terminal_exit_status(&terminal)
            .await
            .map_err(acp::Error::into_internal_error)?;

        Ok(acp::WaitForTerminalExitResponse::new(exit_status))
    }

    async fn kill_terminal(
        &self,
        args: acp::KillTerminalRequest,
    ) -> acp::Result<acp::KillTerminalResponse> {
        let terminal = self
            .terminal_registry
            .get(&args.terminal_id.to_string())
            .map_err(|err| acp::Error::internal_error().data(err))?
            .ok_or_else(|| acp::Error::invalid_params().data("Unknown ACP terminal".to_string()))?;

        terminate_terminal(&terminal)
            .await
            .map_err(acp::Error::into_internal_error)?;

        let _ = self.event_tx.send(AcpEvent::Message {
            kind: AcpMessageKind::Tool,
            text: format!("ACP killed terminal {}", args.terminal_id),
        });

        Ok(acp::KillTerminalResponse::new())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
        Ok(())
    }
}

static ACP_RUNTIME: OnceLock<Mutex<Option<AcpRuntimeHandle>>> = OnceLock::new();

fn runtime_slot() -> &'static Mutex<Option<AcpRuntimeHandle>> {
    ACP_RUNTIME.get_or_init(|| Mutex::new(None))
}

pub async fn connect_acp_agent(request: AcpLaunchRequest) -> Result<AcpConnectionInfo, String> {
    disconnect_acp_agent()?;

    let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = oneshot::channel();
    let worker_request = request.clone();
    let permission_registry = Arc::new(PermissionRegistryInner::new());
    let worker_permission_registry = Arc::clone(&permission_registry);
    let terminal_registry = Arc::new(TerminalRegistryInner::new());
    let worker_terminal_registry = Arc::clone(&terminal_registry);

    let thread = thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                let _ = ready_tx.send(Err(format!("Failed to start ACP runtime: {err}")));
                return;
            }
        };

        let local_set = tokio::task::LocalSet::new();
        local_set.block_on(&runtime, async move {
            run_acp_worker(
                worker_request,
                command_rx,
                event_tx,
                ready_tx,
                worker_permission_registry,
                worker_terminal_registry,
            )
            .await;
        });
    });

    let connection = ready_rx
        .await
        .map_err(|_| "ACP worker stopped before initialization finished".to_string())??;

    *runtime_slot()
        .lock()
        .map_err(|_| "ACP runtime lock poisoned".to_string())? = Some(AcpRuntimeHandle {
        command_tx,
        event_rx,
        permission_registry,
        terminal_registry,
        _thread: thread,
    });

    Ok(connection)
}

pub fn send_acp_prompt(prompt: String) -> Result<(), String> {
    send_command(AcpCommand::Prompt(prompt))
}

pub fn cancel_acp_prompt() -> Result<(), String> {
    send_command(AcpCommand::Cancel)
}

pub fn disconnect_acp_agent() -> Result<(), String> {
    let mut slot = runtime_slot()
        .lock()
        .map_err(|_| "ACP runtime lock poisoned".to_string())?;

    if let Some(handle) = slot.take() {
        let request_ids = handle
            .permission_registry
            .pending
            .lock()
            .map_err(|_| "ACP permission registry lock poisoned".to_string())?
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for request_id in request_ids {
            let _ = respond_to_permission_request(&handle.permission_registry, request_id, None);
        }
        for terminal_id in handle.terminal_registry.ids()? {
            let _ = handle.terminal_registry.take(&terminal_id);
        }
        let _ = handle.command_tx.send(AcpCommand::Disconnect);
    }

    Ok(())
}

pub fn drain_acp_events() -> Vec<AcpEvent> {
    let mut drained = Vec::new();
    let Ok(mut slot) = runtime_slot().lock() else {
        return drained;
    };
    let Some(handle) = slot.as_mut() else {
        return drained;
    };

    while let Ok(event) = handle.event_rx.try_recv() {
        drained.push(event);
    }

    drained
}

pub fn respond_acp_permission(request_id: u64, option_id: Option<String>) -> Result<(), String> {
    let slot = runtime_slot()
        .lock()
        .map_err(|_| "ACP runtime lock poisoned".to_string())?;
    let Some(handle) = slot.as_ref() else {
        return Err("ACP agent is not connected".to_string());
    };

    respond_to_permission_request(&handle.permission_registry, request_id, option_id)
}

fn send_command(command: AcpCommand) -> Result<(), String> {
    let slot = runtime_slot()
        .lock()
        .map_err(|_| "ACP runtime lock poisoned".to_string())?;
    let Some(handle) = slot.as_ref() else {
        return Err("ACP agent is not connected".to_string());
    };

    handle
        .command_tx
        .send(command)
        .map_err(|_| "ACP worker is not available".to_string())
}

async fn run_acp_worker(
    request: AcpLaunchRequest,
    mut command_rx: tokio_mpsc::UnboundedReceiver<AcpCommand>,
    event_tx: mpsc::Sender<AcpEvent>,
    ready_tx: oneshot::Sender<Result<AcpConnectionInfo, String>>,
    permission_registry: PermissionRegistry,
    terminal_registry: TerminalRegistry,
) {
    let command = request.command.trim();
    if command.is_empty() {
        let _ = ready_tx.send(Err("ACP command is empty".to_string()));
        return;
    }

    let args = shlex::split(request.args.trim())
        .ok_or_else(|| "Failed to parse ACP arguments".to_string());
    let args = match args {
        Ok(args) => args,
        Err(err) => {
            let _ = ready_tx.send(Err(err));
            return;
        }
    };

    let cwd = match normalize_cwd(&request.cwd) {
        Ok(cwd) => cwd,
        Err(err) => {
            let _ = ready_tx.send(Err(err));
            return;
        }
    };

    let mut child = match tokio::process::Command::new(command)
        .args(&args)
        .current_dir(&cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = ready_tx.send(Err(format!("Failed to start ACP agent: {err}")));
            return;
        }
    };

    let Some(stdin) = child.stdin.take() else {
        let _ = ready_tx.send(Err("ACP agent stdin is unavailable".to_string()));
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = ready_tx.send(Err("ACP agent stdout is unavailable".to_string()));
        return;
    };

    let (conn, io_task) = acp::ClientSideConnection::new(
        BridgeClient {
            event_tx: event_tx.clone(),
            permission_registry,
            terminal_registry,
            workspace_root: cwd.clone(),
        },
        stdin.compat_write(),
        stdout.compat(),
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    let conn = Rc::new(conn);

    {
        let event_tx = event_tx.clone();
        tokio::task::spawn_local(async move {
            if let Err(err) = io_task.await {
                let _ = event_tx.send(AcpEvent::Error(format!("ACP I/O error: {err}")));
            }
        });
    }

    let init_response = match conn
        .initialize(
            acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                .client_capabilities(
                    acp::ClientCapabilities::new()
                        .fs(acp::FileSystemCapabilities::new()
                            .read_text_file(true)
                            .write_text_file(true))
                        .terminal(true),
                )
                .client_info(
                    acp::Implementation::new("showel", env!("CARGO_PKG_VERSION"))
                        .title("Showel ACP Client"),
                ),
        )
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let _ = ready_tx.send(Err(format!("ACP initialize failed: {err}")));
            return;
        }
    };

    let session_response = match conn
        .new_session(acp::NewSessionRequest::new(cwd.clone()))
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let _ = ready_tx.send(Err(format!("ACP session creation failed: {err}")));
            return;
        }
    };

    let agent_name = init_response
        .agent_info
        .as_ref()
        .and_then(|info| info.title.clone())
        .or_else(|| {
            init_response
                .agent_info
                .as_ref()
                .map(|info| info.name.clone())
        })
        .unwrap_or_else(|| command.to_string());

    let connection = AcpConnectionInfo {
        agent_name: agent_name.clone(),
        session_id: session_response.session_id.to_string(),
        protocol_version: format!("{:?}", init_response.protocol_version),
    };

    if ready_tx.send(Ok(connection.clone())).is_err() {
        return;
    }

    let _ = event_tx.send(AcpEvent::Connected(connection));
    let _ = event_tx.send(AcpEvent::Status(format!("Connected to {agent_name}")));

    let active_session_id = Rc::new(RefCell::new(session_response.session_id.to_string()));

    while let Some(command) = command_rx.recv().await {
        match command {
            AcpCommand::Prompt(prompt) => {
                let conn = Rc::clone(&conn);
                let event_tx = event_tx.clone();
                let active_session_id = Rc::clone(&active_session_id);

                tokio::task::spawn_local(async move {
                    let session_id = active_session_id.borrow().clone();

                    let _ = event_tx.send(AcpEvent::PromptStarted);
                    match conn
                        .prompt(acp::PromptRequest::new(
                            session_id,
                            vec![ContentBlock::from(prompt)],
                        ))
                        .await
                    {
                        Ok(response) => {
                            let _ = event_tx.send(AcpEvent::PromptFinished {
                                stop_reason: format!("{:?}", response.stop_reason),
                            });
                        }
                        Err(err) => {
                            let _ =
                                event_tx.send(AcpEvent::Error(format!("ACP prompt failed: {err}")));
                        }
                    }
                });
            }
            AcpCommand::Cancel => {
                let session_id = active_session_id.borrow().clone();
                if let Err(err) = conn.cancel(acp::CancelNotification::new(session_id)).await {
                    let _ = event_tx.send(AcpEvent::Error(format!("ACP cancel failed: {err}")));
                } else {
                    let _ = event_tx.send(AcpEvent::Status("Cancelling prompt...".to_string()));
                }
            }
            AcpCommand::Disconnect => {
                break;
            }
        }
    }

    drop(child);
    let _ = event_tx.send(AcpEvent::Disconnected);
}

fn normalize_cwd(cwd: &str) -> Result<PathBuf, String> {
    let trimmed = cwd.trim();
    let resolved = if trimmed.is_empty() {
        std::env::current_dir().map_err(|err| format!("Failed to resolve cwd: {err}"))?
    } else {
        let path = PathBuf::from(trimmed);
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map(|current_dir| current_dir.join(path))
                .map_err(|err| format!("Failed to resolve cwd: {err}"))?
        }
    };

    normalize_cwd_path(resolved)
}

fn normalize_cwd_path(path: PathBuf) -> Result<PathBuf, String> {
    if let Ok(metadata) = std::fs::metadata(&path) {
        if metadata.is_dir() {
            return Ok(path);
        }

        if metadata.is_file() {
            return path
                .parent()
                .map(PathBuf::from)
                .ok_or_else(|| format!("ACP cwd has no parent directory: {}", path.display()));
        }
    }

    let mut ancestor = path.parent();
    while let Some(parent) = ancestor {
        if parent.is_dir() {
            return Ok(parent.to_path_buf());
        }
        ancestor = parent.parent();
    }

    Ok(path)
}

fn resolve_workspace_path(
    workspace_root: &std::path::Path,
    requested_path: &std::path::Path,
    allow_missing_leaf: bool,
) -> Result<PathBuf, String> {
    if !requested_path.is_absolute() {
        return Err("ACP file path must be absolute".to_string());
    }

    let canonical_root = std::fs::canonicalize(workspace_root)
        .map_err(|err| format!("Failed to resolve ACP workspace root: {err}"))?;

    let resolved = if allow_missing_leaf && !requested_path.exists() {
        resolve_future_workspace_path(&canonical_root, requested_path)?
    } else {
        std::fs::canonicalize(requested_path)
            .map_err(|err| format!("Failed to resolve ACP file path: {err}"))?
    };

    if !resolved.starts_with(&canonical_root) {
        return Err(format!(
            "ACP file access outside workspace is not allowed: {}",
            resolved.display()
        ));
    }

    Ok(resolved)
}

fn resolve_future_workspace_path(
    canonical_root: &std::path::Path,
    requested_path: &std::path::Path,
) -> Result<PathBuf, String> {
    let mut existing = requested_path;
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| "ACP file path has no existing ancestor".to_string())?;
    }

    let canonical_existing = std::fs::canonicalize(existing)
        .map_err(|err| format!("Failed to resolve ACP path ancestor: {err}"))?;

    if !canonical_existing.starts_with(canonical_root) {
        return Err(format!(
            "ACP file access outside workspace is not allowed: {}",
            requested_path.display()
        ));
    }

    Ok(requested_path.to_path_buf())
}

fn apply_read_window(content: String, line: Option<u32>, limit: Option<u32>) -> String {
    let start = line.unwrap_or(1).max(1) as usize - 1;
    let limit = limit.map(|value| value as usize);
    let lines = content.lines().collect::<Vec<_>>();
    let selected = lines
        .into_iter()
        .skip(start)
        .take(limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();

    if selected.is_empty() {
        String::new()
    } else {
        selected.join("\n")
    }
}

async fn read_terminal_stream<R>(
    terminal: Arc<TerminalState>,
    mut reader: R,
    event_tx: mpsc::Sender<AcpEvent>,
    terminal_id: String,
    stream_label: &'static str,
) where
    R: tokio::io::AsyncRead + Unpin + 'static,
{
    let mut buffer = vec![0_u8; 4096];

    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => {
                let chunk = String::from_utf8_lossy(&buffer[..read]).into_owned();
                if let Err(err) = append_terminal_output(&terminal, &chunk) {
                    let _ = event_tx.send(AcpEvent::Error(err));
                    break;
                }
            }
            Err(err) => {
                let _ = event_tx.send(AcpEvent::Error(format!(
                    "ACP terminal {terminal_id} {stream_label} read failed: {err}"
                )));
                break;
            }
        }
    }

    if let Err(err) = update_terminal_exit_status(&terminal).await {
        let _ = event_tx.send(AcpEvent::Error(format!(
            "ACP terminal {terminal_id} exit status update failed: {err}"
        )));
    }
}

fn append_terminal_output(terminal: &Arc<TerminalState>, chunk: &str) -> Result<(), String> {
    let mut output = terminal
        .output
        .lock()
        .map_err(|_| "ACP terminal output lock poisoned".to_string())?;
    output.push_str(chunk);

    if let Some(limit) = terminal.output_limit
        && output.len() > limit
    {
        let trim_target = output.len() - limit;
        let trim_at = output
            .char_indices()
            .map(|(idx, _)| idx)
            .find(|idx| *idx >= trim_target)
            .unwrap_or(output.len());
        output.drain(..trim_at);
        *terminal
            .truncated
            .lock()
            .map_err(|_| "ACP terminal truncation lock poisoned".to_string())? = true;
    }

    Ok(())
}

async fn update_terminal_exit_status(terminal: &Arc<TerminalState>) -> Result<(), std::io::Error> {
    if terminal
        .exit_status
        .lock()
        .map_err(|_| std::io::Error::other("ACP terminal exit status lock poisoned"))?
        .is_some()
    {
        return Ok(());
    }

    let status = {
        let mut child = terminal.child.lock().await;
        child.try_wait()?
    };

    if let Some(status) = status {
        *terminal
            .exit_status
            .lock()
            .map_err(|_| std::io::Error::other("ACP terminal exit status lock poisoned"))? =
            Some(terminal_exit_status(status));
    }

    Ok(())
}

async fn wait_for_terminal_exit_status(
    terminal: &Arc<TerminalState>,
) -> Result<acp::TerminalExitStatus, std::io::Error> {
    if let Some(status) = terminal
        .exit_status
        .lock()
        .map_err(|_| std::io::Error::other("ACP terminal exit status lock poisoned"))?
        .clone()
    {
        return Ok(status);
    }

    let status = {
        let mut child = terminal.child.lock().await;
        child.wait().await?
    };
    let status = terminal_exit_status(status);
    *terminal
        .exit_status
        .lock()
        .map_err(|_| std::io::Error::other("ACP terminal exit status lock poisoned"))? =
        Some(status.clone());

    Ok(status)
}

async fn terminate_terminal(terminal: &Arc<TerminalState>) -> Result<(), std::io::Error> {
    if terminal
        .exit_status
        .lock()
        .map_err(|_| std::io::Error::other("ACP terminal exit status lock poisoned"))?
        .is_some()
    {
        return Ok(());
    }

    let status = {
        let mut child = terminal.child.lock().await;
        match child.try_wait()? {
            Some(status) => status,
            None => {
                child.kill().await?;
                child.wait().await?
            }
        }
    };

    *terminal
        .exit_status
        .lock()
        .map_err(|_| std::io::Error::other("ACP terminal exit status lock poisoned"))? =
        Some(terminal_exit_status(status));

    Ok(())
}

fn terminal_exit_status(status: std::process::ExitStatus) -> acp::TerminalExitStatus {
    let exit_code = status.code().map(|code| code as u32);
    #[cfg(unix)]
    let signal = status.signal().map(|signal| signal.to_string());
    #[cfg(not(unix))]
    let signal = None::<String>;

    acp::TerminalExitStatus::new()
        .exit_code(exit_code)
        .signal(signal)
}

fn send_chunk(event_tx: &mpsc::Sender<AcpEvent>, kind: AcpMessageKind, chunk: ContentChunk) {
    let text = content_to_text(chunk.content);
    if text.is_empty() {
        return;
    }

    let _ = event_tx.send(AcpEvent::Message { kind, text });
}

fn content_to_text(content: ContentBlock) -> String {
    match content {
        ContentBlock::Text(text) => text.text,
        ContentBlock::Image(_) => "<image>".to_string(),
        ContentBlock::Audio(_) => "<audio>".to_string(),
        ContentBlock::ResourceLink(link) => link.uri,
        ContentBlock::Resource(_) => "<resource>".to_string(),
        _ => "<content>".to_string(),
    }
}

fn respond_to_permission_request(
    registry: &PermissionRegistry,
    request_id: u64,
    option_id: Option<String>,
) -> Result<(), String> {
    let sender = registry
        .take(request_id)?
        .ok_or_else(|| "Permission request no longer exists".to_string())?;

    let response = match option_id {
        Some(option_id) => RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
            SelectedPermissionOutcome::new(option_id),
        )),
        None => RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled),
    };

    sender
        .send(response)
        .map_err(|_| "Permission request receiver dropped".to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_cwd_path;

    #[test]
    fn normalize_cwd_path_keeps_existing_directory() {
        let dir = std::env::temp_dir();
        assert_eq!(normalize_cwd_path(dir.clone()).unwrap(), dir);
    }

    #[test]
    fn normalize_cwd_path_uses_parent_for_existing_file() {
        let temp_root =
            std::env::temp_dir().join(format!("showel-acp-runtime-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).unwrap();
        let file = temp_root.join("workspace.db");
        std::fs::write(&file, b"test").unwrap();

        assert_eq!(normalize_cwd_path(file).unwrap(), temp_root);
    }

    #[test]
    fn normalize_cwd_path_uses_existing_parent_for_missing_file_like_path() {
        let dir = std::env::temp_dir();
        let path = dir.join("missing").join("project.db");

        assert_eq!(normalize_cwd_path(path).unwrap(), dir);
    }
}
