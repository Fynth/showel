use crate::app_state::{APP_STATE, activate_session};
use dioxus::prelude::*;
use models::{QueryHistoryItem, QueryTabState};

use crate::screens::workspace::actions::set_active_tab_sql;

#[component]
pub fn QueryHistoryPanel(
    history: Vec<QueryHistoryItem>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let session_ids_by_name = APP_STATE
        .read()
        .sessions
        .iter()
        .map(|session| (session.name.clone(), session.id))
        .collect::<std::collections::HashMap<_, _>>();
    let history_entries = history
        .into_iter()
        .map(|item| {
            let source_session_id = session_ids_by_name.get(&item.connection_name).copied();
            let (connection_kind, connection_target) = item
                .connection_name
                .split_once(" · ")
                .map(|(kind, target)| (kind.to_string(), target.to_string()))
                .unwrap_or_else(|| (String::new(), item.connection_name.clone()));
            let outcome_class = if item.outcome.starts_with("Error") {
                "history__outcome history__outcome--error"
            } else {
                "history__outcome history__outcome--success"
            };
            (
                item,
                source_session_id,
                connection_kind,
                connection_target,
                outcome_class,
            )
        })
        .collect::<Vec<_>>();

    rsx! {
        section {
            class: "history",
            h2 { class: "workspace__section-title", "History" }
            if history_entries.is_empty() {
                p { class: "empty-state", "No executed queries yet." }
            } else {
                for (item, source_session_id, connection_kind, connection_target, outcome_class) in history_entries {
                    div {
                        class: "history__item",
                        div {
                            class: "history__meta",
                            div {
                                class: "history__topline",
                                p { class: "history__title", "{item.tab_title}" }
                                p { class: outcome_class, "{item.outcome}" }
                            }
                            if !connection_target.is_empty() {
                                div {
                                    class: "history__connection",
                                    if !connection_kind.is_empty() {
                                        span { class: "history__connection-kind", "{connection_kind}" }
                                    }
                                    span {
                                        class: "history__connection-target",
                                        title: "{item.connection_name}",
                                        "{connection_target}"
                                    }
                                }
                            }
                        }
                        pre {
                            class: "history__sql",
                            title: "{item.sql}",
                            "{item.sql}"
                        }
                        div {
                            class: "history__actions",
                            if let Some(session_id) = source_session_id {
                                button {
                                    class: "button button--ghost button--small",
                                    onclick: move |_| activate_session(session_id),
                                    "Activate Source"
                                }
                            },
                            button {
                                class: "button button--ghost button--small",
                                onclick: {
                                    let sql = item.sql.clone();
                                    move |_| {
                                        set_active_tab_sql(
                                            tabs,
                                            active_tab_id(),
                                            sql.clone(),
                                            "Loaded query from history".to_string(),
                                        );
                                    }
                                },
                                "Reuse in active tab"
                            }
                        }
                    }
                }
            }
        }
    }
}
