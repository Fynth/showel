mod actions;
mod components;

use crate::app_state::{APP_SHOW_HISTORY, APP_STATE, APP_UI_SETTINGS, open_connection_screen};
use dioxus::{html::input_data::MouseButton, prelude::*};
use models::{
    AcpPanelState, AcpUiMessage, ChatThreadSummary, QueryHistoryItem, QueryTabState, SavedQuery,
    WorkspaceToolDock, WorkspaceToolLayout, WorkspaceToolPanel,
};
use std::collections::HashSet;
use std::time::Duration;

use self::{
    actions::{new_query_tab, update_active_tab_sql},
    components::{
        AcpAgentPanel, ActionIcon, AgentSqlExecutionMode, ExplorerConnectionSection, IconButton,
        QueryHistoryPanel, SavedQueriesPanel, SessionRail, SidebarConnectionTree, TabsManager,
        apply_acp_events, default_acp_panel_state, execute_agent_sql_request,
        extract_sql_candidate, replace_messages,
    },
};

const SIDEBAR_MIN_WIDTH: f64 = 240.0;
const SIDEBAR_MAX_WIDTH: f64 = 560.0;
const INSPECTOR_MIN_WIDTH: f64 = 260.0;
const INSPECTOR_MAX_WIDTH: f64 = 640.0;

#[derive(Clone, Copy, PartialEq)]
struct ColumnResizeState {
    start_x: f64,
    start_width: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DockDropTarget {
    dock: WorkspaceToolDock,
    index: usize,
}

async fn load_explorer_section(
    session: models::ConnectionSession,
    active_session_id: Option<u64>,
) -> ExplorerConnectionSection {
    let kind_label = match session.kind {
        models::DatabaseKind::Sqlite => "SQLite".to_string(),
        models::DatabaseKind::Postgres => "PostgreSQL".to_string(),
        models::DatabaseKind::ClickHouse => "ClickHouse".to_string(),
    };

    match explorer::load_connection_tree(session.connection.clone()).await {
        Ok(nodes) => ExplorerConnectionSection {
            session_id: session.id,
            name: connection_target_label(&session.request),
            kind_label,
            status: "Ready".to_string(),
            is_active: Some(session.id) == active_session_id,
            nodes,
        },
        Err(err) => ExplorerConnectionSection {
            session_id: session.id,
            name: connection_target_label(&session.request),
            kind_label,
            status: format!("Error: {err:?}"),
            is_active: Some(session.id) == active_session_id,
            nodes: Vec::new(),
        },
    }
}

fn connection_target_label(request: &models::ConnectionRequest) -> String {
    request.short_name()
}

fn unloaded_explorer_section(
    session: &models::ConnectionSession,
    active_session_id: Option<u64>,
    status: &str,
) -> ExplorerConnectionSection {
    let kind_label = match session.kind {
        models::DatabaseKind::Sqlite => "SQLite".to_string(),
        models::DatabaseKind::Postgres => "PostgreSQL".to_string(),
        models::DatabaseKind::ClickHouse => "ClickHouse".to_string(),
    };

    ExplorerConnectionSection {
        session_id: session.id,
        name: connection_target_label(&session.request),
        kind_label,
        status: status.to_string(),
        is_active: Some(session.id) == active_session_id,
        nodes: Vec::new(),
    }
}

fn is_tool_panel_visible(
    panel: WorkspaceToolPanel,
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
    ai_features_enabled: bool,
) -> bool {
    match panel {
        WorkspaceToolPanel::Connections => show_connections,
        WorkspaceToolPanel::Explorer => show_explorer,
        WorkspaceToolPanel::SavedQueries => true,
        WorkspaceToolPanel::History => show_history,
        WorkspaceToolPanel::Agent => ai_features_enabled && show_agent_panel,
    }
}

fn visible_tool_panels(
    panels: &[WorkspaceToolPanel],
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
    ai_features_enabled: bool,
) -> Vec<WorkspaceToolPanel> {
    panels
        .iter()
        .copied()
        .filter(|panel| {
            is_tool_panel_visible(
                *panel,
                show_connections,
                show_explorer,
                show_history,
                show_agent_panel,
                ai_features_enabled,
            )
        })
        .collect()
}

fn visible_insert_index(
    panels: &[WorkspaceToolPanel],
    target_visible_index: usize,
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
    ai_features_enabled: bool,
) -> usize {
    if !panels.iter().any(|panel| {
        is_tool_panel_visible(
            *panel,
            show_connections,
            show_explorer,
            show_history,
            show_agent_panel,
            ai_features_enabled,
        )
    }) {
        return 0;
    }

    let mut visible_index = 0;
    for (index, panel) in panels.iter().enumerate() {
        if !is_tool_panel_visible(
            *panel,
            show_connections,
            show_explorer,
            show_history,
            show_agent_panel,
            ai_features_enabled,
        ) {
            continue;
        }

        if visible_index == target_visible_index {
            return index;
        }

        visible_index += 1;
    }

    panels.len()
}

fn move_tool_panel_layout(
    layout: &mut WorkspaceToolLayout,
    panel: WorkspaceToolPanel,
    target: DockDropTarget,
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
    ai_features_enabled: bool,
) {
    let mut normalized = layout.normalized();
    normalized.sidebar.retain(|existing| *existing != panel);
    normalized.inspector.retain(|existing| *existing != panel);

    let target_panels = match target.dock {
        WorkspaceToolDock::Sidebar => &mut normalized.sidebar,
        WorkspaceToolDock::Inspector => &mut normalized.inspector,
    };
    let insert_at = visible_insert_index(
        target_panels,
        target.index,
        show_connections,
        show_explorer,
        show_history,
        show_agent_panel,
        ai_features_enabled,
    )
    .min(target_panels.len());
    target_panels.insert(insert_at, panel);

    *layout = normalized;
}

fn apply_tool_panel_drop(
    mut dragging_panel: Signal<Option<WorkspaceToolPanel>>,
    mut drop_target: Signal<Option<DockDropTarget>>,
    target: DockDropTarget,
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
    ai_features_enabled: bool,
) {
    if let Some(panel) = dragging_panel() {
        APP_UI_SETTINGS.with_mut(|settings| {
            move_tool_panel_layout(
                &mut settings.tool_panel_layout,
                panel,
                target,
                show_connections,
                show_explorer,
                show_history,
                show_agent_panel,
                ai_features_enabled,
            );
        });
    }

    dragging_panel.set(None);
    drop_target.set(None);
}

fn compact_chat_title(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return "New chat".to_string();
    }

