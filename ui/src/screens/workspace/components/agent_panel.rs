#[path = "agent_panel/prompt.rs"]
mod prompt;
#[path = "agent_panel/registry_card.rs"]
mod registry_card;
#[path = "agent_panel/state.rs"]
mod state;

use dioxus::prelude::*;
use models::{AcpMessageKind, AcpPanelState, QueryTabState};

use super::{ActionIcon, IconButton};

use self::{
    prompt::{
        active_editor_connection, active_editor_focus_source, active_editor_prompt_context,
        build_chat_prompt, build_sql_generation_prompt, insert_sql_into_editor,
    },
    registry_card::RegistryAgentCard,
    state::{
        apply_connected, message_kind_class, message_kind_label, permission_button_class,
        push_message,
    },
};

pub(crate) use self::{
    prompt::extract_sql_candidate,
    state::{apply_acp_events, default_acp_panel_state},
};

fn send_chat_prompt_request(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    connection_label: String,
) {
    let prompt = panel_state().prompt.trim().to_string();
    if prompt.is_empty() || panel_state().busy {
        return;
    }

    let connection = active_editor_connection(tabs, active_tab_id);
    let focus_source = active_editor_focus_source(tabs, active_tab_id);
    let active_tab_context = active_editor_prompt_context(tabs, active_tab_id);
    panel_state.with_mut(|state| {
        state.busy = true;
        state.pending_sql_insert = false;
        state.status = "Preparing connected database context for the agent...".to_string();
    });

    spawn(async move {
        let contextual_prompt = match connection {
            Some(connection) => {
                match acp::build_acp_database_context(
                    connection,
                    connection_label.clone(),
                    focus_source,
                )
                .await
                {
                    Ok(db_context) => build_chat_prompt(
                        &connection_label,
                        &prompt,
                        Some(db_context),
                        active_tab_context.clone(),
                    ),
                    Err(_) => build_chat_prompt(
                        &connection_label,
                        &prompt,
                        None,
                        active_tab_context.clone(),
                    ),
                }
            }
            None => build_chat_prompt(&connection_label, &prompt, None, active_tab_context.clone()),
        };

        match acp::send_acp_prompt(contextual_prompt) {
            Ok(()) => {
                panel_state.with_mut(|state| {
                    push_message(state, AcpMessageKind::User, prompt.clone());
                    state.prompt.clear();
                    state.busy = true;
                    state.pending_sql_insert = false;
                    state.status = "Waiting for agent response...".to_string();
                });
            }
            Err(err) => {
                panel_state.with_mut(|state| {
                    state.status = err.clone();
                    state.busy = false;
                    push_message(state, AcpMessageKind::Error, err);
                });
            }
        }
    });
}

fn is_connection_notice(kind: &AcpMessageKind, text: &str) -> bool {
    matches!(kind, AcpMessageKind::System) && text.starts_with("Connected to ")
}

fn is_visible_message(kind: &AcpMessageKind, text: &str) -> bool {
    !is_connection_notice(kind, text)
        && !matches!(kind, AcpMessageKind::System | AcpMessageKind::Tool)
}

const OPENCODE_REGISTRY_AGENT_ID: &str = "opencode";
const CODEX_REGISTRY_AGENT_ID: &str = "codex-acp";

#[derive(Clone, Copy, PartialEq, Eq)]
enum AgentSetupMode {
    Ollama,
    OpenCode,
    Codex,
    Custom,
}

impl AgentSetupMode {
    const ALL: [Self; 4] = [Self::Ollama, Self::OpenCode, Self::Codex, Self::Custom];

    fn label(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::OpenCode => "OpenCode",
            Self::Codex => "Codex",
            Self::Custom => "Custom",
        }
    }

    fn meta(self) -> &'static str {
        match self {
            Self::Ollama => "Embedded",
            Self::OpenCode | Self::Codex => "Registry",
            Self::Custom => "stdio",
        }
    }

    fn registry_agent_id(self) -> Option<&'static str> {
        match self {
            Self::OpenCode => Some(OPENCODE_REGISTRY_AGENT_ID),
            Self::Codex => Some(CODEX_REGISTRY_AGENT_ID),
            Self::Ollama | Self::Custom => None,
        }
    }

    fn registry_name(self) -> Option<&'static str> {
        match self {
            Self::OpenCode => Some("OpenCode"),
            Self::Codex => Some("Codex CLI"),
            Self::Ollama | Self::Custom => None,
        }
    }

    fn registry_hint(self) -> Option<&'static str> {
        match self {
            Self::OpenCode => Some("OpenCode from the ACP registry."),
            Self::Codex => Some("Codex CLI from the ACP registry."),
            Self::Ollama | Self::Custom => None,
        }
    }
}

