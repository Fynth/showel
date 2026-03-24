#[path = "agent_panel/prompt.rs"]
mod prompt;
#[path = "agent_panel/registry_card.rs"]
mod registry_card;
#[path = "agent_panel/state.rs"]
mod state;

use dioxus::prelude::*;
use models::{AcpMessageKind, AcpPanelState, ChatArtifact, ChatThreadSummary, QueryTabState};

use crate::screens::workspace::actions::{run_query_for_tab, tab_connection_or_error};

use super::{ActionIcon, IconButton};

use self::{
    prompt::{
        active_editor_connection, active_editor_focus_source, active_editor_prompt_context,
        build_chat_prompt, build_sql_generation_prompt, build_thread_history_context,
        insert_sql_into_editor,
    },
    registry_card::RegistryAgentCard,
    state::{
        apply_connected, message_kind_class, message_kind_label, permission_button_class,
        push_message, push_message_with_artifact,
    },
};

pub(crate) use self::{
    prompt::extract_sql_candidate,
    state::{apply_acp_events, default_acp_panel_state, replace_messages},
};

fn send_chat_prompt_request(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    connection_label: String,
    mut chat_revision: Signal<u64>,
    allow_db_read: bool,
    prompt: String,
    mut prompt_draft: Signal<String>,
) {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() || panel_state().busy {
        return;
    }

    let thread_history = build_thread_history_context(&panel_state().messages);
    let connection = if allow_db_read {
        active_editor_connection(tabs, active_tab_id)
    } else {
        None
    };
    let focus_source = active_editor_focus_source(tabs, active_tab_id);
    let active_tab_context = if allow_db_read {
        active_editor_prompt_context(tabs, active_tab_id)
    } else {
        None
    };
    panel_state.with_mut(|state| {
        state.busy = true;
        state.pending_sql_insert = false;
        state.status = if allow_db_read {
            "Preparing connected database context for the agent...".to_string()
        } else {
            "Preparing prompt for the agent...".to_string()
        };
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
                        thread_history.clone(),
                    ),
                    Err(_) => build_chat_prompt(
                        &connection_label,
                        &prompt,
                        None,
                        active_tab_context.clone(),
                        thread_history.clone(),
                    ),
                }
            }
            None => build_chat_prompt(
                &connection_label,
                &prompt,
                None,
                active_tab_context.clone(),
                thread_history.clone(),
            ),
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
                prompt_draft.set(String::new());
                chat_revision += 1;
            }
            Err(err) => {
                panel_state.with_mut(|state| {
                    state.status = err.clone();
                    state.busy = false;
                    push_message(state, AcpMessageKind::Error, err);
                });
                chat_revision += 1;
            }
        }
    });
}

fn send_sql_generation_request(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    connection_label: String,
    mut chat_revision: Signal<u64>,
    allow_db_read: bool,
    prompt: String,
    mut prompt_draft: Signal<String>,
) {
    let request = prompt.trim().to_string();
    if request.is_empty() || panel_state().busy {
        return;
    }

    let connection = if allow_db_read {
        active_editor_connection(tabs, active_tab_id)
    } else {
        None
    };
    let focus_source = active_editor_focus_source(tabs, active_tab_id);
    let active_tab_context = if allow_db_read {
        active_editor_prompt_context(tabs, active_tab_id)
    } else {
        None
    };
    let thread_history = build_thread_history_context(&panel_state().messages);
    panel_state.with_mut(|state| {
        state.busy = true;
        state.pending_sql_insert = true;
        state.status = if allow_db_read {
            "Preparing connected database context for the agent...".to_string()
        } else {
            "Preparing prompt for the agent...".to_string()
        };
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
                        thread_history.clone(),
                    ),
                    Err(_) => build_sql_generation_prompt(
                        &connection_label,
                        &request,
                        None,
                        active_tab_context.clone(),
                        thread_history.clone(),
                    ),
                }
            }
            None => build_sql_generation_prompt(
                &connection_label,
                &request,
                None,
                active_tab_context.clone(),
                thread_history.clone(),
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
                    state.status = "Waiting for agent SQL to insert into the editor...".to_string();
                });
                prompt_draft.set(String::new());
                chat_revision += 1;
            }
            Err(err) => {
                panel_state.with_mut(|state| {
                    state.status = err.clone();
                    state.busy = false;
                    state.pending_sql_insert = false;
                    push_message(state, AcpMessageKind::Error, err);
                });
                chat_revision += 1;
            }
        }
    });
}

