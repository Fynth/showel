use models::{
    AcpConnectionInfo, AcpEvent, AcpLaunchRequest, AcpMessageKind, AcpOllamaConfig, AcpPanelState,
    AcpUiMessage,
};

pub(crate) fn default_acp_panel_state() -> AcpPanelState {
    let cwd = std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    AcpPanelState::new(
        AcpLaunchRequest {
            command: std::env::var("SHOWEL_ACP_COMMAND").unwrap_or_default(),
            args: std::env::var("SHOWEL_ACP_ARGS").unwrap_or_default(),
            cwd,
        },
        AcpOllamaConfig {
            base_url: std::env::var("SHOWEL_OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434/api".to_string()),
            model: std::env::var("SHOWEL_OLLAMA_MODEL").unwrap_or_default(),
            api_key: std::env::var("OLLAMA_API_KEY").unwrap_or_default(),
        },
    )
}

pub(crate) fn apply_acp_events(state: &mut AcpPanelState, events: Vec<AcpEvent>) {
    for event in events {
        match event {
            AcpEvent::Connected(connection) => {
                apply_connected(state, connection);
            }
            AcpEvent::Status(status) => {
                state.status = status;
            }
            AcpEvent::Message { kind, text } => {
                push_or_append_message(state, kind, text);
            }
            AcpEvent::PermissionRequested(request) => {
                state.pending_permission = Some(request);
                state.busy = true;
                state.status = "ACP agent is waiting for permission.".to_string();
            }
            AcpEvent::PromptStarted => {
                state.busy = true;
                state.status = "Agent is working...".to_string();
            }
            AcpEvent::PromptFinished { stop_reason } => {
                state.busy = false;
                state.pending_permission = None;
                state.status = format!("Prompt finished: {stop_reason}");
                state
                    .messages
                    .retain(|message| !matches!(message.kind, AcpMessageKind::Thought));
            }
            AcpEvent::Error(error) => {
                state.busy = false;
                state.pending_permission = None;
                state.pending_sql_insert = false;
                state.status = error.clone();
                state
                    .messages
                    .retain(|message| !matches!(message.kind, AcpMessageKind::Thought));
                push_message(state, AcpMessageKind::Error, error);
            }
            AcpEvent::Disconnected => {
                state.busy = false;
                state.connected = false;
                state.pending_sql_insert = false;
                state.connection = None;
                state.pending_permission = None;
                state.status = "ACP agent disconnected.".to_string();
                state
                    .messages
                    .retain(|message| !matches!(message.kind, AcpMessageKind::Thought));
            }
        }
    }
}

pub(super) fn apply_connected(state: &mut AcpPanelState, connection: AcpConnectionInfo) {
    state.connected = true;
    state.busy = false;
    state.pending_sql_insert = false;
    state.connection = Some(connection.clone());
    state.pending_permission = None;
    state.messages.retain(|message| {
        !matches!(message.kind, AcpMessageKind::System)
            || !message.text.starts_with("Connected to ")
    });
    state.status = format!("Connected to {}", connection.agent_name);
}

fn push_or_append_message(state: &mut AcpPanelState, kind: AcpMessageKind, text: String) {
    if text.is_empty() && !matches!(kind, AcpMessageKind::Tool | AcpMessageKind::Thought) {
        return;
    }

    if matches!(kind, AcpMessageKind::Thought) {
        if state
            .messages
            .last()
            .is_some_and(|last| matches!(last.kind, AcpMessageKind::Thought))
        {
            return;
        }

        push_message(state, kind, String::new());
        return;
    }

    if let Some(last) = state.messages.last_mut()
        && last.kind == kind
    {
        if matches!(kind, AcpMessageKind::Tool) {
            return;
        }
        last.text.push_str(&text);
        return;
    }

    if matches!(kind, AcpMessageKind::Tool) {
        push_message(state, kind, "🛠".to_string());
    } else {
        push_message(state, kind, text);
    }
}

pub(super) fn push_message(state: &mut AcpPanelState, kind: AcpMessageKind, text: String) {
    let id = state.next_message_id;
    state.next_message_id += 1;
    state.messages.push(AcpUiMessage { id, kind, text });
}

pub(super) fn message_kind_label(kind: &AcpMessageKind) -> &'static str {
    match kind {
        AcpMessageKind::User => "You",
        AcpMessageKind::Agent => "Agent",
        AcpMessageKind::Thought => "Working",
        AcpMessageKind::Tool => "Tool",
        AcpMessageKind::System => "Status",
        AcpMessageKind::Error => "Error",
    }
}

pub(super) fn message_kind_class(kind: &AcpMessageKind) -> &'static str {
    match kind {
        AcpMessageKind::User => "user",
        AcpMessageKind::Agent => "agent",
        AcpMessageKind::Thought => "thought",
        AcpMessageKind::Tool => "tool",
        AcpMessageKind::System => "system",
        AcpMessageKind::Error => "error",
    }
}

pub(super) fn permission_button_class(kind: &str) -> &'static str {
    if kind.contains("Allow") {
        "button button--primary button--small"
    } else {
        "button button--ghost button--small"
    }
}
