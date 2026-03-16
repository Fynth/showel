use crate::app_state::{APP_STATE, activate_session, open_connection_screen, remove_session};
use dioxus::prelude::*;
use models::QueryTabState;

#[component]
pub fn SessionRail(
    mut tabs: Signal<Vec<QueryTabState>>,
    mut active_tab_id: Signal<u64>,
) -> Element {
    let (sessions, active_session_id) = {
        let app_state = APP_STATE.read();
        (app_state.sessions.clone(), app_state.active_session_id)
    };
    let session_cards = sessions
        .into_iter()
        .map(|session| {
            let kind_label = match session.kind {
                models::DatabaseKind::Sqlite => "SQLite",
                models::DatabaseKind::Postgres => "PostgreSQL",
                models::DatabaseKind::ClickHouse => "ClickHouse",
            };
            (session, kind_label)
        })
        .collect::<Vec<_>>();

    rsx! {
        section {
            class: "session-list",
            div {
                class: "session-list__header",
                h2 { class: "workspace__section-title", "Connections" }
                button {
                    class: "button button--ghost button--small",
                    onclick: move |_| open_connection_screen(),
                    "Add"
                }
            }

            if session_cards.is_empty() {
                p { class: "empty-state", "No active connections." }
            } else {
                for (session, kind_label) in session_cards {
                    div {
                        class: if Some(session.id) == active_session_id {
                            "session-list__item session-list__item--active"
                        } else {
                            "session-list__item"
                        },
                        button {
                            class: "session-list__main",
                            onclick: {
                                let session_id = session.id;
                                move |_| activate_session(session_id)
                            },
                            span { class: "session-list__kind", "{kind_label}" }
                            strong { class: "session-list__name", "{session.name}" }
                        }
                        button {
                            class: "button button--ghost button--small",
                            onclick: {
                                let session_id = session.id;
                                move |_| {
                                    tabs.with_mut(|all_tabs| all_tabs.retain(|tab| tab.session_id != session_id));
                                    if let Some(first_tab) = tabs.read().first() {
                                        active_tab_id.set(first_tab.id);
                                        activate_session(first_tab.session_id);
                                    } else {
                                        active_tab_id.set(0);
                                    }
                                    remove_session(session_id);
                                }
                            },
                            "Close"
                        }
                    }
                }
            }
        }
    }
}