fn setup_mode_button_class(mode: AgentSetupMode, active_mode: AgentSetupMode) -> &'static str {
    if mode == active_mode {
        "button button--ghost button--active agent-panel__mode-button"
    } else {
        "button button--ghost agent-panel__mode-button"
    }
}

#[component]
pub fn AcpAgentPanel(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    mut show_sql_editor: Signal<bool>,
    sql_connection_label: String,
) -> Element {
    let state = panel_state();
    let sql_generation_label = sql_connection_label.clone();
    let chat_label = sql_connection_label.clone();
    let enter_chat_label = chat_label.clone();
    let mut setup_mode = use_signal(|| AgentSetupMode::Ollama);
    let mut registry_busy = use_signal(|| false);
    let mut registry_status = use_signal(String::new);
    let registry_agents =
        use_resource(move || async move { acp::load_acp_registry_agents().await });
    let registry_result = registry_agents();
    let selected_registry_mode = setup_mode();
    let selected_registry_agent = selected_registry_mode
        .registry_agent_id()
        .and_then(|agent_id| {
            registry_result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .and_then(|agents| agents.iter().find(|agent| agent.id == agent_id))
                .cloned()
        });
    let visible_messages = state
        .messages
        .clone()
        .into_iter()
        .filter(|message| is_visible_message(&message.kind, &message.text))
        .collect::<Vec<_>>();

    rsx! {
        aside { class: "agent-panel",
            div { class: "agent-panel__header",
                div { class: "agent-panel__header-copy",
                    h3 { class: "agent-panel__title", "ACP Agent" }
                    p { class: "agent-panel__meta", "{state.status}" }
                }
                div { class: "agent-panel__header-actions",
                    if let Some(connection) = state.connection.clone() {
                        div { class: "agent-panel__badge",
                            "{connection.agent_name}"
                        }
                    }
                    if state.connected && state.busy {
                        IconButton {
                            icon: ActionIcon::Clear,
                            label: "Cancel request".to_string(),
                            onclick: move |_| {
                                if let Err(err) = acp::cancel_acp_prompt() {
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
                            small: true,
                            disabled: !state.busy,
                        }
                    }
                    if state.connected {
                        IconButton {
                            icon: ActionIcon::Close,
                            label: "Disconnect agent".to_string(),
                            onclick: move |_| {
                                let _ = acp::disconnect_acp_agent();
                                panel_state.with_mut(|state| {
                                    state.connected = false;
                                    state.busy = false;
                                    state.pending_sql_insert = false;
                                    state.connection = None;
                                    state.status = "ACP agent is disconnected.".to_string();
                                });
                            },
                            small: true,
                        }
                    }
                }
            }

            if state.connected {
                div { class: "agent-panel__session",
                    div { class: "agent-panel__messages",
                        if visible_messages.is_empty() {
                            p { class: "empty-state", "Ask for SQL, schema, or data help." }
                        } else {
                            for message in visible_messages {
                                article {
                                    class: format!("agent-panel__message agent-panel__message--{}", message_kind_class(&message.kind)),
                                    div { class: "agent-panel__message-meta",
                                        p { class: "agent-panel__message-role", "{message_kind_label(&message.kind)}" }
                                        if matches!(message.kind, AcpMessageKind::Thought) {
                                            div { class: "agent-panel__thinking",
                                                span { class: "agent-panel__thinking-dot" }
                                                span { class: "agent-panel__thinking-dot" }
                                                span { class: "agent-panel__thinking-dot" }
                                            }
                                        }
                                    }
                                    if !matches!(message.kind, AcpMessageKind::Thought) {
                                        pre { class: "agent-panel__message-body", "{message.text}" }
                                    }
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
                                                    "Insert SQL"
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
                                p { class: "agent-panel__message-role", "Permission" }
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
                                                match acp::respond_acp_permission(
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
                                            match acp::respond_acp_permission(request_id, None) {
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
                                    }
                                },
                                "Cancel"
                            }
                        }
                    }

                    div { class: "agent-panel__composer",
                        textarea {
                            class: "input agent-panel__prompt",
                            value: "{state.prompt}",
                            placeholder: "For example: show active users created today",
                            oninput: move |event| {
                                let value = event.value();
                                panel_state.with_mut(|state| state.prompt = value);
                            },
                            onkeydown: move |event| {
                                if event.key() != Key::Enter
                                    || event.modifiers().contains(Modifiers::SHIFT)
                                {
                                    return;
                                }
                                event.prevent_default();
                                send_chat_prompt_request(
                                    panel_state,
                                    tabs,
                                    active_tab_id(),
                                    enter_chat_label.clone(),
                                );
                            }
                        }
                        div { class: "agent-panel__composer-actions",
                            button {
                                class: "button button--ghost button--small",
                                disabled: state.busy || state.prompt.trim().is_empty(),
                                onclick: move |_| {
                                    let request = panel_state().prompt.trim().to_string();
                                    if request.is_empty() {
                                        return;
                                }
                                let connection = active_editor_connection(tabs, active_tab_id());
                                let focus_source =
                                    active_editor_focus_source(tabs, active_tab_id());
                                let active_tab_context =
                                    active_editor_prompt_context(tabs, active_tab_id());
                                let connection_label = sql_generation_label.clone();
                                panel_state.with_mut(|state| {
                                    state.busy = true;
                                        state.pending_sql_insert = true;
                                        state.status =
                                            "Preparing connected database context for the agent..."
                                                .to_string();
                                    });
                                    spawn(async move {
                                        let prompt = match connection {
                                            Some(connection) => {
                                                match acp::build_acp_database_context(
                                                    connection,
                                                    connection_label.clone(),
                                                    focus_source,
                                                )
                                            .await
                                            {
                                                Ok(db_context) => build_sql_generation_prompt(
                                                    &connection_label,
                                                    &request,
                                                    Some(db_context),
                                                    active_tab_context.clone(),
                                                ),
                                                Err(_) => build_sql_generation_prompt(
                                                    &connection_label,
                                                    &request,
                                                    None,
                                                    active_tab_context.clone(),
                                                ),
                                            }
                                        }
                                        None => build_sql_generation_prompt(
                                            &connection_label,
                                            &request,
                                            None,
                                            active_tab_context.clone(),
                                        ),
                                    };

                                        match acp::send_acp_prompt(prompt) {
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
                                                        "Waiting for agent SQL to insert into the editor..."
                                                            .to_string();
                                                });
                                            }
                                            Err(err) => {
                                                panel_state.with_mut(|state| {
                                                    state.status = err.clone();
                                                    state.busy = false;
                                                    state.pending_sql_insert = false;
                                                    push_message(state, AcpMessageKind::Error, err);
                                                });
                                            }
                                        }
                                    });
                                },
                                "Generate SQL"
                            }
                            button {
                                class: "button button--primary button--small",
                                disabled: state.busy || state.prompt.trim().is_empty(),
                                onclick: move |_| {
                                    send_chat_prompt_request(
                                        panel_state,
                                        tabs,
                                        active_tab_id(),
                                        chat_label.clone(),
                                    );
                                },
                                "Send"
                            }
                        }
                    }
                }
            } else {
                div { class: "agent-panel__connect",
                    div { class: "agent-panel__mode-switch",
                        for mode in AgentSetupMode::ALL {
                            button {
                                class: setup_mode_button_class(mode, setup_mode()),
                                onclick: move |_| setup_mode.set(mode),
                                span { class: "agent-panel__mode-name", "{mode.label()}" }
                                span { class: "agent-panel__mode-kind", "{mode.meta()}" }
                            }
                        }
                    }

                    {match setup_mode() {
                        AgentSetupMode::Ollama => rsx! {
                            div { class: "agent-panel__section",
                                div { class: "agent-panel__section-header",
                                    div { class: "agent-panel__section-copy",
                                        h4 { class: "agent-panel__section-title", "Built-in Ollama ACP" }
                                        p { class: "agent-panel__hint", "Local or remote `/api` endpoint." }
                                    }
                                    span { class: "agent-panel__badge", "Embedded" }
                                }
                                div { class: "agent-panel__field-grid",
                                    div { class: "field",
                                        label { class: "field__label", "Base URL" }
                                        input {
                                            class: "input",
                                            value: "{state.ollama.base_url}",
                                            placeholder: "http://localhost:11434/api",
                                            oninput: move |event| {
                                                let value = event.value();
                                                panel_state.with_mut(|state| state.ollama.base_url = value);
                                            }
                                        }
                                    }
                                    div { class: "field",
                                        label { class: "field__label", "Model" }
                                        input {
                                            class: "input",
                                            value: "{state.ollama.model}",
                                            placeholder: "qwen3:latest",
                                            oninput: move |event| {
                                                let value = event.value();
                                                panel_state.with_mut(|state| state.ollama.model = value);
                                            }
                                        }
                                    }
                                }
                                div { class: "field",
                                    label { class: "field__label", "API key" }
                                    input {
                                        class: "input",
                                        r#type: "password",
                                        value: "{state.ollama.api_key}",
                                        placeholder: "Optional bearer token",
                                        oninput: move |event| {
                                            let value = event.value();
                                            panel_state.with_mut(|state| state.ollama.api_key = value);
                                        }
                                    }
                                }
                                button {
                                    class: "button button--primary button--small",
                                    disabled: state.busy || state.ollama.model.trim().is_empty(),
                                    onclick: move |_| {
                                        let cwd = panel_state().launch.cwd.clone();
                                        let ollama = panel_state().ollama.clone();
                                        panel_state.with_mut(|state| {
                                            state.busy = true;
                                            state.status = format!(
                                                "Connecting to Ollama model {}...",
                                                ollama.model.trim()
                                            );
                                        });
                                        spawn(async move {
                                            match acp::build_embedded_ollama_launch(cwd, ollama.clone()) {
                                                Ok(launch) => {
                                                    panel_state.with_mut(|state| {
                                                        state.launch = launch.clone();
                                                        state.status = format!(
                                                            "Launching embedded Ollama ACP bridge for {}...",
                                                            ollama.model.trim()
                                                        );
                                                    });

                                                    match acp::connect_acp_agent(launch).await {
                                                        Ok(connection) => {
                                                            panel_state.with_mut(|state| {
                                                                apply_connected(state, connection);
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
                                                }
                                                Err(err) => {
                                                    panel_state.with_mut(|state| {
                                                        state.busy = false;
                                                        state.status = err.clone();
                                                        push_message(state, AcpMessageKind::Error, err);
                                                    });
                                                }
                                            }
                                        });
                                    },
                                    "Connect Ollama"
                                }
                            }
                        },
                        AgentSetupMode::OpenCode | AgentSetupMode::Codex => rsx! {
                            div { class: "agent-panel__section",
                                {
                                    let registry_name = selected_registry_mode
                                        .registry_name()
                                        .unwrap_or("Registry agent");
                                    let registry_hint = selected_registry_mode
                                        .registry_hint()
                                        .unwrap_or("Quick start from the ACP registry.");
                                    let registry_agent_id = selected_registry_mode
                                        .registry_agent_id()
                                        .unwrap_or_default()
                                        .to_string();

                                    rsx! {
                                div { class: "agent-panel__section-header",
                                    div { class: "agent-panel__section-copy",
                                        h4 { class: "agent-panel__section-title", "{registry_name}" }
                                        p { class: "agent-panel__hint", "{registry_hint}" }
                                    }
                                    span { class: "agent-panel__badge", "Registry" }
                                }
                                if !registry_status().trim().is_empty() {
                                    p { class: "agent-panel__hint agent-panel__hint--status", "{registry_status}" }
                                }
                                if let Some(agent) = selected_registry_agent {
                                    RegistryAgentCard {
                                        agent,
                                        busy: registry_busy(),
                                        on_connect: move |_| {
                                            let cwd = panel_state().launch.cwd.clone();
                                            let registry_name = registry_name.to_string();
                                            let registry_agent_id = registry_agent_id.clone();
                                            registry_busy.set(true);
                                            registry_status.set(format!(
                                                "Preparing {registry_name} from the ACP registry..."
                                            ));
                                            spawn(async move {
                                                match acp::install_acp_registry_agent(registry_agent_id, cwd).await {
                                                    Ok(launch) => {
                                                        panel_state.with_mut(|state| {
                                                            state.launch = launch.clone();
                                                            state.busy = true;
                                                            state.status =
                                                                format!("Connecting to {registry_name}...");
                                                        });
                                                        match acp::connect_acp_agent(launch).await {
                                                            Ok(connection) => {
                                                                panel_state.with_mut(|state| {
                                                                    apply_connected(state, connection);
                                                                });
                                                                registry_status.set(format!(
                                                                    "{registry_name} connected."
                                                                ));
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
                                            p { class: "agent-panel__hint", "{registry_name} is not available in the ACP registry for this platform." }
                                        },
                                        Err(err) => rsx! {
                                            p { class: "agent-panel__hint", "Failed to load ACP registry: {err}" }
                                        },
                                    }
                                } else {
                                    p { class: "agent-panel__hint", "Loading ACP registry..." }
                                }
                                    }
                                }
                            }
                        },
                        AgentSetupMode::Custom => rsx! {
                            div { class: "agent-panel__section",
                                div { class: "agent-panel__section-header",
                                    div { class: "agent-panel__section-copy",
                                        h4 { class: "agent-panel__section-title", "Custom ACP agent" }
                                        p { class: "agent-panel__hint", "Connect any ACP-compatible binary over stdio." }
                                    }
                                    span { class: "agent-panel__badge", "stdio" }
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
                                            match acp::connect_acp_agent(launch).await {
                                                Ok(connection) => {
                                                    panel_state.with_mut(|state| {
                                                        apply_connected(state, connection);
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
                        },
                    }}
                }
            }
        }
    }
}
