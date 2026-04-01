mod actions;
mod chat;
mod components;
mod context;
mod helpers;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::app_state::{
    APP_SHOW_HISTORY, APP_STATE, APP_UI_SETTINGS, open_connection_screen, toast_error,
};
use dioxus::{html::input_data::MouseButton, prelude::*};
use models::{
    AcpPanelState, ChatThreadSummary, QueryHistoryItem, QueryTabState, SavedQuery,
    WorkspaceToolDock, WorkspaceToolPanel,
};

use self::{
    actions::{new_query_tab, update_active_tab_sql},
    chat::{create_chat_thread, delete_chat_thread, select_chat_thread},
    components::{
        AcpAgentPanel, ActionIcon, AgentSqlExecutionMode, ExplorerConnectionSection, IconButton,
        QueryHistoryPanel, SavedQueriesPanel, SessionRail, SidebarConnectionTree, TabsManager,
        apply_acp_events, default_acp_panel_state, ensure_opencode_connected,
        execute_agent_sql_request, extract_sql_candidate, preferred_sql_target_tab_id,
        replace_messages,
    },
    helpers::{
        DockDropTarget, INSPECTOR_MAX_WIDTH, INSPECTOR_MIN_WIDTH, SIDEBAR_MAX_WIDTH,
        SIDEBAR_MIN_WIDTH, WORKSPACE_ROOT_ID, apply_tool_panel_drop, derive_chat_thread_title,
        launch_uses_opencode, load_explorer_section, reset_panel_for_thread, tool_panel_class,
        unloaded_explorer_section, upsert_chat_thread_summary, visible_tool_panels,
        workspace_resize_script,
    },
};

#[component]
fn WorkspaceDropSlot(
    dock: WorkspaceToolDock,
    index: usize,
    empty: bool,
    dragging_panel: Signal<Option<WorkspaceToolPanel>>,
    mut drop_target: Signal<Option<DockDropTarget>>,
) -> Element {
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
}

#[component]
fn ExplorerToolPanel(
    tree_status: Signal<String>,
    tree_sections: Signal<Vec<ExplorerConnectionSection>>,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
) -> Element {
    rsx! {
        div {
            class: "workspace__panel",
            div {
                class: "workspace__panel-header",
                h2 { class: "workspace__section-title", "Explorer" }
                p { class: "workspace__hint", "{tree_status()}" }
            }
            SidebarConnectionTree {
                sections: tree_sections(),
                tree_reload,
                tabs,
                active_tab_id,
                next_tab_id,
            }
        }
    }
}

#[component]
fn AgentToolPanel(
    mut acp_panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
    chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    chat_threads: Signal<Vec<ChatThreadSummary>>,
    mut active_chat_thread_id: Signal<Option<i64>>,
    connection_label: String,
) -> Element {
    let active_chat_thread = use_memo(move || {
        chat_threads
            .read()
            .iter()
            .find(|thread| Some(thread.id) == active_chat_thread_id())
            .cloned()
    });
    let thread_title = active_chat_thread
        .read()
        .as_ref()
        .map(|thread| thread.title.clone())
        .unwrap_or_else(|| "New chat".to_string());
    let thread_connection_name = active_chat_thread
        .read()
        .as_ref()
        .map(|thread| thread.connection_name.clone())
        .unwrap_or_else(|| connection_label.clone());
    let new_thread_connection = connection_label.clone();
    let delete_thread_connection = connection_label.clone();
    let sql_connection_label = connection_label.clone();

    rsx! {
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
            thread_title,
            thread_connection_name,
            sql_connection_label,
            on_new_thread: move |_| {
                create_chat_thread(
                    chat_threads,
                    active_chat_thread_id,
                    new_thread_connection.clone(),
                );
            },
            on_select_thread: move |thread_id| {
                select_chat_thread(active_chat_thread_id, thread_id);
            },
            on_delete_thread: move |thread_id| {
                delete_chat_thread(
                    chat_threads,
                    active_chat_thread_id,
                    delete_thread_connection.clone(),
                    thread_id,
                );
            },
        }
    }
}

