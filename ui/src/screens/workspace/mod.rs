mod actions;
mod components;

use crate::app_state::{APP_SHOW_HISTORY, APP_STATE, APP_UI_SETTINGS, open_connection_screen};
use dioxus::{html::input_data::MouseButton, prelude::*};
use futures_util::future::join_all;
use models::{
    QueryHistoryItem, QueryTabState, SavedQuery, WorkspaceToolDock, WorkspaceToolLayout,
    WorkspaceToolPanel,
};
use std::collections::HashSet;
use std::time::Duration;

use self::{
    actions::{new_query_tab, update_active_tab_sql},
    components::{
        AcpAgentPanel, ActionIcon, ExplorerConnectionSection, IconButton, QueryHistoryPanel,
        SavedQueriesPanel, SessionRail, SidebarConnectionTree, TabsManager, apply_acp_events,
        default_acp_panel_state, extract_sql_candidate,
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

fn is_tool_panel_visible(
    panel: WorkspaceToolPanel,
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
) -> bool {
    match panel {
        WorkspaceToolPanel::Connections => show_connections,
        WorkspaceToolPanel::Explorer => show_explorer,
        WorkspaceToolPanel::SavedQueries => true,
        WorkspaceToolPanel::History => show_history,
        WorkspaceToolPanel::Agent => show_agent_panel,
    }
}

fn visible_tool_panels(
    panels: &[WorkspaceToolPanel],
    show_connections: bool,
    show_explorer: bool,
    show_history: bool,
    show_agent_panel: bool,
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
) -> usize {
    if !panels.iter().any(|panel| {
        is_tool_panel_visible(
            *panel,
            show_connections,
            show_explorer,
            show_history,
            show_agent_panel,
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
            );
        });
    }

    dragging_panel.set(None);
    drop_target.set(None);
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
    let mut show_agent_panel = use_signal(|| APP_UI_SETTINGS().show_agent_panel);
    let mut acp_panel_state = use_signal(default_acp_panel_state);
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

    use_effect(move || {
        let reload_tick = tree_reload();
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

            tree_status.set("Loading explorer...".to_string());
            let sections = join_all(
                sessions
                    .into_iter()
                    .map(|session| load_explorer_section(session, active_session_id)),
            )
            .await;
            let failed_count = sections
                .iter()
                .filter(|section| section.status.starts_with("Error:"))
                .count();

            tree_sections.set(sections);
            if failed_count == 0 {
                tree_status.set("Explorer ready".to_string());
            } else {
                tree_status.set(format!(
                    "Explorer ready, {failed_count} connection(s) failed"
                ));
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
        let settings = APP_UI_SETTINGS();
        show_connections.set(settings.show_connections);
        show_explorer.set(settings.show_explorer);
        show_sql_editor.set(settings.show_sql_editor);
        show_agent_panel.set(settings.show_agent_panel);
        *APP_SHOW_HISTORY.write() = settings.show_history;
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
        spawn(async move {
            loop {
                let events = acp::drain_acp_events();
                if !events.is_empty() {
                    acp_panel_state.with_mut(|state| apply_acp_events(state, events));

                    let sql_to_insert = {
                        let panel_state = acp_panel_state();
                        if panel_state.pending_sql_insert {
                            panel_state.messages.iter().rev().find_map(|message| {
                                match message.kind {
                                    models::AcpMessageKind::Agent => {
                                        extract_sql_candidate(&message.text)
                                    }
                                    _ => None,
                                }
                            })
                        } else {
                            None
                        }
                    };

                    if let Some(sql) = sql_to_insert {
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

                tokio::time::sleep(Duration::from_millis(120)).await;
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
    );
    let inspector_panels = visible_tool_panels(
        &tool_panel_layout.inspector,
        show_connections(),
        show_explorer(),
        show_history,
        show_agent_panel(),
    );
    let show_sidebar = !sidebar_panels.is_empty() || dragging_panel().is_some();
    let show_inspector = !inspector_panels.is_empty() || dragging_panel().is_some();

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
                                sql_connection_label: connection_label.clone(),
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