pub(crate) fn execute_agent_sql_request(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    show_sql_editor: Signal<bool>,
    mut chat_revision: Signal<u64>,
    sql: String,
    execution_mode: AgentSqlExecutionMode,
) {
    if active_tab_id == 0 {
        panel_state.with_mut(|state| {
            state.status = "No active SQL tab to execute in.".to_string();
            push_message(
                state,
                AcpMessageKind::Error,
                "No active SQL tab to execute in.".to_string(),
            );
        });
        chat_revision += 1;
        return;
    }

    let current_tab = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id)
        .cloned();
    let Some(current_tab) = current_tab else {
        panel_state.with_mut(|state| {
            state.status = "Active SQL tab was not found.".to_string();
            push_message(
                state,
                AcpMessageKind::Error,
                "Active SQL tab was not found.".to_string(),
            );
        });
        chat_revision += 1;
        return;
    };

    let Some(connection) = tab_connection_or_error(tabs, current_tab.id, current_tab.session_id)
    else {
        panel_state.with_mut(|state| {
            state.status = "The active tab connection is not available.".to_string();
            push_message(
                state,
                AcpMessageKind::Error,
                "The active tab connection is not available.".to_string(),
            );
        });
        chat_revision += 1;
        return;
    };

    insert_sql_into_editor(
        panel_state,
        tabs,
        active_tab_id,
        show_sql_editor,
        sql.clone(),
    );

    let (status, summary) = match execution_mode {
        AgentSqlExecutionMode::Manual => (
            "Executing agent SQL in the active tab...".to_string(),
            "Executed agent SQL in the active SQL tab.".to_string(),
        ),
        AgentSqlExecutionMode::AutoReadOnly => (
            "Automatically running read-only SQL from ACP agent...".to_string(),
            "Automatically executed read-only SQL from ACP agent.".to_string(),
        ),
    };

    panel_state.with_mut(|state| {
        state.status = status;
        push_message_with_artifact(
            state,
            AcpMessageKind::System,
            summary.clone(),
            Some(ChatArtifact::QuerySummary {
                sql: sql.clone(),
                summary,
            }),
        );
    });
    chat_revision += 1;

    run_query_for_tab(
        tabs,
        current_tab.id,
        connection,
        sql,
        0,
        current_tab.page_size,
        None,
    );
}

fn is_connection_notice(kind: &AcpMessageKind, text: &str) -> bool {
    matches!(kind, AcpMessageKind::System) && text.starts_with("Connected to ")
}

fn is_visible_message(kind: &AcpMessageKind, text: &str) -> bool {
    !is_connection_notice(kind, text) && !matches!(kind, AcpMessageKind::Tool)
}

#[derive(Clone, PartialEq, Eq)]
enum MessageChunk {
    Text(String),
    Code {
        language: Option<String>,
        code: String,
    },
}

#[derive(Clone, PartialEq, Eq)]
enum TextSegment {
    Plain(String),
    InlineCode(String),
}

fn parse_message_chunks(text: &str) -> Vec<MessageChunk> {
    let mut chunks = Vec::new();
    let mut cursor = 0;

    while let Some(start_offset) = text[cursor..].find("```") {
        let start = cursor + start_offset;
        let before = text[cursor..start].trim();
        if !before.is_empty() {
            chunks.push(MessageChunk::Text(before.to_string()));
        }

        let fence_meta_start = start + 3;
        let Some(meta_end_offset) = text[fence_meta_start..].find('\n') else {
            break;
        };
        let meta_end = fence_meta_start + meta_end_offset;
        let language = text[fence_meta_start..meta_end].trim().to_string();
        let code_start = meta_end + 1;
        let Some(code_end_offset) = text[code_start..].find("```") else {
            break;
        };
        let code_end = code_start + code_end_offset;
        let code = text[code_start..code_end].trim();

        if !code.is_empty() {
            chunks.push(MessageChunk::Code {
                language: (!language.is_empty()).then_some(language),
                code: code.to_string(),
            });
        }

        cursor = code_end + 3;
    }

    let remaining = text[cursor..].trim();
    if !remaining.is_empty() {
        chunks.push(MessageChunk::Text(remaining.to_string()));
    }

    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(MessageChunk::Text(text.trim().to_string()));
    }

    chunks
}

