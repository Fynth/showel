#[path = "sql_editor/completion.rs"]
mod completion;
#[path = "sql_editor/highlight.rs"]
mod highlight;
#[path = "sql_editor/selection.rs"]
mod selection;

use crate::screens::workspace::actions::replace_active_tab_sql;
use crate::screens::workspace::components::ExplorerConnectionSection;
use dioxus::prelude::*;
use models::QueryTabState;
use std::time::Duration;

use self::{
    completion::{extract_current_token, CompletionItem, SchemaMetadata, complete_sql},
    highlight::SqlHighlightContent,
    selection::{
        EditorSelection, apply_suggestion, set_editor_selection_script, sync_editor_selection,
    },
};

const SQL_EDITOR_TEXTAREA_ID: &str = "workspace-sql-editor";

#[derive(Clone, Debug, Default)]
struct CompletionPopup {
    items: Vec<CompletionItem>,
    selected: usize,
    visible: bool,
    cursor_position: usize,
    partial_token: String,
}

fn build_schema_metadata(
    sections: &[ExplorerConnectionSection],
    active_session_id: u64,
) -> SchemaMetadata {
    let Some(section) = sections.iter().find(|s| s.session_id == active_session_id) else {
        return SchemaMetadata::default();
    };

    let mut tables = Vec::new();
    let mut schemas = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for node in &section.nodes {
        match node.kind {
            models::ExplorerNodeKind::Schema => {
                if seen.insert(node.name.clone()) {
                    schemas.push(node.name.clone());
                }
                for child in &node.children {
                    if seen.insert(child.qualified_name.clone()) {
                        tables.push(completion::TableMeta {
                            schema: child.schema.clone().or_else(|| Some(node.name.clone())),
                            name: child.name.clone(),
                            qualified_name: child.qualified_name.clone(),
                            columns: Vec::new(),
                        });
                    }
                }
            }
            models::ExplorerNodeKind::Table | models::ExplorerNodeKind::View => {
                if seen.insert(node.qualified_name.clone()) {
                    tables.push(completion::TableMeta {
                        schema: node.schema.clone(),
                        name: node.name.clone(),
                        qualified_name: node.qualified_name.clone(),
                        columns: Vec::new(),
                    });
                }
            }
        }
    }

    SchemaMetadata { tables, schemas }
}