#[component]
fn WorkspacePanelContent(
    panel: WorkspaceToolPanel,
    tree_status: Signal<String>,
    tree_sections: Signal<Vec<ExplorerConnectionSection>>,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    history: Signal<Vec<QueryHistoryItem>>,
    saved_queries: Signal<Vec<SavedQuery>>,
    next_saved_query_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
    acp_panel_state: Signal<AcpPanelState>,
    chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    chat_threads: Signal<Vec<ChatThreadSummary>>,
    active_chat_thread_id: Signal<Option<i64>>,
    connection_label: String,
) -> Element {
    match panel {
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
            ExplorerToolPanel {
                tree_status,
                tree_sections,
                tree_reload,
                tabs,
                active_tab_id,
                next_tab_id,
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
            AgentToolPanel {
                acp_panel_state,
                tabs,
                active_tab_id,
                show_sql_editor,
                chat_revision,
                allow_agent_db_read,
                allow_agent_read_sql_run,
                allow_agent_write_sql_run,
                allow_agent_tool_run,
                chat_threads,
                active_chat_thread_id,
                connection_label,
            }
        },
    }
}

#[component]
fn WorkspaceDockPanel(
    panel: WorkspaceToolPanel,
    dock: WorkspaceToolDock,
    index: usize,
    dragging_panel: Signal<Option<WorkspaceToolPanel>>,
    mut drop_target: Signal<Option<DockDropTarget>>,
    tree_status: Signal<String>,
    tree_sections: Signal<Vec<ExplorerConnectionSection>>,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    history: Signal<Vec<QueryHistoryItem>>,
    saved_queries: Signal<Vec<SavedQuery>>,
    next_saved_query_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
    acp_panel_state: Signal<AcpPanelState>,
    chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    chat_threads: Signal<Vec<ChatThreadSummary>>,
    active_chat_thread_id: Signal<Option<i64>>,
    connection_label: String,
) -> Element {
    let target = DockDropTarget { dock, index };
    let mut class_name = "workspace__tool-panel".to_string();
    class_name.push_str(tool_panel_class(panel));
    if dragging_panel() == Some(panel) {
        class_name.push_str(" workspace__tool-panel--dragging");
    }
    if drop_target() == Some(target) {
        class_name.push_str(" workspace__tool-panel--drop-target");
    }

    rsx! {
        div {
            key: "{panel.label()}",
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
            WorkspacePanelContent {
                panel,
                tree_status,
                tree_sections,
                tree_reload,
                tabs,
                active_tab_id,
                next_tab_id,
                history,
                saved_queries,
                next_saved_query_id,
                show_sql_editor,
                acp_panel_state,
                chat_revision,
                allow_agent_db_read,
                allow_agent_read_sql_run,
                allow_agent_write_sql_run,
                allow_agent_tool_run,
                chat_threads,
                active_chat_thread_id,
                connection_label,
            }
        }
    }
}

#[component]
fn WorkspaceDock(
    dock: WorkspaceToolDock,
    panels: Vec<WorkspaceToolPanel>,
    dragging_panel: Signal<Option<WorkspaceToolPanel>>,
    drop_target: Signal<Option<DockDropTarget>>,
    tree_status: Signal<String>,
    tree_sections: Signal<Vec<ExplorerConnectionSection>>,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    history: Signal<Vec<QueryHistoryItem>>,
    saved_queries: Signal<Vec<SavedQuery>>,
    next_saved_query_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
    acp_panel_state: Signal<AcpPanelState>,
    chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    chat_threads: Signal<Vec<ChatThreadSummary>>,
    active_chat_thread_id: Signal<Option<i64>>,
    connection_label: String,
) -> Element {
    rsx! {
        if panels.is_empty() {
            WorkspaceDropSlot {
                dock,
                index: 0,
                empty: true,
                dragging_panel,
                drop_target,
            }
        } else {
            for (index, panel) in panels.iter().copied().enumerate() {
                WorkspaceDropSlot {
                    dock,
                    index,
                    empty: false,
                    dragging_panel,
                    drop_target,
                }
                WorkspaceDockPanel {
                    panel,
                    dock,
                    index,
                    dragging_panel,
                    drop_target,
                    tree_status,
                    tree_sections,
                    tree_reload,
                    tabs,
                    active_tab_id,
                    next_tab_id,
                    history,
                    saved_queries,
                    next_saved_query_id,
                    show_sql_editor,
                    acp_panel_state,
                    chat_revision,
                    allow_agent_db_read,
                    allow_agent_read_sql_run,
                    allow_agent_write_sql_run,
                    allow_agent_tool_run,
                    chat_threads,
                    active_chat_thread_id,
                    connection_label: connection_label.clone(),
                }
            }
            WorkspaceDropSlot {
                dock,
                index: panels.len(),
                empty: false,
                dragging_panel,
                drop_target,
            }
        }
    }
}

