#[path = "agent_panel/prompt.rs"]
mod prompt;
#[path = "agent_panel/registry_card.rs"]
mod registry_card;
#[path = "agent_panel/state.rs"]
mod state;

use dioxus::prelude::*;
use models::{AcpMessageKind, AcpPanelState, QueryTabState};

use self::{
    prompt::{
        active_editor_connection, build_chat_prompt, build_sql_generation_prompt,
        insert_sql_into_editor,
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
                                if matches!(message.kind, AcpMessageKind::Thought) {
                                    div { class: "agent-panel__thinking",
                                        span { class: "agent-panel__thinking-dot" }
                                        span { class: "agent-panel__thinking-dot" }
                                        span { class: "agent-panel__thinking-dot" }
                                    }
                                } else if matches!(message.kind, AcpMessageKind::Tool) {
                                    div { class: "agent-panel__tool-emoji", "🛠" }
                                } else {
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
                                let connection = active_editor_connection(tabs, active_tab_id());
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
                                            match services::build_acp_database_context(
                                                connection,
                                                connection_label.clone(),
                                            )
                                            .await
                                            {
                                                Ok(db_context) => build_sql_generation_prompt(
                                                    &connection_label,
                                                    &request,
                                                    Some(db_context),
                                                ),
                                                Err(_) => build_sql_generation_prompt(
                                                    &connection_label,
                                                    &request,
                                                    None,
                                                ),
                                            }
                                        }
                                        None => build_sql_generation_prompt(
                                            &connection_label,
                                            &request,
                                            None,
                                        ),
                                    };

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
                            class: "button button--primary",
                            disabled: state.busy || state.prompt.trim().is_empty(),
                            onclick: move |_| {
                                let prompt = panel_state().prompt.trim().to_string();
                                if prompt.is_empty() {
                                    return;
                                }
                                let connection = active_editor_connection(tabs, active_tab_id());
                                let connection_label = chat_label.clone();
                                panel_state.with_mut(|state| {
                                    state.busy = true;
                                    state.pending_sql_insert = false;
                                    state.status =
                                        "Preparing connected database context for the agent..."
                                            .to_string();
                                });
                                spawn(async move {
                                    let contextual_prompt = match connection {
                                        Some(connection) => {
                                            match services::build_acp_database_context(
                                                connection,
                                                connection_label.clone(),
                                            )
                                            .await
                                            {
                                                Ok(db_context) => build_chat_prompt(
                                                    &connection_label,
                                                    &prompt,
                                                    Some(db_context),
                                                ),
                                                Err(_) => build_chat_prompt(
                                                    &connection_label,
                                                    &prompt,
                                                    None,
                                                ),
                                            }
                                        }
                                        None => {
                                            build_chat_prompt(&connection_label, &prompt, None)
                                        }
                                    };

                                    match services::send_acp_prompt(contextual_prompt) {
                                        Ok(()) => {
                                            panel_state.with_mut(|state| {
                                                push_message(state, AcpMessageKind::User, prompt);
                                                state.prompt.clear();
                                                state.busy = true;
                                                state.pending_sql_insert = false;
                                                state.status =
                                                    "Waiting for agent response...".to_string();
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