fn parse_inline_code_segments(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut cursor = 0;

    while let Some(start_offset) = text[cursor..].find('`') {
        let start = cursor + start_offset;
        if start > cursor {
            segments.push(TextSegment::Plain(text[cursor..start].to_string()));
        }

        let code_start = start + 1;
        let Some(end_offset) = text[code_start..].find('`') else {
            segments.push(TextSegment::Plain(text[start..].to_string()));
            cursor = text.len();
            break;
        };
        let code_end = code_start + end_offset;
        let code = &text[code_start..code_end];

        if code.is_empty() {
            segments.push(TextSegment::Plain("``".to_string()));
        } else {
            segments.push(TextSegment::InlineCode(code.to_string()));
        }

        cursor = code_end + 1;
    }

    if cursor < text.len() {
        segments.push(TextSegment::Plain(text[cursor..].to_string()));
    }

    if segments.is_empty() {
        segments.push(TextSegment::Plain(text.to_string()));
    }

    segments
}

fn code_chunk_sql(language: Option<&str>, code: &str) -> Option<String> {
    if language.is_some_and(|value| value.eq_ignore_ascii_case("sql")) {
        return Some(code.trim().to_string());
    }

    extract_sql_candidate(code)
        .filter(|candidate| candidate.trim() == code.trim())
        .map(|candidate| candidate.trim().to_string())
}

fn can_execute_agent_sql(sql: &str, allow_read_sql_run: bool, allow_write_sql_run: bool) -> bool {
    if query::is_read_only_sql(sql) {
        allow_read_sql_run
    } else {
        allow_write_sql_run
    }
}

fn copy_text_to_clipboard(mut panel_state: Signal<AcpPanelState>, text: String, label: &str) {
    let result = arboard::Clipboard::new()
        .map_err(|err| err.to_string())
        .and_then(|mut clipboard| clipboard.set_text(text).map_err(|err| err.to_string()));

    panel_state.with_mut(|state| {
        state.status = match result {
            Ok(()) => format!("Copied {label} to clipboard."),
            Err(err) => format!("Clipboard error: {err}"),
        };
    });
}