#[component]
fn WorkspaceBody(
    show_sidebar: bool,
    show_inspector: bool,
    sidebar_panels: Vec<WorkspaceToolPanel>,
    inspector_panels: Vec<WorkspaceToolPanel>,
    sidebar_width: Signal<f64>,
    mut sidebar_resize_active: Signal<bool>,
    inspector_width: Signal<f64>,
    mut inspector_resize_active: Signal<bool>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    history: Signal<Vec<QueryHistoryItem>>,
    next_history_id: Signal<u64>,
    saved_queries: Signal<Vec<SavedQuery>>,
    next_saved_query_id: Signal<u64>,
    tree_status: Signal<String>,
    tree_sections: Signal<Vec<ExplorerConnectionSection>>,
    show_connections: Signal<bool>,
    show_explorer: Signal<bool>,
    show_sql_editor: Signal<bool>,
    ai_features_enabled: Signal<bool>,
    show_agent_panel: Signal<bool>,
    show_history: bool,
    tree_reload: Signal<u64>,
    dragging_panel: Signal<Option<WorkspaceToolPanel>>,
    drop_target: Signal<Option<DockDropTarget>>,
    acp_panel_state: Signal<AcpPanelState>,
    chat_revision: Signal<u64>,
    allow_agent_db_read: Signal<bool>,
    allow_agent_read_sql_run: Signal<bool>,
    allow_agent_write_sql_run: Signal<bool>,
    allow_agent_tool_run: Signal<bool>,
    chat_threads: Signal<Vec<ChatThreadSummary>>,
    active_chat_thread_id: Signal<Option<i64>>,
    connection_label: String,
) -> Element {
    rsx! {
        if show_sidebar {
            aside {
                class: "workspace__sidebar",
                div {
                    class: "workspace__sidebar-body",
                    WorkspaceDock {
                        dock: WorkspaceToolDock::Sidebar,
                        panels: sidebar_panels.clone(),
                        dragging_panel,
                        drop_target,
                        tree_status,
                        tree_sections,
                        tree_reload,
                        tabs,
                        active_tab_id,
                        next_tab_id,
                        history,
                        saved_queries,
                        next_saved_query_id,
                        show_sql_editor,
                        acp_panel_state,
                        chat_revision,
                        allow_agent_db_read,
                        allow_agent_read_sql_run,
                        allow_agent_write_sql_run,
                        allow_agent_tool_run,
                        chat_threads,
                        active_chat_thread_id,
                        connection_label: connection_label.clone(),
                    }
                }
            }
            div {
                class: if sidebar_resize_active() {
                    "workspace__resize-handle workspace__resize-handle--active"
                } else {
                    "workspace__resize-handle"
                },
                onmousedown: move |event| {
                    if event.trigger_button() != Some(MouseButton::Primary) {
                        return;
                    }

                    event.prevent_default();
                    event.stop_propagation();

                    let start_x = event.client_coordinates().x;
                    let start_width = sidebar_width();
                    sidebar_resize_active.set(true);
                    spawn(async move {
                        let result = document::eval(&workspace_resize_script(
                            "--workspace-sidebar-width",
                            start_x,
                            start_width,
                            SIDEBAR_MIN_WIDTH,
                            SIDEBAR_MAX_WIDTH,
                            false,
                        ))
                        .join::<f64>()
                        .await;

                        match result {
                            Ok(width) => sidebar_width.set(width),
                            Err(err) => {
                                eprintln!("Failed to resize workspace sidebar: {err:?}");
                            }
                        }

                        sidebar_resize_active.set(false);
                    });
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
                        acp_panel_state,
                        chat_revision,
                        allow_agent_db_read,
                        ai_features_enabled,
                    }
                }
                if show_inspector {
                    div {
                        class: if inspector_resize_active() {
                            "workspace__resize-handle workspace__resize-handle--inspector workspace__resize-handle--active"
                        } else {
                            "workspace__resize-handle workspace__resize-handle--inspector"
                        },
                        onmousedown: move |event| {
                            if event.trigger_button() != Some(MouseButton::Primary) {
                                return;
                            }

                            event.prevent_default();
                            event.stop_propagation();

                            let start_x = event.client_coordinates().x;
                            let start_width = inspector_width();
                            inspector_resize_active.set(true);
                            spawn(async move {
                                let result = document::eval(&workspace_resize_script(
                                    "--workspace-inspector-width",
                                    start_x,
                                    start_width,
                                    INSPECTOR_MIN_WIDTH,
                                    INSPECTOR_MAX_WIDTH,
                                    true,
                                ))
                                .join::<f64>()
                                .await;

                                match result {
                                    Ok(width) => inspector_width.set(width),
                                    Err(err) => {
                                        eprintln!(
                                            "Failed to resize workspace inspector: {err:?}"
                                        );
                                    }
                                }

                                inspector_resize_active.set(false);
                            });
                        }
                    }
                    aside {
                        class: "workspace__inspector",
                        WorkspaceDock {
                            dock: WorkspaceToolDock::Inspector,
                            panels: inspector_panels,
                            dragging_panel,
                            drop_target,
                            tree_status,
                            tree_sections,
                            tree_reload,
                            tabs,
                            active_tab_id,
                            next_tab_id,
                            history,
                            saved_queries,
                            next_saved_query_id,
                            show_sql_editor,
                            acp_panel_state,
                            chat_revision,
                            allow_agent_db_read,
                            allow_agent_read_sql_run,
                            allow_agent_write_sql_run,
                            allow_agent_tool_run,
                            chat_threads,
                            active_chat_thread_id,
                            connection_label: connection_label.clone(),
                        }
                    }
                }
            }
        }
    }
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
    let tree_reload = use_signal(|| 0_u64);
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
    let allow_agent_db_read = use_signal(|| true);
    let allow_agent_read_sql_run = use_signal(|| true);
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
    let mut opencode_autostart_attempted = use_signal(|| false);
    let mut warmed_schema_session_id = use_signal(|| 0_u64);
    let sidebar_width = use_signal(|| 320.0);
    let sidebar_resize_active = use_signal(|| false);
    let inspector_width = use_signal(|| 360.0);
    let inspector_resize_active = use_signal(|| false);
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

    context::provide_workspace_tab_context(tabs, active_tab_id, next_tab_id);
    context::provide_workspace_query_context(
        history,
        next_history_id,
        saved_queries,
        next_saved_query_id,
    );
    context::provide_workspace_acp_context(context::WorkspaceAcpContext {
        acp_panel_state,
        chat_revision,
        allow_agent_db_read,
        allow_agent_read_sql_run,
        allow_agent_write_sql_run,
        allow_agent_tool_run,
        chat_threads,
        active_chat_thread_id,
        connection_label: connection_label.clone(),
    });

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
        if !ai_features_enabled() {
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
                        toast_error(format!("Failed to create default chat thread: {err}"));
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
        if !ai_features_enabled() {
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
                    toast_error(format!("Failed to load chat thread: {err}"));
                    Vec::new()
                }
            };
            let last_message_id = messages.iter().map(|message| message.id).max().unwrap_or(0);

            let _ = acp::disconnect_acp_agent();
            handled_agent_sql_message_id.set(last_message_id);
            opencode_autostart_attempted.set(false);
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
                opencode_autostart_attempted.set(false);
            }
            return;
        }

        if ai_disable_applied() {
            return;
        }

        ai_disable_applied.set(true);
        opencode_autostart_attempted.set(false);
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
        if !ai_features_enabled() {
            return;
        }
        if !chat_threads_loaded() || active_chat_thread_id().is_none() {
            return;
        }
        if opencode_autostart_attempted() {
            return;
        }

        let state = acp_panel_state();
        if state.connected || state.busy || !launch_uses_opencode(&state) {
            return;
        }

        opencode_autostart_attempted.set(true);
        spawn(async move {
            let _ = ensure_opencode_connected(acp_panel_state, chat_revision).await;
        });
    });

    use_effect(move || {
        if !ai_features_enabled() || !allow_agent_db_read() {
            return;
        }

        let active_session = { APP_STATE.read().active_session().cloned() };
        let Some(session) = active_session else {
            return;
        };

        if warmed_schema_session_id() == session.id {
            return;
        }

        warmed_schema_session_id.set(session.id);
        spawn(async move {
            let _ = acp::warm_acp_database_schema_context(
                session.connection.clone(),
                session.name.clone(),
            )
            .await;
        });
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
                    toast_error(format!("Failed to persist chat thread: {err}"));
                }
            }
        });
    });

    use_effect(move || {
        static STOP_FLAG: AtomicBool = AtomicBool::new(false);
        STOP_FLAG.store(false, Ordering::Relaxed);
        let _ = spawn(async move {
            loop {
                if STOP_FLAG.load(Ordering::Relaxed) {
                    break;
                }
                let ai_active = ai_features_enabled();
                let panel_visible = show_agent_panel();
                let poll_delay = if ai_active && acp_panel_state().connected {
                    if panel_visible {
                        Duration::from_millis(120)
                    } else {
                        Duration::from_millis(180)
                    }
                } else {
                    Duration::from_millis(400)
                };

                if !ai_active {
                    let _ = acp::drain_acp_events();
                    tokio::time::sleep(poll_delay).await;
                    continue;
                }

                if STOP_FLAG.load(Ordering::Relaxed) {
                    break;
                }

                let events = acp::drain_acp_events();
                if !events.is_empty() {
                    acp_panel_state.with_mut(|state| apply_acp_events(state, events));
                    chat_revision += 1;

                    let pending_hidden_agent_sql = {
                        let panel_state = acp_panel_state();
                        extract_sql_candidate(&panel_state.hidden_agent_response)
                            .map(|sql| (sql, panel_state.pending_sql_insert))
                    };
                    let pending_agent_sql = if pending_hidden_agent_sql.is_none() {
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
                    } else {
                        None
                    };

                    if let Some((sql, pending_sql_insert)) = pending_hidden_agent_sql {
                        acp_panel_state.with_mut(|state| state.hidden_agent_response.clear());

                        if query::is_read_only_sql(&sql) && allow_agent_read_sql_run() {
                            execute_agent_sql_request(
                                acp_panel_state,
                                tabs,
                                active_tab_id,
                                show_sql_editor,
                                chat_revision,
                                sql,
                                AgentSqlExecutionMode::AutoReadOnly,
                                false,
                            );
                        } else if pending_sql_insert
                            && let Some(target_tab_id) =
                                preferred_sql_target_tab_id(tabs, active_tab_id())
                        {
                            show_sql_editor.set(true);
                            active_tab_id.set(target_tab_id);
                            update_active_tab_sql(
                                tabs,
                                target_tab_id,
                                sql,
                                "SQL generated by ACP agent".to_string(),
                            );
                            acp_panel_state.with_mut(|state| {
                                state.pending_sql_insert = false;
                                state.status =
                                    "Inserted generated SQL into the active editor.".to_string();
                            });
                        }
                    } else if let Some((message_id, sql, pending_sql_insert)) = pending_agent_sql {
                        handled_agent_sql_message_id.set(message_id);

                        if query::is_read_only_sql(&sql) && allow_agent_read_sql_run() {
                            execute_agent_sql_request(
                                acp_panel_state,
                                tabs,
                                active_tab_id,
                                show_sql_editor,
                                chat_revision,
                                sql,
                                AgentSqlExecutionMode::AutoReadOnly,
                                true,
                            );
                        } else if pending_sql_insert
                            && let Some(target_tab_id) =
                                preferred_sql_target_tab_id(tabs, active_tab_id())
                        {
                            show_sql_editor.set(true);
                            active_tab_id.set(target_tab_id);
                            update_active_tab_sql(
                                tabs,
                                target_tab_id,
                                sql,
                                "SQL generated by ACP agent".to_string(),
                            );
                            acp_panel_state.with_mut(|state| {
                                state.pending_sql_insert = false;
                                state.status =
                                    "Inserted generated SQL into the active editor.".to_string();
                            });
                        }
                    } else if !acp_panel_state().suppress_transcript
                        && !acp_panel_state().hidden_agent_response.is_empty()
                    {
                        acp_panel_state.with_mut(|state| state.hidden_agent_response.clear());
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

    rsx! {
        div {
            id: WORKSPACE_ROOT_ID,
            class: {
                let mut class_name = if show_sidebar {
                    "workspace".to_string()
                } else {
                    "workspace workspace--sidebar-hidden".to_string()
                };

                if sidebar_resize_active() || inspector_resize_active() {
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
            onmouseup: move |_| {
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
                if dragging_panel().is_some() {
                    drop_target.set(None);
                }
            },
            WorkspaceBody {
                show_sidebar,
                show_inspector,
                sidebar_panels,
                inspector_panels,
                sidebar_width,
                sidebar_resize_active,
                inspector_width,
                inspector_resize_active,
                tabs,
                active_tab_id,
                next_tab_id,
                history,
                next_history_id,
                saved_queries,
                next_saved_query_id,
                tree_status,
                tree_sections,
                show_connections,
                show_explorer,
                show_sql_editor,
                ai_features_enabled,
                show_agent_panel,
                show_history,
                tree_reload,
                dragging_panel,
                drop_target,
                acp_panel_state,
                chat_revision,
                allow_agent_db_read,
                allow_agent_read_sql_run,
                allow_agent_write_sql_run,
                allow_agent_tool_run,
                chat_threads,
                active_chat_thread_id,
                connection_label: connection_label.clone(),
            }
        }
    }
}

pub(crate) use self::components::SqlFormatSettingsFields;