    let count = compact.chars().count();
    if count <= max_chars {
        compact
    } else {
        format!("{}...", compact.chars().take(max_chars).collect::<String>())
    }
}

fn derive_chat_thread_title(
    current_title: Option<&str>,
    messages: &[AcpUiMessage],
    connection_label: &str,
) -> String {
    if let Some(current_title) = current_title
        .map(str::trim)
        .filter(|title| !title.is_empty() && *title != "New chat")
    {
        return current_title.to_string();
    }

    if let Some(first_user_message) = messages
        .iter()
        .find(|message| matches!(message.kind, models::AcpMessageKind::User))
        .map(|message| {
            message
                .text
                .strip_prefix("Generate SQL:")
                .unwrap_or(&message.text)
                .trim()
        })
        .filter(|text| !text.is_empty())
    {
        return compact_chat_title(first_user_message, 56);
    }

    format!("New chat · {}", compact_chat_title(connection_label, 28))
}

fn upsert_chat_thread_summary(threads: &mut Vec<ChatThreadSummary>, summary: ChatThreadSummary) {
    if let Some(existing) = threads.iter_mut().find(|thread| thread.id == summary.id) {
        *existing = summary;
    } else {
        threads.push(summary);
    }

    threads.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn reset_panel_for_thread(state: &mut AcpPanelState, title: &str, messages: Vec<AcpUiMessage>) {
    let launch = state.launch.clone();
    let ollama = state.ollama.clone();
    *state = AcpPanelState::new(launch, ollama);
    replace_messages(state, messages);
    state.status = if title.trim().is_empty() {
        "Chat ready. Connect an agent to continue.".to_string()
    } else {
        format!("{title} is ready. Connect an agent to continue.")
    };
}

fn create_chat_thread(
    mut chat_threads: Signal<Vec<ChatThreadSummary>>,
    mut active_chat_thread_id: Signal<Option<i64>>,
    connection_name: String,
) {
    let _ = acp::disconnect_acp_agent();
    spawn(async move {
        match storage::create_chat_thread(connection_name, Some("New chat".to_string())).await {
            Ok(thread) => {
                chat_threads
                    .with_mut(|threads| upsert_chat_thread_summary(threads, thread.clone()));
                active_chat_thread_id.set(Some(thread.id));
            }
            Err(err) => {
                eprintln!("Failed to create chat thread: {err}");
            }
        }
    });
}

fn select_chat_thread(mut active_chat_thread_id: Signal<Option<i64>>, thread_id: i64) {
    if active_chat_thread_id() == Some(thread_id) {
        return;
    }

    let _ = acp::disconnect_acp_agent();
    active_chat_thread_id.set(Some(thread_id));
}

fn delete_chat_thread(
    mut chat_threads: Signal<Vec<ChatThreadSummary>>,
    mut active_chat_thread_id: Signal<Option<i64>>,
    connection_name: String,
    thread_id: i64,
) {
    let was_active = active_chat_thread_id() == Some(thread_id);
    let fallback_active = active_chat_thread_id();

    spawn(async move {
        if let Err(err) = storage::delete_chat_thread(thread_id).await {
            eprintln!("Failed to delete chat thread {thread_id}: {err}");
            return;
        }

        let mut next_thread_id = fallback_active.filter(|current| *current != thread_id);
        chat_threads.with_mut(|threads| {
            threads.retain(|thread| thread.id != thread_id);
            if was_active {
                next_thread_id = threads.first().map(|thread| thread.id);
            }
        });

        if was_active {
            let _ = acp::disconnect_acp_agent();
        }

        if let Some(next_thread_id) = next_thread_id {
            active_chat_thread_id.set(Some(next_thread_id));
            return;
        }

        match storage::create_chat_thread(connection_name, Some("New chat".to_string())).await {
            Ok(thread) => {
                chat_threads
                    .with_mut(|threads| upsert_chat_thread_summary(threads, thread.clone()));
                active_chat_thread_id.set(Some(thread.id));
            }
            Err(err) => {
                eprintln!("Failed to recreate chat thread after delete: {err}");
            }
        }
    });
}

#[component]
pub fn Workspace() -> Element {
    let active_session = { APP_STATE.read().active_session().cloned() };
    let connection_label = active_session
        .as_ref()
        .map(|session| session.name.clone())
        .unwrap_or_else(|| "No connection".to_string());
    let show_history = APP_SHOW_HISTORY();

    let mut tree_status = use_signal(|| "Loading explorer...".to_string());
    let mut tree_sections = use_signal(Vec::<ExplorerConnectionSection>::new);
    let mut tree_reload = use_signal(|| 0_u64);
    let mut next_tab_id = use_signal(|| 1_u64);
    let mut next_history_id = use_signal(|| 1_u64);
    let mut next_saved_query_id = use_signal(|| 1_u64);
    let mut active_tab_id = use_signal(|| 0_u64);
    let mut tabs = use_signal(Vec::<QueryTabState>::new);
    let mut history = use_signal(Vec::<QueryHistoryItem>::new);
    let mut saved_queries = use_signal(Vec::<SavedQuery>::new);
    let mut show_connections = use_signal(|| APP_UI_SETTINGS().show_connections);
    let mut show_explorer = use_signal(|| APP_UI_SETTINGS().show_explorer);
    let mut show_sql_editor = use_signal(|| APP_UI_SETTINGS().show_sql_editor);
    let mut ai_features_enabled = use_signal(|| APP_UI_SETTINGS().ai_features_enabled);
    let mut show_agent_panel = use_signal(|| APP_UI_SETTINGS().show_agent_panel);
    let allow_agent_db_read = use_signal(|| false);
    let allow_agent_read_sql_run = use_signal(|| false);
    let allow_agent_write_sql_run = use_signal(|| false);
    let allow_agent_tool_run = use_signal(|| false);
    let mut acp_panel_state = use_signal(default_acp_panel_state);
    let mut chat_threads = use_signal(Vec::<ChatThreadSummary>::new);
    let mut active_chat_thread_id = use_signal(|| None::<i64>);
    let mut chat_revision = use_signal(|| 0_u64);
    let mut handled_agent_sql_message_id = use_signal(|| 0_u64);
    let mut ai_disable_applied = use_signal(|| false);
    let mut chat_threads_loaded = use_signal(|| false);
    let mut chat_bootstrap_inflight = use_signal(|| false);
    let mut sidebar_width = use_signal(|| 320.0);
    let mut inspector_width = use_signal(|| 360.0);
    let mut sidebar_resize = use_signal(|| None::<ColumnResizeState>);
    let mut inspector_resize = use_signal(|| None::<ColumnResizeState>);
    let mut dragging_panel = use_signal(|| None::<WorkspaceToolPanel>);
    let mut drop_target = use_signal(|| None::<DockDropTarget>);
    let persisted_history =
        use_resource(
            move || async move { storage::load_query_history().await.unwrap_or_default() },
        );
    let persisted_saved_queries =
        use_resource(
            move || async move { storage::load_saved_queries().await.unwrap_or_default() },
        );
    let chat_bootstrap_connection_label = connection_label.clone();
    let chat_persist_connection_label = connection_label.clone();

    use_effect(move || {
        let reload_tick = tree_reload();
        let explorer_visible = show_explorer();
        let (sessions, active_session_id) = {
            let app_state = APP_STATE.read();
            (app_state.sessions.clone(), app_state.active_session_id)
        };

        spawn(async move {
            let _ = reload_tick;
            if sessions.is_empty() {
                tree_sections.set(Vec::new());
                tree_status.set("Select or create a connection".to_string());
                return;
            }

            if !explorer_visible {
                tree_sections.set(
                    sessions
                        .iter()
                        .map(|session| {
                            unloaded_explorer_section(session, active_session_id, "Explorer hidden")
                        })
                        .collect(),
                );
                tree_status.set("Explorer hidden".to_string());
                return;
            }

            let active_index = sessions
                .iter()
                .position(|session| Some(session.id) == active_session_id)
                .unwrap_or(0);
            let mut sections = sessions
                .iter()
                .map(|session| {
                    unloaded_explorer_section(
                        session,
                        active_session_id,
                        "Activate this connection to load explorer",
                    )
                })
                .collect::<Vec<_>>();

            tree_status.set("Loading explorer...".to_string());
            let active_section = load_explorer_section(
                sessions[active_index].clone(),
                active_session_id.or(Some(sessions[active_index].id)),
            )
            .await;
            let active_failed = active_section.status.starts_with("Error:");
            sections[active_index] = active_section;

            tree_sections.set(sections);
            if active_failed {
                tree_status.set("Explorer failed for the active connection".to_string());
            } else {
                tree_status.set("Explorer ready for the active connection".to_string());
            }
        });
    });

    use_effect(move || {
        let (session_ids, active_session_id) = {
            let app_state = APP_STATE.read();
            (
                app_state
                    .sessions
                    .iter()
                    .map(|session| session.id)
                    .collect::<HashSet<_>>(),
                app_state.active_session_id,
            )
        };

        tabs.with_mut(|all_tabs| all_tabs.retain(|tab| session_ids.contains(&tab.session_id)));

        if let Some(session_id) = active_session_id {
            let current_active_matches = tabs
                .read()
                .iter()
                .any(|tab| tab.id == active_tab_id() && tab.session_id == session_id);

            if current_active_matches {
                return;
            }

            if let Some(existing_tab_id) = tabs
                .read()
                .iter()
                .find(|tab| tab.session_id == session_id)
                .map(|tab| tab.id)
            {
                active_tab_id.set(existing_tab_id);
                return;
            }

            let tab_id = next_tab_id();
            next_tab_id += 1;
            tabs.with_mut(|all_tabs| {
                all_tabs.push(new_query_tab(
                    tab_id,
                    session_id,
                    format!("Query {tab_id}"),
                    "select 1 as id;".to_string(),
                ));
            });
            active_tab_id.set(tab_id);
        } else {
            active_tab_id.set(0);
        }
    });

    use_effect(move || {
        if let Some(items) = persisted_history() {
            let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
            history.set(items);
            next_history_id.set(next_id);
        }
    });

    use_effect(move || {
        if let Some(items) = persisted_saved_queries() {
            let next_id = items.iter().map(|item| item.id).max().unwrap_or(0) + 1;
            saved_queries.set(items);
            next_saved_query_id.set(next_id);
        }
    });

    use_effect(move || {
        if !ai_features_enabled() || !show_agent_panel() {
            return;
        }
        if chat_threads_loaded() {
            return;
        }
        if chat_bootstrap_inflight() {
            return;
        }

        chat_bootstrap_inflight.set(true);
        let default_connection = chat_bootstrap_connection_label.clone();
        spawn(async move {
            let items = storage::load_chat_threads().await.unwrap_or_default();
            if items.is_empty() {
                match storage::create_chat_thread(default_connection, Some("New chat".to_string()))
                    .await
                {
                    Ok(thread) => {
                        chat_threads.set(vec![thread.clone()]);
                        active_chat_thread_id.set(Some(thread.id));
                    }
                    Err(err) => {
                        eprintln!("Failed to create default chat thread: {err}");
                    }
                }
            } else {
                let next_active_thread_id = active_chat_thread_id()
                    .filter(|thread_id| items.iter().any(|thread| thread.id == *thread_id))
                    .or_else(|| items.first().map(|thread| thread.id));
                chat_threads.set(items);
                active_chat_thread_id.set(next_active_thread_id);
            }

            chat_threads_loaded.set(true);
            chat_bootstrap_inflight.set(false);
        });
    });

    use_effect(move || {
        if !ai_features_enabled() || !show_agent_panel() {
            return;
        }
        let Some(thread_id) = active_chat_thread_id() else {
            return;
        };

        spawn(async move {
            let thread_title = chat_threads
                .read()
                .iter()
                .find(|thread| thread.id == thread_id)
                .map(|thread| thread.title.clone())
                .unwrap_or_else(|| "New chat".to_string());
            let messages = match storage::load_chat_thread_messages(thread_id).await {
                Ok(messages) => messages,
                Err(err) => {
                    eprintln!("Failed to load chat thread {thread_id}: {err}");
                    Vec::new()
                }
            };
            let last_message_id = messages.iter().map(|message| message.id).max().unwrap_or(0);

            let _ = acp::disconnect_acp_agent();
            handled_agent_sql_message_id.set(last_message_id);
            acp_panel_state
                .with_mut(|state| reset_panel_for_thread(state, &thread_title, messages));
        });
    });

    use_effect(move || {
        let settings = APP_UI_SETTINGS();
        show_connections.set(settings.show_connections);
        show_explorer.set(settings.show_explorer);
        show_sql_editor.set(settings.show_sql_editor);
        ai_features_enabled.set(settings.ai_features_enabled);
        show_agent_panel.set(settings.ai_features_enabled && settings.show_agent_panel);
        *APP_SHOW_HISTORY.write() = settings.show_history;
    });

    use_effect(move || {
        if ai_features_enabled() {
            if ai_disable_applied() {
                ai_disable_applied.set(false);
            }
            return;
        }

        if ai_disable_applied() {
            return;
        }

        ai_disable_applied.set(true);
        let _ = acp::disconnect_acp_agent();
        acp_panel_state.with_mut(|state| {
            let launch = state.launch.clone();
            let ollama = state.ollama.clone();
            let existing_messages = state.messages.clone();
            *state = AcpPanelState::new(launch, ollama);
            replace_messages(state, existing_messages);
            state.status = "AI features are disabled.".to_string();
        });
    });

    use_effect(move || {
        let normalized = APP_UI_SETTINGS().tool_panel_layout.normalized();
        if APP_UI_SETTINGS().tool_panel_layout != normalized {
            APP_UI_SETTINGS.with_mut(|settings| {
                settings.tool_panel_layout = normalized;
            });
        }
    });

    use_effect(move || {
        let revision = chat_revision();
        if revision == 0 {
            return;
        }

        let connection_name = chat_persist_connection_label.clone();
        spawn(async move {
            let Some(thread_id) = active_chat_thread_id() else {
                return;
            };

            let messages = acp_panel_state().messages.clone();
            let current_title = chat_threads
                .read()
                .iter()
                .find(|thread| thread.id == thread_id)
                .map(|thread| thread.title.clone());
            let next_title =
                derive_chat_thread_title(current_title.as_deref(), &messages, &connection_name);

            match storage::save_chat_thread_snapshot(
                thread_id,
                next_title,
                connection_name.clone(),
                messages,
            )
            .await
            {
                Ok(summary) => {
                    chat_threads.with_mut(|threads| upsert_chat_thread_summary(threads, summary));
                }
                Err(err) => {
                    eprintln!("Failed to persist chat thread {thread_id}: {err}");
                }
            }
        });
    });

    use_effect(move || {
        spawn(async move {
            loop {
                let panel_active = ai_features_enabled() && show_agent_panel();
                let poll_delay = if panel_active && acp_panel_state().connected {
                    Duration::from_millis(120)
                } else {
                    Duration::from_millis(400)
                };

                if !panel_active {
                    let _ = acp::drain_acp_events();
                    tokio::time::sleep(poll_delay).await;
                    continue;
                }

                let events = acp::drain_acp_events();
                if !events.is_empty() {
                    acp_panel_state.with_mut(|state| apply_acp_events(state, events));
                    chat_revision += 1;

                    let pending_agent_sql = {
                        let panel_state = acp_panel_state();
                        let handled_message_id = handled_agent_sql_message_id();
                        panel_state
                            .messages
                            .iter()
                            .filter(|message| message.id > handled_message_id)
                            .find_map(|message| match message.kind {
                                models::AcpMessageKind::Agent => {
                                    extract_sql_candidate(&message.text).map(|sql| {
                                        (message.id, sql, panel_state.pending_sql_insert)
                                    })
                                }
                                _ => None,
                            })
                    };

                    if let Some((message_id, sql, pending_sql_insert)) = pending_agent_sql {
                        handled_agent_sql_message_id.set(message_id);

                        if query::is_read_only_sql(&sql) && allow_agent_read_sql_run() {
                            execute_agent_sql_request(
                                acp_panel_state,
                                tabs,
                                active_tab_id(),
                                show_sql_editor,
                                chat_revision,
                                sql,
                                AgentSqlExecutionMode::AutoReadOnly,
                            );
                        } else if pending_sql_insert {
                            show_sql_editor.set(true);
                            update_active_tab_sql(
                                tabs,
                                active_tab_id(),
                                sql,
                                "SQL generated by ACP agent".to_string(),
                            );
                            acp_panel_state.with_mut(|state| {
                                state.pending_sql_insert = false;
                                state.status =
                                    "Inserted generated SQL into the active editor.".to_string();
                            });
                        }
                    }
                }

                tokio::time::sleep(poll_delay).await;
            }
        });
    });

    let tool_panel_layout = APP_UI_SETTINGS().tool_panel_layout.normalized();
    let sidebar_panels = visible_tool_panels(
        &tool_panel_layout.sidebar,
        show_connections(),
        show_explorer(),
        show_history,
        show_agent_panel(),
        ai_features_enabled(),
    );
    let inspector_panels = visible_tool_panels(
        &tool_panel_layout.inspector,
        show_connections(),
        show_explorer(),
        show_history,
        show_agent_panel(),
        ai_features_enabled(),
    );
    let show_sidebar = !sidebar_panels.is_empty() || dragging_panel().is_some();
    let show_inspector = !inspector_panels.is_empty() || dragging_panel().is_some();
    let active_chat_thread = chat_threads
        .read()
        .iter()
        .find(|thread| Some(thread.id) == active_chat_thread_id())
        .cloned();
    let active_chat_thread_title = active_chat_thread
        .as_ref()
        .map(|thread| thread.title.clone())
        .unwrap_or_else(|| "New chat".to_string());
    let active_chat_thread_connection = active_chat_thread
        .as_ref()
        .map(|thread| thread.connection_name.clone())
        .unwrap_or_else(|| connection_label.clone());
    let inspector_connection_label = connection_label.clone();
    let create_thread_connection_label = connection_label.clone();
    let delete_thread_connection_label = connection_label.clone();

    let render_drop_slot = move |dock: WorkspaceToolDock, index: usize, empty: bool| -> Element {
        let target = DockDropTarget { dock, index };
        let mut class_name = "workspace__dock-dropzone".to_string();
        if empty {
            class_name.push_str(" workspace__dock-dropzone--empty");
        }
        if drop_target() == Some(target) {
            class_name.push_str(" workspace__dock-dropzone--active");
        }

        rsx! {
            div {
                class: class_name,
                onmousemove: move |event| {
                    if dragging_panel().is_none() {
                        return;
                    }

                    if event.held_buttons().is_empty() {
                        return;
                    }

                    if drop_target() != Some(target) {
                        drop_target.set(Some(target));
                    }
                },
                if empty {
                    span { class: "workspace__dock-dropzone-copy", "Drop panel here" }
                }
            }
        }
    };

    let render_tool_panel =
        move |panel: WorkspaceToolPanel, dock: WorkspaceToolDock, index: usize| -> Element {
            let mut class_name = "workspace__tool-panel".to_string();
            let panel_key = panel.label();
            let target = DockDropTarget { dock, index };
            let agent_thread_title = active_chat_thread_title.clone();
            let agent_thread_connection = active_chat_thread_connection.clone();
            let agent_sql_connection_label = inspector_connection_label.clone();
            let agent_create_connection_label = create_thread_connection_label.clone();
            let agent_delete_connection_label = delete_thread_connection_label.clone();
            class_name.push_str(match panel {
                WorkspaceToolPanel::Connections => " workspace__tool-panel--connections",
                WorkspaceToolPanel::Explorer => " workspace__tool-panel--explorer",
                WorkspaceToolPanel::SavedQueries => " workspace__tool-panel--saved",
                WorkspaceToolPanel::History => " workspace__tool-panel--history",
                WorkspaceToolPanel::Agent => " workspace__tool-panel--agent",
            });
            if dragging_panel() == Some(panel) {
                class_name.push_str(" workspace__tool-panel--dragging");
            }
            if drop_target() == Some(target) {
                class_name.push_str(" workspace__tool-panel--drop-target");
            }

            rsx! {
            div {
                key: "{panel_key}",
                class: class_name,
                onmousemove: move |event| {
                    if dragging_panel().is_none() {
                        return;
                    }

                    if event.held_buttons().is_empty() {
                        return;
                    }

                    if drop_target() != Some(target) {
                        drop_target.set(Some(target));
                    }
                },
                div {
                    class: "workspace__tool-panel-grip",
                    title: format!("Drag {} panel", panel.label()),
                    onmousedown: move |event| {
                        if event.trigger_button() != Some(MouseButton::Primary) {
                            return;
                        }

                        event.prevent_default();
                        event.stop_propagation();
                        dragging_panel.set(Some(panel));
                        drop_target.set(None);
                    },
                    span { class: "workspace__tool-panel-grip-dots" }
                }
                    {match panel {
                        WorkspaceToolPanel::Connections => rsx! {
                            div {
                                class: "workspace__panel",
                                SessionRail {
                                    tabs,
                                    active_tab_id,
                                }
                            }
                        },
                        WorkspaceToolPanel::Explorer => rsx! {
                            div {
                                class: "workspace__panel",
                                div {
                                    class: "workspace__panel-header",
                                    h2 { class: "workspace__section-title", "Explorer" }
                                    p { class: "workspace__hint", "{tree_status}" }
                                }
                                SidebarConnectionTree {
                                    sections: tree_sections(),
                                    tabs,
                                    active_tab_id,
                                    next_tab_id,
                                }
                            }
                        },
                        WorkspaceToolPanel::SavedQueries => rsx! {
                            SavedQueriesPanel {
                                saved_queries: saved_queries(),
                                saved_queries_signal: saved_queries,
                                next_saved_query_id,
                                tabs,
                                active_tab_id,
                                next_tab_id,
                            }
                        },
                        WorkspaceToolPanel::History => rsx! {
                            div {
                                class: "workspace__panel workspace__panel--history",
                                QueryHistoryPanel {
                                    history: history(),
                                    tabs,
                                    active_tab_id,
                                }
                            }
                        },
                        WorkspaceToolPanel::Agent => rsx! {
                            AcpAgentPanel {
                                panel_state: acp_panel_state,
                                tabs,
                                active_tab_id,
                                show_sql_editor,
                                chat_revision,
                                allow_agent_db_read,
                                allow_agent_read_sql_run,
                                allow_agent_write_sql_run,
                                allow_agent_tool_run,
                                chat_threads: chat_threads(),
                                active_thread_id: active_chat_thread_id(),
                                thread_title: agent_thread_title,
                                thread_connection_name: agent_thread_connection,
                                sql_connection_label: agent_sql_connection_label,
                                on_new_thread: move |_| {
                                    create_chat_thread(
                                        chat_threads,
                                        active_chat_thread_id,
                                        agent_create_connection_label.clone(),
                                    );
                                },
                                on_select_thread: move |thread_id| {
                                    select_chat_thread(active_chat_thread_id, thread_id);
                                },
                                on_delete_thread: move |thread_id| {
                                    delete_chat_thread(
                                        chat_threads,
                                        active_chat_thread_id,
                                        agent_delete_connection_label.clone(),
                                        thread_id,
                                    );
                                },
                            }
                        },
                    }}
                }
            }
        };

    rsx! {
        div {
            class: {
                let mut class_name = if show_sidebar {
                    "workspace".to_string()
                } else {
                    "workspace workspace--sidebar-hidden".to_string()
                };

                if sidebar_resize().is_some() {
                    class_name.push_str(" workspace--resizing");
                }
                if inspector_resize().is_some() {
                    class_name.push_str(" workspace--resizing");
                }
                if dragging_panel().is_some() {
                    class_name.push_str(" workspace--panel-dragging");
                }

                class_name
            },
            style: format!(
                "--workspace-sidebar-width: {:.0}px; --workspace-inspector-width: {:.0}px;",
                sidebar_width(),
                inspector_width(),
            ),
            onmousemove: move |event| {
                if let Some(resize) = sidebar_resize() {
                    if event.held_buttons().is_empty() {
                        sidebar_resize.set(None);
                        return;
                    }

                    let delta_x = event.client_coordinates().x - resize.start_x;
                    let next_width =
                        (resize.start_width + delta_x).clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
                    sidebar_width.set(next_width);
                    return;
                }

                let Some(resize) = inspector_resize() else {
                    return;
                };

                if event.held_buttons().is_empty() {
                    inspector_resize.set(None);
                    return;
                }

                let delta_x = event.client_coordinates().x - resize.start_x;
                let next_width =
                    (resize.start_width - delta_x).clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);
                inspector_width.set(next_width);
            },
            onmouseup: move |_| {
                sidebar_resize.set(None);
                inspector_resize.set(None);
                if let Some(target) = drop_target() {
                    apply_tool_panel_drop(
                        dragging_panel,
                        drop_target,
                        target,
                        show_connections(),
                        show_explorer(),
                        APP_SHOW_HISTORY(),
                        show_agent_panel(),
                        ai_features_enabled(),
                    );
                } else {
                    dragging_panel.set(None);
                    drop_target.set(None);
                }
            },
            onmouseleave: move |_| {
                sidebar_resize.set(None);
                inspector_resize.set(None);
                if dragging_panel().is_some() {
                    drop_target.set(None);
                }
            },
            if show_sidebar {
                aside {
                    class: "workspace__sidebar",
                    div {
                        class: "workspace__sidebar-body",
                        if sidebar_panels.is_empty() {
                            {render_drop_slot(WorkspaceToolDock::Sidebar, 0, true)}
                        } else {
                            for (index, panel) in sidebar_panels.iter().copied().enumerate() {
                                {render_drop_slot(WorkspaceToolDock::Sidebar, index, false)}
                                {render_tool_panel(panel, WorkspaceToolDock::Sidebar, index)}
                            }
                            {render_drop_slot(
                                WorkspaceToolDock::Sidebar,
                                sidebar_panels.len(),
                                false,
                            )}
                        }
                    }
                }
                div {
                    class: if sidebar_resize().is_some() {
                        "workspace__resize-handle workspace__resize-handle--active"
                    } else {
                        "workspace__resize-handle"
                    },
                    onmousedown: move |event| {
                        event.prevent_default();
                        sidebar_resize.set(Some(ColumnResizeState {
                            start_x: event.client_coordinates().x,
                            start_width: sidebar_width(),
                        }));
                    }
                }
            }
            section {
                class: "workspace__main",
                header {
                    class: "workspace__header",
                    div {
                        class: "workspace__toolbar",
                        IconButton {
                            icon: ActionIcon::Connections,
                            label: if show_connections() {
                                "Hide connections".to_string()
                            } else {
                                "Show connections".to_string()
                            },
                            active: show_connections(),
                            small: true,
                            onclick: move |_| {
                                let next = !show_connections();
                                show_connections.set(next);
                                APP_UI_SETTINGS.with_mut(|settings| {
                                    settings.show_connections = next;
                                });
                            },
                        }
                        IconButton {
                            icon: ActionIcon::Explorer,
                            label: if show_explorer() {
                                "Hide explorer".to_string()
                            } else {
                                "Show explorer".to_string()
                            },
                            active: show_explorer(),
                            small: true,
                            onclick: move |_| {
                                let next = !show_explorer();
                                show_explorer.set(next);
                                APP_UI_SETTINGS.with_mut(|settings| {
                                    settings.show_explorer = next;
                                });
                            },
                        }
                        IconButton {
                            icon: ActionIcon::History,
                            label: if show_history {
                                "Hide history".to_string()
                            } else {
                                "Show history".to_string()
                            },
                            active: show_history,
                            small: true,
                            onclick: move |_| {
                                let next = !APP_SHOW_HISTORY();
                                *APP_SHOW_HISTORY.write() = next;
                                APP_UI_SETTINGS.with_mut(|settings| {
                                    settings.show_history = next;
                                });
                            },
                        }
                        IconButton {
                            icon: ActionIcon::SqlEditor,
                            label: if show_sql_editor() {
                                "Hide SQL editor".to_string()
                            } else {
                                "Show SQL editor".to_string()
                            },
                            active: show_sql_editor(),
                            small: true,
                            onclick: move |_| {
                                let next = !show_sql_editor();
                                show_sql_editor.set(next);
                                APP_UI_SETTINGS.with_mut(|settings| {
                                    settings.show_sql_editor = next;
                                });
                            },
                        }
                        if ai_features_enabled() {
                            IconButton {
                                icon: ActionIcon::Agent,
                                label: if show_agent_panel() {
                                    "Hide agent panel".to_string()
                                } else {
                                    "Show agent panel".to_string()
                                },
                                active: show_agent_panel(),
                                small: true,
                                onclick: move |_| {
                                    let next = !show_agent_panel();
                                    show_agent_panel.set(next);
                                    APP_UI_SETTINGS.with_mut(|settings| {
                                        settings.show_agent_panel = next;
                                    });
                                },
                            }
                        }
                        IconButton {
                            icon: ActionIcon::Refresh,
                            label: "Refresh explorer".to_string(),
                            small: true,
                            onclick: move |_| tree_reload += 1,
                        }
                        IconButton {
                            icon: ActionIcon::NewConnection,
                            label: "New connection".to_string(),
                            primary: true,
                            small: true,
                            onclick: move |_| open_connection_screen(),
                        }
                    }
                }
                div {
                    class: if show_inspector {
                        "workspace__content workspace__content--with-inspector"
                    } else {
                        "workspace__content"
                    },
                    div {
                        class: "workspace__canvas",
                        TabsManager {
                            tabs,
                            active_tab_id,
                            next_tab_id,
                            history,
                            next_history_id,
                            show_sql_editor,
                            explorer_sections: tree_sections,
                        }
                    }
                    if show_inspector {
                        div {
                            class: if inspector_resize().is_some() {
                                "workspace__resize-handle workspace__resize-handle--inspector workspace__resize-handle--active"
                            } else {
                                "workspace__resize-handle workspace__resize-handle--inspector"
                            },
                            onmousedown: move |event| {
                                event.prevent_default();
                                inspector_resize.set(Some(ColumnResizeState {
                                    start_x: event.client_coordinates().x,
                                    start_width: inspector_width(),
                                }));
                            }
                        }
                        aside {
                            class: "workspace__inspector",
                            if inspector_panels.is_empty() {
                                {render_drop_slot(WorkspaceToolDock::Inspector, 0, true)}
                            } else {
                                for (index, panel) in inspector_panels.iter().copied().enumerate() {
                                    {render_drop_slot(WorkspaceToolDock::Inspector, index, false)}
                                    {render_tool_panel(panel, WorkspaceToolDock::Inspector, index)}
                                }
                                {render_drop_slot(
                                    WorkspaceToolDock::Inspector,
                                    inspector_panels.len(),
                                    false,
                                )}
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) use self::components::SqlFormatSettingsFields;