#[component]
pub fn SqlEditor(
    sql: String,
    active_tab: QueryTabState,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    explorer_sections: Signal<Vec<ExplorerConnectionSection>>,
) -> Element {
    let active_tab_id_value = active_tab.id;
    let synced_tab_sql = active_tab.sql.clone();
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let mut draft_sql = use_signal(|| sql.clone());
    let mut editor_selection = use_signal(|| EditorSelection::collapsed(sql.len()));
    let mut sync_revision = use_signal(|| 0_u64);
    let mut completion_popup = use_signal(CompletionPopup::default);
    let current_sql = draft_sql();

    let schema_meta =
        use_memo(move || build_schema_metadata(&explorer_sections(), active_tab.session_id));

    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    let inline_suffix = use_memo(move || {
        let popup = completion_popup();
        if popup.visible && !popup.items.is_empty() && !popup.partial_token.is_empty() {
            if let Some(first) = popup.items.first() {
                if first.label.to_lowercase().starts_with(&popup.partial_token.to_lowercase()) {
                    let remainder = &first.label[popup.partial_token.len()..];
                    if !remainder.is_empty() {
                        return Some(remainder.to_string());
                    }
                }
            }
        }
        None
    });

    // Sync tab SQL when the active tab changes.
    use_effect(move || {
        let _ = active_tab_id();
        let next_sql = sql.clone();
        let next_len = next_sql.len();
        draft_sql.set(next_sql);
        editor_selection.set(EditorSelection::collapsed(next_len));
    });

    // Debounced persist of draft SQL back to the active tab.
    use_effect(move || {
        let next_sql = draft_sql();
        let revision = sync_revision();
        if next_sql == synced_tab_sql {
            return;
        }

        spawn(async move {
            tokio::time::sleep(Duration::from_millis(90)).await;
            if sync_revision() != revision {
                return;
            }

            let already_synced = tabs
                .read()
                .iter()
                .find(|tab| tab.id == active_tab_id_value)
                .is_some_and(|tab| tab.sql == next_sql);
            if already_synced {
                return;
            }

            replace_active_tab_sql(tabs, active_tab_id_value, next_sql, "Ready".to_string());
        });
    });

    rsx! {
        div {
            class: "sql-editor",

            // ── Syntax-highlighted overlay ────────────────────────────
            div {
                class: "sql-editor__viewport",
                pre {
                    class: "sql-editor__highlight",
                    style: "{editor_offset}",
                    aria_hidden: "true",
                    SqlHighlightContent {
                        sql: current_sql.clone(),
                        inline_cursor_position: if inline_suffix().is_some() { Some(completion_popup().cursor_position) } else { None },
                        inline_suffix: inline_suffix(),
                    }
                }
            }

            // ── Textarea ─────────────────────────────────────────────
            textarea {
                id: SQL_EDITOR_TEXTAREA_ID,
                class: "sql-editor__input",
                value: "{current_sql}",
                rows: "16",
                cols: "80",
                spellcheck: "false",

                oninput: move |event| {
                    let new_sql = event.value();
                    let cursor = editor_selection().start;
                    let meta = schema_meta();
                    let items = complete_sql(&new_sql, cursor.min(new_sql.len()), &meta);
                    let (partial_token, _) = extract_current_token(&new_sql, cursor.min(new_sql.len()));
                    if items.is_empty() {
                        completion_popup.set(CompletionPopup::default());
                    } else {
                        completion_popup.set(CompletionPopup {
                            items,
                            selected: 0,
                            visible: true,
                            cursor_position: cursor.min(new_sql.len()),
                            partial_token,
                        });
                    }
                    draft_sql.set(new_sql);
                    sync_revision += 1;
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onkeydown: move |event| {
                    let popup = completion_popup();
                    if popup.visible && !popup.items.is_empty() {
                        match event.key() {
                            Key::Tab | Key::Enter => {
                                event.prevent_default();
                                let selected_idx = popup.selected;
                                if let Some(item) = popup.items.get(selected_idx).cloned() {
                                    let sql_text = draft_sql();
                                    let sel = editor_selection();
                                    let (new_sql, new_cursor) =
                                        apply_suggestion(&sql_text, sel, &item.insert_text);
                                    draft_sql.set(new_sql.clone());
                                    sync_revision += 1;
                                    replace_active_tab_sql(
                                        tabs,
                                        active_tab_id_value,
                                        new_sql,
                                        "Ready".to_string(),
                                    );
                                    completion_popup.set(CompletionPopup::default());
                                    spawn(async move {
                                        let _ = document::eval(
                                            &set_editor_selection_script(
                                                SQL_EDITOR_TEXTAREA_ID,
                                                new_cursor,
                                            ),
                                        )
                                        .join::<bool>()
                                        .await;
                                    });
                                }
                            }
                            Key::Escape => {
                                event.prevent_default();
                                completion_popup.set(CompletionPopup::default());
                            }
                            Key::ArrowDown => {
                                event.prevent_default();
                                let next =
                                    (popup.selected + 1).min(popup.items.len().saturating_sub(1));
                                completion_popup.set(CompletionPopup {
                                    selected: next,
                                    ..popup
                                });
                            }
                            Key::ArrowUp => {
                                event.prevent_default();
                                let prev = popup.selected.saturating_sub(1);
                                completion_popup.set(CompletionPopup {
                                    selected: prev,
                                    ..popup
                                });
                            }
                            _ => {}
                        }
                    }
                },

                onkeyup: move |_| {
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onmouseup: move |_| {
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onclick: move |_| {
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onfocus: move |_| {
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onblur: move |_| {
                    completion_popup.set(CompletionPopup::default());
                },

                onscroll: move |event| {
                    scroll_top.set(event.data().scroll_top());
                    scroll_left.set(event.data().scroll_left());
                },
            }

            // ── Autocomplete popup ──────────────────────────────────
            if completion_popup().visible && !completion_popup().items.is_empty() {
                div {
                    class: "sql-editor__autocomplete",

                    for (index, item) in completion_popup().items.iter().enumerate() {
                        button {
                            key: "{index}",
                            onmousedown: move |event| {
                                event.prevent_default();
                            },
                            class: if index == completion_popup().selected {
                                "sql-editor__autocomplete-item sql-editor__autocomplete-item--active"
                            } else {
                                "sql-editor__autocomplete-item"
                            },

                            onclick: {
                                let insert = item.insert_text.clone();
                                move |_| {
                                    let sql_text = draft_sql();
                                    let sel = editor_selection();
                                    let (new_sql, new_cursor) =
                                        apply_suggestion(&sql_text, sel, &insert);
                                    draft_sql.set(new_sql.clone());
                                    sync_revision += 1;
                                    replace_active_tab_sql(
                                        tabs,
                                        active_tab_id_value,
                                        new_sql,
                                        "Ready".to_string(),
                                    );
                                    completion_popup.set(CompletionPopup::default());
                                    let cursor = new_cursor;
                                    spawn(async move {
                                        let _ = document::eval(
                                            &set_editor_selection_script(
                                                SQL_EDITOR_TEXTAREA_ID,
                                                cursor,
                                            ),
                                        )
                                        .join::<bool>()
                                        .await;
                                    });
                                }
                            },

                            div {
                                class: "sql-editor__autocomplete-copy",

                                span {
                                    class: "sql-editor__autocomplete-label",
                                    "{item.label}"
                                }

                                if let Some(ref detail) = item.detail {
                                    span {
                                        class: "sql-editor__autocomplete-detail",
                                        "{detail}"
                                    }
                                }
                            }

                            span {
                                class: "sql-editor__autocomplete-kind",
                                "{item.kind.label()}"
                            }
                        }
                    }
                }
            }
        }
    }
}
