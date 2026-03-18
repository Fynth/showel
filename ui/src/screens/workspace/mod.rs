mod actions;
mod components;

use crate::app_state::{APP_SHOW_HISTORY, APP_STATE, open_connection_screen};
use dioxus::prelude::*;
use futures_util::future::join_all;
use models::{QueryHistoryItem, QueryTabState, SavedQuery};
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

#[derive(Clone, Copy, PartialEq)]
struct SidebarResizeState {
    start_x: f64,
    start_width: f64,
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
    let mut show_connections = use_signal(|| false);
    let mut show_explorer = use_signal(|| true);
    let mut show_sql_editor = use_signal(|| true);
    let mut show_agent_panel = use_signal(|| false);
    let mut acp_panel_state = use_signal(default_acp_panel_state);
    let mut sidebar_width = use_signal(|| 320.0);
    let mut sidebar_resize = use_signal(|| None::<SidebarResizeState>);
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

    rsx! {
        div {
            class: {
                let mut class_name = if show_connections() || show_explorer() || show_history {
                    "workspace".to_string()
                } else {
                    "workspace workspace--sidebar-hidden".to_string()
                };

                if sidebar_resize().is_some() {
                    class_name.push_str(" workspace--resizing");
                }

                class_name
            },
            style: if show_connections() || show_explorer() || show_history {
                format!("--workspace-sidebar-width: {:.0}px;", sidebar_width())
            } else {
                String::new()
            },
            onmousemove: move |event| {
                let Some(resize) = sidebar_resize() else {
                    return;
                };

                if event.held_buttons().is_empty() {
                    sidebar_resize.set(None);
                    return;
                }

                let delta_x = event.client_coordinates().x - resize.start_x;
                let next_width =
                    (resize.start_width + delta_x).clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
                sidebar_width.set(next_width);
            },
            onmouseup: move |_| sidebar_resize.set(None),
            onmouseleave: move |_| sidebar_resize.set(None),
            if show_connections() || show_explorer() || show_history {
                aside {
                    class: "workspace__sidebar",
                    div {
                        class: "workspace__sidebar-body",
                        if show_connections() {
                            SessionRail {
                                tabs,
                                active_tab_id,
                            }
                        }
                        if show_explorer() {
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
                        }
                        SavedQueriesPanel {
                            saved_queries: saved_queries(),
                            saved_queries_signal: saved_queries,
                            next_saved_query_id,
                            tabs,
                            active_tab_id,
                            next_tab_id,
                        }
                        if show_history {
                            QueryHistoryPanel {
                                history: history(),
                                tabs,
                                active_tab_id,
                            }
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
                        sidebar_resize.set(Some(SidebarResizeState {
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
                            onclick: move |_| show_connections.toggle(),
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
                            onclick: move |_| show_explorer.toggle(),
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
                            onclick: move |_| APP_SHOW_HISTORY.with_mut(|visible| *visible = !*visible),
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
                            onclick: move |_| show_sql_editor.toggle(),
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
                            onclick: move |_| show_agent_panel.toggle(),
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
                    class: if show_agent_panel() {
                        "workspace__content workspace__content--with-agent"
                    } else {
                        "workspace__content"
                    },
                    TabsManager {
                        tabs,
                        active_tab_id,
                        next_tab_id,
                        history,
                        next_history_id,
                        show_sql_editor,
                        explorer_sections: tree_sections,
                    }
                    if show_agent_panel() {
                        AcpAgentPanel {
                            panel_state: acp_panel_state,
                            tabs,
                            active_tab_id,
                            show_sql_editor,
                            sql_connection_label: connection_label.clone(),
                        }
                    }
                }
            }
        }
    }
}
