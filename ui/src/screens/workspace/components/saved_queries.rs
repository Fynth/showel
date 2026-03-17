use crate::{
    app_state::APP_STATE,
    screens::workspace::actions::{append_to_tab_sql, ensure_tab_for_session, set_active_tab_sql},
};
use dioxus::prelude::*;
use models::{QueryTabState, SavedQuery, SavedQueryKind};
use std::collections::BTreeMap;

#[component]
pub fn SavedQueriesPanel(
    saved_queries: Vec<SavedQuery>,
    saved_queries_signal: Signal<Vec<SavedQuery>>,
    next_saved_query_id: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
) -> Element {
    let mut save_title = use_signal(String::new);
    let mut save_folder = use_signal(|| "General".to_string());
    let mut panel_status = use_signal(String::new);

    let active_tab = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id())
        .cloned();
    let active_sql = active_tab
        .as_ref()
        .map(|tab| tab.sql.trim().to_string())
        .unwrap_or_default();
    let can_save = !active_sql.is_empty();

    let sessions_by_name = APP_STATE
        .read()
        .sessions
        .iter()
        .map(|session| (session.name.clone(), session.id))
        .collect::<std::collections::HashMap<_, _>>();

    let mut grouped = BTreeMap::<String, Vec<SavedQuery>>::new();
    for item in saved_queries {
        grouped
            .entry(item.folder_name().to_string())
            .or_default()
            .push(item);
    }
    for items in grouped.values_mut() {
        items.sort_by(|left, right| {
            left.title
                .cmp(&right.title)
                .then_with(|| left.id.cmp(&right.id))
        });
    }

    rsx! {
        section {
            class: "workspace__panel saved-queries",
            h2 { class: "workspace__section-title", "Saved Queries" }
            p {
                class: "workspace__hint",
                if panel_status().trim().is_empty() {
                    "Reusable queries and snippets grouped by folder."
                } else {
                    "{panel_status}"
                }
            }

            div { class: "saved-queries__form",
                div { class: "field",
                    label { class: "field__label", "Title" }
                    input {
                        class: "input",
                        value: "{save_title}",
                        placeholder: active_tab
                            .as_ref()
                            .map(|tab| tab.title.clone())
                            .unwrap_or_else(|| "Saved Query".to_string()),
                        oninput: move |event| save_title.set(event.value()),
                    }
                }
                div { class: "field",
                    label { class: "field__label", "Folder" }
                    input {
                        class: "input",
                        value: "{save_folder}",
                        placeholder: "General",
                        oninput: move |event| save_folder.set(event.value()),
                    }
                }
                div { class: "saved-queries__form-actions",
                    button {
                        class: "button button--ghost button--small",
                        disabled: !can_save,
                        onclick: {
                            let active_tab = active_tab.clone();
                            move |_| {
                                save_current_sql(
                                    SavedQueryKind::Snippet,
                                    active_tab.clone(),
                                    save_title,
                                    save_folder,
                                    next_saved_query_id,
                                    saved_queries_signal,
                                    panel_status,
                                );
                            }
                        },
                        "Save Snippet"
                    }
                    button {
                        class: "button button--primary button--small",
                        disabled: !can_save,
                        onclick: {
                            let active_tab = active_tab.clone();
                            move |_| {
                                save_current_sql(
                                    SavedQueryKind::Query,
                                    active_tab.clone(),
                                    save_title,
                                    save_folder,
                                    next_saved_query_id,
                                    saved_queries_signal,
                                    panel_status,
                                );
                            }
                        },
                        "Save Query"
                    }
                }
            }

            if grouped.is_empty() {
                p { class: "empty-state", "No saved queries or snippets yet." }
            } else {
                for (folder_name, items) in grouped {
                    div {
                        class: "saved-queries__folder",
                        div {
                            class: "saved-queries__folder-header",
                            h3 { class: "saved-queries__folder-title", "{folder_name}" }
                            span { class: "saved-queries__folder-count", "{items.len()}" }
                        }
                        div { class: "saved-queries__folder-body",
                            for item in items {
                                {
                                    let source_session_id = item
                                        .connection_name
                                        .as_ref()
                                        .and_then(|name| sessions_by_name.get(name))
                                        .copied();
                                    let load_label = if item.kind == SavedQueryKind::Snippet {
                                        "Insert in tab"
                                    } else {
                                        "Load in tab"
                                    };

                                    rsx! {
                                        article { class: "saved-queries__item",
                                            div { class: "saved-queries__item-top",
                                                p { class: "saved-queries__title", "{item.title}" }
                                                span { class: "saved-queries__kind", "{item.kind_label()}" }
                                            }
                                            if let Some(connection_name) = item.connection_name.clone() {
                                                p {
                                                    class: "saved-queries__connection",
                                                    title: "{connection_name}",
                                                    "{connection_name}"
                                                }
                                            }
                                            pre {
                                                class: "saved-queries__sql",
                                                title: "{item.sql}",
                                                "{item.sql}"
                                            }
                                            div { class: "saved-queries__actions",
                                                button {
                                                    class: "button button--ghost button--small",
                                                    onclick: {
                                                        let item = item.clone();
                                                        move |_| {
                                                            load_saved_query_into_workspace(
                                                                item.clone(),
                                                                source_session_id,
                                                                tabs,
                                                                active_tab_id,
                                                                next_tab_id,
                                                            );
                                                            panel_status.set(format!(
                                                                "{} loaded into workspace.",
                                                                item.title
                                                            ));
                                                        }
                                                    },
                                                    "{load_label}"
                                                }
                                                button {
                                                    class: "button button--ghost button--small",
                                                    onclick: {
                                                        let item_id = item.id;
                                                        let item_title = item.title.clone();
                                                        move |_| {
                                                            saved_queries_signal.with_mut(|items| {
                                                                items.retain(|existing| existing.id != item_id);
                                                            });
                                                            panel_status.set(format!("Deleted {item_title}."));
                                                            spawn(async move {
                                                                let _ = storage::delete_saved_query(item_id).await;
                                                            });
                                                        }
                                                    },
                                                    "Delete"
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
}

fn save_current_sql(
    kind: SavedQueryKind,
    active_tab: Option<QueryTabState>,
    mut save_title: Signal<String>,
    save_folder: Signal<String>,
    mut next_saved_query_id: Signal<u64>,
    mut saved_queries_signal: Signal<Vec<SavedQuery>>,
    mut panel_status: Signal<String>,
) {
    let Some(active_tab) = active_tab else {
        panel_status.set("No active SQL tab available.".to_string());
        return;
    };
    if active_tab.sql.trim().is_empty() {
        panel_status.set("Current SQL tab is empty.".to_string());
        return;
    }

    let title = if save_title().trim().is_empty() {
        active_tab.title.clone()
    } else {
        save_title().trim().to_string()
    };
    let folder = if save_folder().trim().is_empty() {
        "General".to_string()
    } else {
        save_folder().trim().to_string()
    };
    let connection_name = APP_STATE.read().session_name(active_tab.session_id);
    let item = SavedQuery {
        id: next_saved_query_id(),
        title: title.clone(),
        folder,
        sql: active_tab.sql,
        kind,
        connection_name,
    };

    next_saved_query_id += 1;
    saved_queries_signal.with_mut(|items| {
        items.push(item.clone());
        items.sort_by(|left, right| {
            left.folder_name()
                .cmp(right.folder_name())
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.id.cmp(&right.id))
        });
    });
    save_title.set(String::new());
    panel_status.set(format!("Saved {}.", title));

    spawn(async move {
        let _ = storage::save_saved_query(item).await;
    });
}

fn load_saved_query_into_workspace(
    item: SavedQuery,
    source_session_id: Option<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
) {
    let target_tab_id = if let Some(session_id) = source_session_id {
        ensure_tab_for_session(tabs, active_tab_id, next_tab_id, session_id)
    } else {
        active_tab_id()
    };

    if target_tab_id == 0 {
        return;
    }

    match item.kind {
        SavedQueryKind::Query => set_active_tab_sql(
            tabs,
            target_tab_id,
            item.sql,
            "Loaded saved query".to_string(),
        ),
        SavedQueryKind::Snippet => append_to_tab_sql(
            tabs,
            target_tab_id,
            item.sql,
            "Inserted saved snippet".to_string(),
        ),
    }
}
