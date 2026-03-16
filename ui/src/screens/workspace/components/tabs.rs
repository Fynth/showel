use crate::{
    app_state::{APP_STATE, open_connection_screen},
    screens::workspace::{
        actions::{
            load_tab_page, new_query_tab, run_query_for_tab, tab_connection_or_error,
            update_active_tab_sql,
        },
        components::{ResultTable, SqlEditor},
    },
};
use dioxus::prelude::*;
use models::{QueryHistoryItem, QueryOutput, QueryTabState};

#[component]
pub fn TabsManager(
    mut tabs: Signal<Vec<QueryTabState>>,
    mut active_tab_id: Signal<u64>,
    mut next_tab_id: Signal<u64>,
    history: Signal<Vec<QueryHistoryItem>>,
    next_history_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
) -> Element {
    let active_tab = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id())
        .cloned();

    let session_labels = {
        let app_state = APP_STATE.read();
        app_state
            .sessions
            .iter()
            .map(|session| (session.id, session.name.clone()))
            .collect::<std::collections::HashMap<_, _>>()
    };

    rsx! {
        div {
            class: if show_sql_editor() {
                "editor-shell"
            } else {
                "editor-shell editor-shell--editor-hidden"
            },
            if show_sql_editor() {
                div {
                    class: "tabbar",
                    for tab in tabs() {
                        div {
                            class: if tab.id == active_tab_id() {
                                "tabbar__tab tabbar__tab--active"
                            } else {
                                "tabbar__tab"
                            },
                            onclick: {
                                let tab_id = tab.id;
                                let session_id = tab.session_id;
                                move |_| {
                                    active_tab_id.set(tab_id);
                                    crate::app_state::activate_session(session_id);
                                }
                            },
                            div {
                                class: "tabbar__copy",
                                span { class: "tabbar__label", "{tab.title}" }
                                if let Some(session_name) = session_labels.get(&tab.session_id) {
                                    span { class: "tabbar__context", "{session_name}" }
                                }
                            }
                            button {
                                class: "tabbar__close",
                                onclick: {
                                    let tab_id = tab.id;
                                    move |event| {
                                        event.stop_propagation();
                                        if tabs.read().len() == 1 {
                                            return;
                                        }

                                        tabs.with_mut(|all_tabs| all_tabs.retain(|tab| tab.id != tab_id));
                                        if active_tab_id() == tab_id {
                                            if let Some(first_tab) = tabs.read().first() {
                                                active_tab_id.set(first_tab.id);
                                                crate::app_state::activate_session(first_tab.session_id);
                                            }
                                        }
                                    }
                                },
                                "x"
                            }
                        }
                    }
                    button {
                        class: "tabbar__add",
                        onclick: move |_| {
                            let Some(session_id) = APP_STATE.read().active_session_id else {
                                open_connection_screen();
                                return;
                            };

                            let new_id = next_tab_id();
                            next_tab_id += 1;
                            tabs.with_mut(|all_tabs| {
                                all_tabs.push(new_query_tab(
                                    new_id,
                                    session_id,
                                    format!("Query {new_id}"),
                                    String::new(),
                                ));
                            });
                            active_tab_id.set(new_id);
                        },
                        "+ Tab"
                    }
                }
            }

            if let Some(active_tab) = active_tab {
                if show_sql_editor() {
                    div {
                        class: "editor",
                        SqlEditor {
                            sql: active_tab.sql.clone(),
                            tabs,
                            active_tab_id,
                        }
                    }
                }
                div {
                    class: "editor__actions",
                    button {
                        class: "button button--primary",
                        onclick: move |_| {
                            let current_id = active_tab_id();
                            let current_tab = tabs
                                .read()
                                .iter()
                                .find(|tab| tab.id == current_id)
                                .cloned();

                            let Some(current_tab) = current_tab else {
                                return;
                            };

                            let sql = current_tab.sql.trim().to_string();
                            let tab_title = current_tab.title.clone();
                            let page_size = current_tab.page_size;
                            let connection_name = session_labels
                                .get(&current_tab.session_id)
                                .cloned()
                                .unwrap_or_else(|| "Detached session".to_string());

                            if sql.is_empty() {
                                tabs.with_mut(|all_tabs| {
                                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                                        tab.status = "Query is empty".to_string();
                                    }
                                });
                                return;
                            }

                            let Some(connection) =
                                tab_connection_or_error(tabs, current_id, current_tab.session_id)
                            else {
                                return;
                            };

                            run_query_for_tab(
                                tabs,
                                current_id,
                                connection,
                                sql,
                                0,
                                page_size,
                                Some((history, next_history_id, tab_title, connection_name)),
                            );
                        },
                        "Run SQL"
                    }
                    button {
                        class: "button button--ghost",
                        onclick: {
                            let current_id = active_tab_id();
                            move |_| {
                                update_active_tab_sql(
                                    tabs,
                                    current_id,
                                    String::new(),
                                    "Ready".to_string(),
                                );
                            }
                        },
                        "Clear"
                    }
                }
                if let Some(QueryOutput::Table(page)) = active_tab.result.clone() {
                    div {
                        class: "editor__pagination",
                        p { class: "editor__pagination-meta",
                            "Rows {page.offset + 1}-{page.offset + page.rows.len() as u64} · page size {page.page_size}"
                        }
                        button {
                            class: "button button--ghost",
                            disabled: !page.has_previous || active_tab.last_run_sql.is_none(),
                            onclick: {
                                let current_tab = active_tab.clone();
                                move |_| {
                                    if current_tab.last_run_sql.is_none()
                                        && current_tab.preview_source.is_none()
                                    {
                                        return;
                                    };
                                    load_tab_page(
                                        tabs,
                                        current_tab.clone(),
                                        page.offset.saturating_sub(current_tab.page_size as u64),
                                    );
                                }
                            },
                            "Previous"
                        }
                        button {
                            class: "button button--ghost",
                            disabled: !page.has_next || active_tab.last_run_sql.is_none(),
                            onclick: {
                                let current_tab = active_tab.clone();
                                move |_| {
                                    if current_tab.last_run_sql.is_none()
                                        && current_tab.preview_source.is_none()
                                    {
                                        return;
                                    };
                                    load_tab_page(
                                        tabs,
                                        current_tab.clone(),
                                        page.offset + current_tab.page_size as u64,
                                    );
                                }
                            },
                            "Next"
                        }
                    }
                }
                div {
                    class: "workspace__results",
                    p { class: "workspace__status", "Status: {active_tab.status}" }
                    ResultTable {
                        result: active_tab.result.clone(),
                        tabs,
                        active_tab_id,
                    }
                }
            } else {
                div {
                    class: "workspace__empty",
                    p { class: "empty-state", "No active tab for the selected connection." }
                }
            }
        }
    }
}
