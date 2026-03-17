use dioxus::prelude::*;
use models::{
    AcpConnectionInfo, AcpEvent, AcpLaunchRequest, AcpMessageKind, AcpPanelState, AcpRegistryAgent,
    AcpUiMessage, QueryTabState,
};

use crate::screens::workspace::actions::update_active_tab_sql;

#[component]
pub fn AcpAgentPanel(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    mut show_sql_editor: Signal<bool>,
    sql_connection_label: String,
) -> Element {
    let state = panel_state();
    let mut registry_busy = use_signal(|| false);
    let mut registry_status = use_signal(String::new);
    let registry_agents =
        use_resource(move || async move { services::load_acp_registry_agents().await });
    let opencode_agent = registry_agents().and_then(|result| {
        result
            .ok()
            .and_then(|agents| agents.into_iter().find(|agent| agent.id == "opencode"))
    });

    rsx! {
        aside { class: "agent-panel",
            div { class: "agent-panel__header",
                div {
                    h3 { class: "agent-panel__title", "ACP Agent" }
                    p { class: "agent-panel__meta", "{state.status}" }
                }
                if let Some(connection) = state.connection.clone() {
                    div { class: "agent-panel__badge",
                        "{connection.agent_name}"
                    }
                }
            }

            if state.connected {
                div { class: "agent-panel__session",
                    if let Some(connection) = state.connection.clone() {
                        p { class: "agent-panel__session-line",
                            span { class: "agent-panel__session-label", "Session" }
                            span { class: "agent-panel__session-value", "{connection.session_id}" }
                        }
                        p { class: "agent-panel__session-line",
                            span { class: "agent-panel__session-label", "Protocol" }
                            span { class: "agent-panel__session-value", "{connection.protocol_version}" }
                        }
                    }
                    div { class: "agent-panel__session-actions",
                        button {
                            class: "button button--ghost button--small",
                            disabled: !state.busy,
                            onclick: move |_| {
                                if let Err(err) = services::cancel_acp_prompt() {
                                    panel_state.with_mut(|state| {
                                        state.status = err.clone();
                                        push_message(state, AcpMessageKind::Error, err);
                                    });
                                } else {
                                    panel_state.with_mut(|state| {
                                        state.status = "Cancelling prompt...".to_string();
                                    });
                                }
                            },
                            "Cancel"
                        }
                        button {
                            class: "button button--ghost button--small",
                            onclick: move |_| {
                                let _ = services::disconnect_acp_agent();
                                panel_state.with_mut(|state| {
                                    state.connected = false;
                                    state.busy = false;
                                    state.pending_sql_insert = false;
                                    state.connection = None;
                                    state.status = "ACP agent is disconnected.".to_string();
                                });
                            },
                            "Disconnect"
                        }
                    }
                }

                div { class: "agent-panel__messages",
                    if state.messages.is_empty() {
                        p { class: "empty-state", "Ask for SQL or schema help. Generated SQL can be inserted into the active editor." }
                    } else {
                        for message in state.messages {
                            article {
                                class: format!("agent-panel__message agent-panel__message--{}", message_kind_class(&message.kind)),
                                p { class: "agent-panel__message-role", "{message_kind_label(&message.kind)}" }
                                pre { class: "agent-panel__message-body", "{message.text}" }
                                if matches!(message.kind, AcpMessageKind::Agent) {
                                    if let Some(sql) = extract_sql_candidate(&message.text) {
                                        div { class: "agent-panel__message-actions",
                                            button {
                                                class: "button button--ghost button--small",
                                                onclick: {
                                                    let sql = sql.clone();
                                                    move |_| {
                                                        insert_sql_into_editor(
                                                            panel_state,
                                                            tabs,
                                                            active_tab_id(),
                                                            show_sql_editor,
                                                            sql.clone(),
                                                        );
                                                    }
                                                },
                                                "Insert into SQL editor"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(permission_request) = state.pending_permission.clone() {
                    div { class: "agent-panel__permission",
                        div {
                            class: "agent-panel__permission-copy",
                            p { class: "agent-panel__message-role", "Permission Required" }
                            pre { class: "agent-panel__message-body", "{permission_request.tool_summary}" }
                        }
                        div { class: "agent-panel__permission-actions",
                            for option in permission_request.options {
                                button {
                                    class: permission_button_class(&option.kind),
                                    onclick: {
                                        let request_id = permission_request.request_id;
                                        let option_id = option.option_id.clone();
                                        let label = option.label.clone();
                                        move |_| {
                                            match services::respond_acp_permission(
                                                request_id,
                                                Some(option_id.clone()),
                                            ) {
                                                Ok(()) => {
                                                    panel_state.with_mut(|state| {
                                                        state.pending_permission = None;
                                                        state.status = format!("Permission response sent: {label}");
                                                        push_message(
                                                            state,
                                                            AcpMessageKind::System,
                                                            format!("Selected permission option: {label}"),
                                                        );
                                                    });
                                                }
                                                Err(err) => {
                                                    panel_state.with_mut(|state| {
                                                        state.status = err.clone();
                                                        push_message(state, AcpMessageKind::Error, err);
                                                    });
                                                }
                                            }
                                        }
                                    },
                                    "{option.label}"
                                }
                            }
                            button {
                                class: "button button--ghost button--small",
                                onclick: {
                                    let request_id = permission_request.request_id;
                                    move |_| {
                                        match services::respond_acp_permission(request_id, None) {
                                            Ok(()) => {
                                                panel_state.with_mut(|state| {
                                                    state.pending_permission = None;
                                                    state.status = "Permission cancelled.".to_string();
                                                    push_message(
                                                        state,
                                                        AcpMessageKind::System,
                                                        "Cancelled permission request.".to_string(),
                                                    );
                                                });
                                            }
                                            Err(err) => {
                                                panel_state.with_mut(|state| {
                                                    state.status = err.clone();
                                                    push_message(state, AcpMessageKind::Error, err);
                                                });
                                            }
                                        }
                                    }
                                },
                                "Cancel Request"
                            }
                        }
                    }
                }

                div { class: "agent-panel__composer",
                    textarea {
                        class: "input agent-panel__prompt",
                        value: "{state.prompt}",
                        placeholder: "Например: сделай мне команду вывода всей инфы из таблицы users",
                        oninput: move |event| {
                            let value = event.value();
                            panel_state.with_mut(|state| state.prompt = value);
                        }
                    }
                    div { class: "agent-panel__composer-actions",
                        button {
                            class: "button button--ghost",
                            disabled: state.busy || state.prompt.trim().is_empty(),
                            onclick: move |_| {
                                let request = panel_state().prompt.trim().to_string();
                                if request.is_empty() {
                                    return;
                                }

                                let prompt = build_sql_generation_prompt(&sql_connection_label, &request);
                                match services::send_acp_prompt(prompt) {
                                    Ok(()) => {
                                        panel_state.with_mut(|state| {
                                            push_message(
                                                state,
                                                AcpMessageKind::User,
                                                format!("Generate SQL: {request}"),
                                            );
                                            state.prompt.clear();
                                            state.busy = true;
                                            state.pending_sql_insert = true;
                                            state.status =
                                                "Waiting for agent SQL to insert into the editor...".to_string();
                                        });
                                    }
                                    Err(err) => {
                                        panel_state.with_mut(|state| {
                                            state.status = err.clone();
                                            push_message(state, AcpMessageKind::Error, err);
                                        });
                                    }
                                }
                            },
                            "Generate SQL"
                        }
                        button {
                            class: "button button--primary",
                            disabled: state.busy || state.prompt.trim().is_empty(),
                            onclick: move |_| {
                                let prompt = panel_state().prompt.trim().to_string();
                                if prompt.is_empty() {
                                    return;
                                }

                                match services::send_acp_prompt(prompt.clone()) {
                                    Ok(()) => {
                                        panel_state.with_mut(|state| {
                                            push_message(state, AcpMessageKind::User, prompt);
                                            state.prompt.clear();
                                            state.busy = true;
                                            state.pending_sql_insert = false;
                                            state.status = "Waiting for agent response...".to_string();
                                        });
                                    }
                                    Err(err) => {
                                        panel_state.with_mut(|state| {
                                            state.status = err.clone();
                                            push_message(state, AcpMessageKind::Error, err);
                                        });
                                    }
                                }
                            },
                            "Send Prompt"
                        }
                    }
                }
            } else {
                div { class: "agent-panel__connect",
                    div { class: "agent-panel__section",
                        div { class: "agent-panel__section-header",
                            h4 { class: "agent-panel__section-title", "ACP Registry" }
                            if !registry_status().trim().is_empty() {
                                p { class: "agent-panel__hint", "{registry_status}" }
                            }
                        }
                        if let Some(opencode) = opencode_agent {
                            RegistryAgentCard {
                                agent: opencode,
                                busy: registry_busy(),
                                on_connect: move |_| {
                                    let cwd = panel_state().launch.cwd.clone();
                                    registry_busy.set(true);
                                    registry_status.set("Preparing OpenCode from the ACP registry...".to_string());
                                    spawn(async move {
                                        match services::install_acp_registry_agent("opencode".to_string(), cwd).await {
                                            Ok(launch) => {
                                                panel_state.with_mut(|state| {
                                                    state.launch = launch.clone();
                                                    state.busy = true;
                                                    state.status = "Connecting to OpenCode...".to_string();
                                                });
                                                match services::connect_acp_agent(launch).await {
                                                    Ok(connection) => {
                                                        panel_state.with_mut(|state| {
                                                            apply_connected(state, connection.clone());
                                                            push_message(
                                                                state,
                                                                AcpMessageKind::System,
                                                                format!(
                                                                    "Connected to {} using the ACP registry entry.",
                                                                    connection.agent_name
                                                                ),
                                                            );
                                                        });
                                                        registry_status.set("OpenCode connected.".to_string());
                                                    }
                                                    Err(err) => {
                                                        panel_state.with_mut(|state| {
                                                            state.busy = false;
                                                            state.connected = false;
                                                            state.connection = None;
                                                            state.status = err.clone();
                                                            push_message(state, AcpMessageKind::Error, err.clone());
                                                        });
                                                        registry_status.set(err);
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                panel_state.with_mut(|state| {
                                                    state.status = err.clone();
                                                    push_message(state, AcpMessageKind::Error, err.clone());
                                                });
                                                registry_status.set(err);
                                            }
                                        }
                                        registry_busy.set(false);
                                    });
                                }
                            }
                        } else if let Some(result) = registry_agents() {
                            match result {
                                Ok(_) => rsx! {
                                    p { class: "agent-panel__hint", "OpenCode is not available in the ACP registry for this platform." }
                                },
                                Err(err) => rsx! {
                                    p { class: "agent-panel__hint", "Failed to load ACP registry: {err}" }
                                },
                            }
                        } else {
                            p { class: "agent-panel__hint", "Loading ACP registry..." }
                        }
                    }

                    div { class: "field",
                        label { class: "field__label", "ACP command" }
                        input {
                            class: "input",
                            value: "{state.launch.command}",
                            placeholder: "path/to/acp-agent",
                            oninput: move |event| {
                                let value = event.value();
                                panel_state.with_mut(|state| state.launch.command = value);
                            }
                        }
                    }
                    div { class: "field",
                        label { class: "field__label", "Arguments" }
                        input {
                            class: "input",
                            value: "{state.launch.args}",
                            placeholder: "--arg value",
                            oninput: move |event| {
                                let value = event.value();
                                panel_state.with_mut(|state| state.launch.args = value);
                            }
                        }
                    }
                    div { class: "field",
                        label { class: "field__label", "Working directory" }
                        input {
                            class: "input",
                            value: "{state.launch.cwd}",
                            oninput: move |event| {
                                let value = event.value();
                                panel_state.with_mut(|state| state.launch.cwd = value);
                            }
                        }
                    }
                    p {
                        class: "agent-panel__hint",
                        "Use any ACP-compatible agent binary. Showel connects over stdio."
                    }
                    button {
                        class: "button button--primary",
                        disabled: state.busy || state.launch.command.trim().is_empty(),
                        onclick: move |_| {
                            let launch = panel_state().launch.clone();
                            panel_state.with_mut(|state| {
                                state.busy = true;
                                state.status = "Connecting to ACP agent...".to_string();
                            });
                            spawn(async move {
                                match services::connect_acp_agent(launch).await {
                                    Ok(connection) => {
                                        panel_state.with_mut(|state| {
                                            apply_connected(state, connection.clone());
                                            push_message(
                                                state,
                                                AcpMessageKind::System,
                                                format!("Connected to {} using ACP.", connection.agent_name),
                                            );
                                        });
                                    }
                                    Err(err) => {
                                        panel_state.with_mut(|state| {
                                            state.busy = false;
                                            state.connected = false;
                                            state.connection = None;
                                            state.status = err.clone();
                                            push_message(state, AcpMessageKind::Error, err);
                                        });
                                    }
                                }
                            });
                        },
                        "Connect Agent"
                    }
                }
            }
        }
    }
}

#[component]
fn RegistryAgentCard(
    agent: AcpRegistryAgent,
    busy: bool,
    on_connect: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        article { class: "agent-panel__registry-card",
            div { class: "agent-panel__registry-copy",
                div { class: "agent-panel__registry-row",
                    h5 { class: "agent-panel__registry-title", "{agent.name}" }
                    span { class: "agent-panel__badge", "v{agent.version}" }
                }
                p { class: "agent-panel__hint", "{agent.description}" }
                p {
                    class: "agent-panel__hint",
                    if agent.installed {
                        "Installed locally and ready to connect."
                    } else {
                        "Downloads and starts the official registry build as `opencode acp`."
                    }
                }
            }
            button {
                class: "button button--primary button--small",
                disabled: busy,
                onclick: move |event| on_connect.call(event),
                if busy { "Preparing..." } else if agent.installed { "Connect OpenCode" } else { "Install & Connect OpenCode" }
            }
        }
    }
}

pub fn default_acp_panel_state() -> AcpPanelState {
    let cwd = std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    AcpPanelState::new(AcpLaunchRequest {
        command: std::env::var("SHOWEL_ACP_COMMAND").unwrap_or_default(),
        args: std::env::var("SHOWEL_ACP_ARGS").unwrap_or_default(),
        cwd,
    })
}

pub fn apply_acp_events(state: &mut AcpPanelState, events: Vec<AcpEvent>) {
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
            }
            AcpEvent::Error(error) => {
                state.busy = false;
                state.pending_permission = None;
                state.pending_sql_insert = false;
                state.status = error.clone();
                push_message(state, AcpMessageKind::Error, error);
            }
            AcpEvent::Disconnected => {
                state.busy = false;
                state.connected = false;
                state.pending_sql_insert = false;
                state.connection = None;
                state.pending_permission = None;
                state.status = "ACP agent disconnected.".to_string();
            }
        }
    }
}

fn apply_connected(state: &mut AcpPanelState, connection: AcpConnectionInfo) {
    state.connected = true;
    state.busy = false;
    state.pending_sql_insert = false;
    state.connection = Some(connection.clone());
    state.pending_permission = None;
    state.status = format!("Connected to {}", connection.agent_name);
}

fn push_or_append_message(state: &mut AcpPanelState, kind: AcpMessageKind, text: String) {
    if text.is_empty() {
        return;
    }

    if let Some(last) = state.messages.last_mut() {
        if last.kind == kind {
            last.text.push_str(&text);
            return;
        }
    }

    push_message(state, kind, text);
}

fn push_message(state: &mut AcpPanelState, kind: AcpMessageKind, text: String) {
    let id = state.next_message_id;
    state.next_message_id += 1;
    state.messages.push(AcpUiMessage { id, kind, text });
}

fn message_kind_label(kind: &AcpMessageKind) -> &'static str {
    match kind {
        AcpMessageKind::User => "User",
        AcpMessageKind::Agent => "Agent",
        AcpMessageKind::Thought => "Thought",
        AcpMessageKind::Tool => "Tool",
        AcpMessageKind::System => "System",
        AcpMessageKind::Error => "Error",
    }
}

fn message_kind_class(kind: &AcpMessageKind) -> &'static str {
    match kind {
        AcpMessageKind::User => "user",
        AcpMessageKind::Agent => "agent",
        AcpMessageKind::Thought => "thought",
        AcpMessageKind::Tool => "tool",
        AcpMessageKind::System => "system",
        AcpMessageKind::Error => "error",
    }
}

fn permission_button_class(kind: &str) -> &'static str {
    if kind.contains("Allow") {
        "button button--primary button--small"
    } else {
        "button button--ghost button--small"
    }
}

pub fn extract_sql_candidate(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(sql) = extract_fenced_block(trimmed, "sql") {
        return Some(sql);
    }
    if let Some(sql) = extract_any_fenced_block(trimmed) {
        return Some(sql);
    }

    let lowered = trimmed.to_ascii_lowercase();
    [
        "select", "with", "insert", "update", "delete", "create", "alter", "drop", "truncate",
    ]
    .iter()
    .any(|keyword| lowered.starts_with(keyword))
    .then(|| trimmed.to_string())
}

fn extract_fenced_block(text: &str, language: &str) -> Option<String> {
    let needle = format!("```{language}");
    let start = text.find(&needle)?;
    let rest = &text[start + needle.len()..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

fn extract_any_fenced_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let rest = match rest.find('\n') {
        Some(newline) => &rest[newline + 1..],
        None => rest,
    };
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

fn build_sql_generation_prompt(connection_label: &str, request: &str) -> String {
    format!(
        "You are generating SQL for the active database connection.\n\
Database context: {connection_label}\n\
Return exactly one SQL query inside a single ```sql``` block with no explanation.\n\
User request: {request}"
    )
}

fn insert_sql_into_editor(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    mut show_sql_editor: Signal<bool>,
    sql: String,
) {
    if active_tab_id == 0 {
        panel_state.with_mut(|state| {
            state.status = "No active SQL tab to insert into.".to_string();
            push_message(
                state,
                AcpMessageKind::Error,
                "No active SQL tab to insert into.".to_string(),
            );
        });
        return;
    }

    show_sql_editor.set(true);
    update_active_tab_sql(
        tabs,
        active_tab_id,
        sql,
        "SQL inserted from ACP agent".to_string(),
    );
    panel_state.with_mut(|state| {
        state.pending_sql_insert = false;
        state.status = "Inserted agent SQL into the active editor.".to_string();
    });
}