#[component]
fn AgentComposer(
    panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
    chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    busy: bool,
    connection_label: String,
    reset_key: String,
) -> Element {
    let mut prompt_draft = use_signal(String::new);

    use_effect(move || {
        let _ = reset_key.as_str();
        prompt_draft.set(String::new());
    });

    let prompt_is_empty = prompt_draft().trim().is_empty();
    let enter_chat_label = connection_label.clone();
    let generate_sql_label = connection_label.clone();
    let chat_label = connection_label.clone();

    rsx! {
        div { class: "agent-panel__composer",
            div { class: "agent-panel__permissions",
                label { class: "agent-panel__permission-toggle",
                    input {
                        r#type: "checkbox",
                        checked: allow_agent_db_read(),
                        onchange: move |event| {
                            allow_agent_db_read.set(event.checked());
                        }
                    }
                    span { "Allow ACP to read database context" }
                }
                label { class: "agent-panel__permission-toggle",
                    input {
                        r#type: "checkbox",
                        checked: allow_agent_read_sql_run(),
                        onchange: move |event| {
                            allow_agent_read_sql_run.set(event.checked());
                        }
                    }
                    span { "Allow ACP to execute read-only SQL in the active tab" }
                }
                label { class: "agent-panel__permission-toggle",
                    input {
                        r#type: "checkbox",
                        checked: allow_agent_write_sql_run(),
                        onchange: move |event| {
                            allow_agent_write_sql_run.set(event.checked());
                        }
                    }
                    span { "Allow ACP to execute write SQL in the active tab" }
                }
                label { class: "agent-panel__permission-toggle",
                    input {
                        r#type: "checkbox",
                        checked: allow_agent_tool_run(),
                        onchange: move |event| {
                            allow_agent_tool_run.set(event.checked());
                        }
                    }
                    span { "Allow ACP tools and code execution" }
                }
            }
            textarea {
                class: "input agent-panel__prompt",
                value: "{prompt_draft}",
                placeholder: "For example: show active users created today",
                oninput: move |event| prompt_draft.set(event.value()),
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
                        chat_revision,
                        allow_agent_db_read(),
                        prompt_draft(),
                        prompt_draft,
                    );
                }
            }
            div { class: "agent-panel__composer-actions",
                button {
                    class: "button button--ghost button--small",
                    disabled: busy || prompt_is_empty,
                    onclick: move |_| {
                        send_sql_generation_request(
                            panel_state,
                            tabs,
                            active_tab_id(),
                            generate_sql_label.clone(),
                            chat_revision,
                            allow_agent_db_read(),
                            prompt_draft(),
                            prompt_draft,
                        );
                    },
                    "Generate SQL"
                }
                button {
                    class: "button button--primary button--small",
                    disabled: busy || prompt_is_empty,
                    onclick: move |_| {
                        send_chat_prompt_request(
                            panel_state,
                            tabs,
                            active_tab_id(),
                            chat_label.clone(),
                            chat_revision,
                            allow_agent_db_read(),
                            prompt_draft(),
                            prompt_draft,
                        );
                    },
                    "Send"
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentSqlExecutionMode {
    Manual,
    AutoReadOnly,
}

const OPENCODE_REGISTRY_AGENT_ID: &str = "opencode";
const CODEX_REGISTRY_AGENT_ID: &str = "codex-acp";
const AGENT_MESSAGE_BATCH: usize = 32;

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
    mut chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    chat_threads: Vec<ChatThreadSummary>,
    active_thread_id: Option<i64>,
    thread_title: String,
    thread_connection_name: String,
    sql_connection_label: String,
    on_new_thread: EventHandler<()>,
    on_select_thread: EventHandler<i64>,
    on_delete_thread: EventHandler<i64>,
) -> Element {
    let state = panel_state();
    let thread_meta = if thread_connection_name.trim().is_empty() {
        state.status.clone()
    } else {
        format!("{} · {}", thread_connection_name.trim(), state.status)
    };
    let chat_label = sql_connection_label.clone();
    let mut setup_mode = use_signal(|| AgentSetupMode::Ollama);
    let mut show_dialogs = use_signal(|| false);
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
    let visible_message_total = visible_messages.len();
    let mut message_limit = use_signal(|| AGENT_MESSAGE_BATCH);
    let hidden_message_count = visible_message_total.saturating_sub(message_limit());
    let rendered_messages = visible_messages
        .iter()
        .skip(hidden_message_count)
        .cloned()
        .collect::<Vec<_>>();

    use_effect(move || {
        let _ = active_thread_id;
        message_limit.set(AGENT_MESSAGE_BATCH);
    });

    use_effect(move || {
        let Some(permission_request) = panel_state().pending_permission.clone() else {
            return;
        };

        if allow_agent_tool_run() {
            return;
        }

        match acp::respond_acp_permission(permission_request.request_id, None) {
            Ok(()) => {
                panel_state.with_mut(|state| {
                    state.pending_permission = None;
                    state.status = "Blocked ACP tool request because tools/code execution is disabled.".to_string();
                    push_message(
                        state,
                        AcpMessageKind::System,
                        format!(
                            "Blocked ACP tool request because tools/code execution is disabled.\n{}",
                            permission_request.tool_summary
                        ),
                    );
                });
                chat_revision += 1;
            }
            Err(err) => {
                panel_state.with_mut(|state| {
                    state.status = err.clone();
                    push_message(state, AcpMessageKind::Error, err);
                });
                chat_revision += 1;
            }
        }
    });

    rsx! {
        aside { class: "agent-panel",
            div { class: "agent-panel__header",
                div { class: "agent-panel__header-copy",
                    h3 { class: "agent-panel__title", "{thread_title}" }
                    p { class: "agent-panel__meta", "{thread_meta}" }
                }
                div { class: "agent-panel__header-actions",
                    button {
                        class: if show_dialogs() {
                            "button button--ghost button--small button--active"
                        } else {
                            "button button--ghost button--small"
                        },
                        onclick: move |_| show_dialogs.set(!show_dialogs()),
                        "Dialogs"
                    }
                    div { class: "agent-panel__badge", "Database Chat" }
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
                                    chat_revision += 1;
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
            if show_dialogs() {
                div {
                    class: "agent-panel__dialogs-popover",
                    onclick: move |event| event.stop_propagation(),
                    div { class: "agent-panel__dialogs-header",
                        div { class: "agent-panel__dialogs-copy",
                            h4 { class: "agent-panel__section-title", "Dialogs" }
                            p { class: "agent-panel__hint", "Switch or create a persistent database chat." }
                        }
                        button {
                            class: "button button--primary button--small",
                            onclick: move |_| {
                                on_new_thread.call(());
                                show_dialogs.set(false);
                            },
                            "New chat"
                        }
                    }
                    div { class: "agent-panel__dialogs-list",
                        if chat_threads.is_empty() {
                            p { class: "empty-state", "No saved dialogs yet." }
                        } else {
                            for thread in chat_threads {
                                article {
                                    class: if Some(thread.id) == active_thread_id {
                                        "agent-panel__dialog-item agent-panel__dialog-item--active"
                                    } else {
                                        "agent-panel__dialog-item"
                                    },
                                    button {
                                        class: "agent-panel__dialog-main",
                                        onclick: {
                                            let thread_id = thread.id;
                                            move |_| {
                                                on_select_thread.call(thread_id);
                                                show_dialogs.set(false);
                                            }
                                        },
                                        div { class: "agent-panel__dialog-copy",
                                            p { class: "agent-panel__dialog-title", "{thread.title}" }
                                            p { class: "agent-panel__dialog-meta", "{thread.connection_name}" }
                                            if !thread.last_message_preview.trim().is_empty() {
                                                p {
                                                    class: "agent-panel__dialog-preview",
                                                    "{thread.last_message_preview}"
                                                }
                                            }
                                        }
                                    }
                                    button {
                                        class: "agent-panel__dialog-delete",
                                        title: "Delete dialog",
                                        onclick: {
                                            let thread_id = thread.id;
                                            move |event| {
                                                event.stop_propagation();
                                                on_delete_thread.call(thread_id);
                                            }
                                        },
                                        "x"
                                    }
                                }
                            }
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
                            if hidden_message_count > 0 {
                                div { class: "agent-panel__message-actions",
                                    button {
                                        class: "button button--ghost button--small",
                                        onclick: move |_| {
                                            message_limit.set(
                                                (message_limit() + AGENT_MESSAGE_BATCH)
                                                    .min(visible_message_total),
                                            );
                                        },
                                        "Show {hidden_message_count.min(AGENT_MESSAGE_BATCH)} older messages"
                                    }
                                    button {
                                        class: "button button--ghost button--small",
                                        onclick: move |_| message_limit.set(visible_message_total),
                                        "Show all"
                                    }
                                }
                            }
                            for message in rendered_messages {
                                {
                                    let message_chunks = parse_message_chunks(&message.text);
                                    let has_sql_chunk =
                                        message_chunks.iter().any(|chunk| match chunk {
                                            MessageChunk::Code { language, code } => {
                                                code_chunk_sql(language.as_deref(), code).is_some()
                                            }
                                            MessageChunk::Text(_) => false,
                                        });

                                    rsx! {
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
                                                for chunk in message_chunks {
                                                    match chunk {
                                                        MessageChunk::Text(text) => rsx! {
                                                            p { class: "agent-panel__message-text",
                                                                for segment in parse_inline_code_segments(&text) {
                                                                    match segment {
                                                                        TextSegment::Plain(value) => rsx! { span { "{value}" } },
                                                                        TextSegment::InlineCode(value) => rsx! {
                                                                            code { class: "agent-panel__inline-code", "{value}" }
                                                                        },
                                                                    }
                                                                }
                                                            }
                                                        },
                                                        MessageChunk::Code { language, code } => {
                                                            let sql = code_chunk_sql(language.as_deref(), &code);
                                                            let language_label = language
                                                                .clone()
                                                                .filter(|value| !value.trim().is_empty())
                                                                .unwrap_or_else(|| {
                                                                    if sql.is_some() {
                                                                        "SQL".to_string()
                                                                    } else {
                                                                        "Code".to_string()
                                                                    }
                                                                });

                                                            rsx! {
                                                                div { class: "agent-panel__code-card",
                                                                    div { class: "agent-panel__code-header",
                                                                        span { class: "agent-panel__code-language", "{language_label}" }
                                                                        div { class: "agent-panel__code-actions",
                                                                            button {
                                                                                class: "button button--ghost button--small",
                                                                                onclick: {
                                                                                    let code = code.clone();
                                                                                    move |_| {
                                                                                        copy_text_to_clipboard(panel_state, code.clone(), if sql.is_some() { "SQL" } else { "code" });
                                                                                    }
                                                                                },
                                                                                if sql.is_some() { "Copy SQL" } else { "Copy" }
                                                                            }
                                                                            if let Some(sql) = sql.clone() {
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
                                                                                button {
                                                                                    class: "button button--primary button--small",
                                                                                    disabled: !can_execute_agent_sql(
                                                                                        &sql,
                                                                                        allow_agent_read_sql_run(),
                                                                                        allow_agent_write_sql_run(),
                                                                                    ),
                                                                                    onclick: {
                                                                                        let sql = sql.clone();
                                                                                        move |_| {
                                                                                            execute_agent_sql_request(
                                                                                                panel_state,
                                                                                                tabs,
                                                                                                active_tab_id(),
                                                                                                show_sql_editor,
                                                                                                chat_revision,
                                                                                                sql.clone(),
                                                                                                AgentSqlExecutionMode::Manual,
                                                                                            );
                                                                                        }
                                                                                    },
                                                                                    "Run SQL"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    pre { class: "agent-panel__code-body", "{code}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            if let Some(artifact) = message.artifact.clone() {
                                                match artifact {
                                                    ChatArtifact::SqlDraft { sql } => rsx! {
                                                        div { class: "agent-panel__artifact",
                                                            div { class: "agent-panel__artifact-header",
                                                                p { class: "agent-panel__artifact-label", "SQL Draft" }
                                                                div { class: "agent-panel__artifact-actions",
                                                                    button {
                                                                        class: "button button--ghost button--small",
                                                                        onclick: {
                                                                            let sql = sql.clone();
                                                                            move |_| copy_text_to_clipboard(panel_state, sql.clone(), "SQL")
                                                                        },
                                                                        "Copy SQL"
                                                                    }
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
                                                                    button {
                                                                        class: "button button--primary button--small",
                                                                        disabled: !can_execute_agent_sql(
                                                                            &sql,
                                                                            allow_agent_read_sql_run(),
                                                                            allow_agent_write_sql_run(),
                                                                        ),
                                                                        onclick: {
                                                                            let sql = sql.clone();
                                                                            move |_| {
                                                                                execute_agent_sql_request(
                                                                                    panel_state,
                                                                                    tabs,
                                                                                    active_tab_id(),
                                                                                    show_sql_editor,
                                                                                    chat_revision,
                                                                                    sql.clone(),
                                                                                    AgentSqlExecutionMode::Manual,
                                                                                );
                                                                            }
                                                                        },
                                                                        "Run SQL"
                                                                    }
                                                                }
                                                            }
                                                            pre { class: "agent-panel__artifact-body", "{sql}" }
                                                        }
                                                    },
                                                    ChatArtifact::QuerySummary { sql, summary } => rsx! {
                                                        div { class: "agent-panel__artifact",
                                                            div { class: "agent-panel__artifact-header",
                                                                p { class: "agent-panel__artifact-label", "{summary}" }
                                                                div { class: "agent-panel__artifact-actions",
                                                                    button {
                                                                        class: "button button--ghost button--small",
                                                                        onclick: {
                                                                            let sql = sql.clone();
                                                                            move |_| copy_text_to_clipboard(panel_state, sql.clone(), "SQL")
                                                                        },
                                                                        "Copy SQL"
                                                                    }
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
                                                                    button {
                                                                        class: "button button--primary button--small",
                                                                        disabled: !can_execute_agent_sql(
                                                                            &sql,
                                                                            allow_agent_read_sql_run(),
                                                                            allow_agent_write_sql_run(),
                                                                        ),
                                                                        onclick: {
                                                                            let sql = sql.clone();
                                                                            move |_| {
                                                                                execute_agent_sql_request(
                                                                                    panel_state,
                                                                                    tabs,
                                                                                    active_tab_id(),
                                                                                    show_sql_editor,
                                                                                    chat_revision,
                                                                                    sql.clone(),
                                                                                    AgentSqlExecutionMode::Manual,
                                                                                );
                                                                            }
                                                                        },
                                                                        "Run SQL"
                                                                    }
                                                                }
                                                            }
                                                            pre { class: "agent-panel__artifact-body", "{sql}" }
                                                        }
                                                    },
                                                }
                                            }
                                            if matches!(message.kind, AcpMessageKind::Agent) && !has_sql_chunk {
                                                if let Some(sql) = extract_sql_candidate(&message.text) {
                                                    {
                                                        let sql_is_read_only = query::is_read_only_sql(&sql);
                                                        rsx! {
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
                                                                button {
                                                                    class: "button button--primary button--small",
                                                                    disabled: !can_execute_agent_sql(
                                                                        &sql,
                                                                        allow_agent_read_sql_run(),
                                                                        allow_agent_write_sql_run(),
                                                                    ),
                                                                    onclick: {
                                                                        let sql = sql.clone();
                                                                        move |_| {
                                                                            execute_agent_sql_request(
                                                                                panel_state,
                                                                                tabs,
                                                                                active_tab_id(),
                                                                                show_sql_editor,
                                                                                chat_revision,
                                                                                sql.clone(),
                                                                                AgentSqlExecutionMode::Manual,
                                                                            );
                                                                        }
                                                                    },
                                                                    if sql_is_read_only { "Run again" } else { "Run SQL" }
                                                                }
                                                            }
                                                        }
                                                    }
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
                                                        chat_revision += 1;
                                                    }
                                                    Err(err) => {
                                                        panel_state.with_mut(|state| {
                                                            state.status = err.clone();
                                                            push_message(state, AcpMessageKind::Error, err);
                                                        });
                                                        chat_revision += 1;
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
                                                    chat_revision += 1;
                                                }
                                                Err(err) => {
                                                    panel_state.with_mut(|state| {
                                                        state.status = err.clone();
                                                        push_message(state, AcpMessageKind::Error, err);
                                                    });
                                                    chat_revision += 1;
                                                }
                                            }
                                        }
                                    }
                                },
                                "Cancel"
                            }
                        }
                    }

                    AgentComposer {
                        key: format!("{:?}-{}", active_thread_id, state.connected),
                        panel_state,
                        tabs,
                        active_tab_id,
                        show_sql_editor,
                        chat_revision,
                        allow_agent_db_read,
                        allow_agent_read_sql_run,
                        allow_agent_write_sql_run,
                        allow_agent_tool_run,
                        busy: state.busy,
                        connection_label: chat_label.clone(),
                        reset_key: format!("{:?}-{}", active_thread_id, state.connected),
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
                                                            chat_revision += 1;
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    panel_state.with_mut(|state| {
                                                        state.busy = false;
                                                        state.status = err.clone();
                                                        push_message(state, AcpMessageKind::Error, err);
                                                    });
                                                    chat_revision += 1;
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
                                                                chat_revision += 1;
                                                                registry_status.set(err);
                                                            }
                                                        }
                                                    }
                                                    Err(err) => {
                                                        panel_state.with_mut(|state| {
                                                            state.status = err.clone();
                                                            push_message(state, AcpMessageKind::Error, err.clone());
                                                        });
                                                        chat_revision += 1;
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
                                                    chat_revision += 1;
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
