#[path = "sql_editor/autocomplete.rs"]
mod autocomplete;
#[path = "sql_editor/highlight.rs"]
mod highlight;
#[path = "sql_editor/selection.rs"]
mod selection;

use crate::{app_state::session_connection, screens::workspace::actions::update_active_tab_sql};
use dioxus::prelude::*;
use models::{ExplorerNode, QueryTabState};
use std::collections::{HashMap, HashSet};

use self::{
    autocomplete::{
        build_inline_completion, build_suggestions, completion_context, extract_relation_bindings,
        flatten_catalog, relations_to_prefetch, resolved_replacement, should_open_autocomplete,
    },
    highlight::SqlHighlightContent,
    selection::{
        EditorSelection, apply_suggestion, set_editor_selection_script, sync_editor_selection,
    },
};

const SQL_EDITOR_TEXTAREA_ID: &str = "workspace-sql-editor";

#[component]
pub fn SqlEditor(
    sql: String,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let mut autocomplete_open = use_signal(|| false);
    let mut force_autocomplete = use_signal(|| false);
    let mut selected_suggestion = use_signal(|| 0usize);
    let mut suggestion_key = use_signal(String::new);
    let mut editor_selection = use_signal(|| EditorSelection::collapsed(sql.len()));
    let mut pending_cursor_position = use_signal(|| None::<usize>);
    let column_cache = use_signal(HashMap::<String, Vec<String>>::new);

    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    let active_tab = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id())
        .cloned();
    let tree = use_resource(move || async move {
        let current_id = active_tab_id();
        let current_tab = tabs.read().iter().find(|tab| tab.id == current_id).cloned();
        let Some(session_id) = current_tab.as_ref().map(|tab| tab.session_id) else {
            return Vec::<ExplorerNode>::new();
        };
        let Some(connection) = session_connection(session_id) else {
            return Vec::<ExplorerNode>::new();
        };

        services::load_connection_tree(connection)
            .await
            .unwrap_or_default()
    });

    let catalog = tree()
        .map(|nodes| flatten_catalog(&nodes))
        .unwrap_or_default();
    let selection = editor_selection().clamped(&sql);
    let completion_context = completion_context(&sql, selection);
    let relation_bindings = extract_relation_bindings(&sql, &catalog);
    let relations_to_prefetch = relations_to_prefetch(
        active_tab.as_ref(),
        &catalog,
        &relation_bindings,
        &completion_context,
    );
    let cache_snapshot = column_cache();
    let suggestions = build_suggestions(
        &sql,
        selection,
        active_tab.as_ref(),
        &catalog,
        &relation_bindings,
        &cache_snapshot,
        force_autocomplete(),
    );
    let inline_completion = build_inline_completion(
        &sql,
        selection,
        &completion_context,
        if force_autocomplete() {
            suggestions
                .get(selected_suggestion())
                .or_else(|| suggestions.first())
        } else {
            suggestions.first()
        },
    );
    let popup_visible = force_autocomplete() && autocomplete_open() && !suggestions.is_empty();
    let sql_len = sql.len();
    let next_suggestion_key = format!(
        "{}:{}:{}:{}:{}:{}",
        active_tab_id(),
        selection.start,
        selection.end,
        completion_context.raw_token,
        suggestions.len(),
        force_autocomplete()
    );
    let effect_completion_context = completion_context.clone();

    use_effect(move || {
        let _ = active_tab_id();
        editor_selection.set(EditorSelection::collapsed(sql_len));
        autocomplete_open.set(false);
        force_autocomplete.set(false);
    });

    use_effect(move || {
        if suggestion_key() != next_suggestion_key {
            suggestion_key.set(next_suggestion_key.clone());
            selected_suggestion.set(0);
            autocomplete_open
                .set(force_autocomplete() || should_open_autocomplete(&effect_completion_context));
        }
    });

    use_effect(move || {
        let Some(position) = pending_cursor_position() else {
            return;
        };

        let mut pending_cursor_position = pending_cursor_position;
        let mut editor_selection = editor_selection;
        let position = position.min(sql_len);
        spawn(async move {
            let _ = document::eval(&set_editor_selection_script(
                SQL_EDITOR_TEXTAREA_ID,
                position,
            ))
            .await;
            editor_selection.set(EditorSelection::collapsed(position));
            pending_cursor_position.set(None);
        });
    });

    use_effect(move || {
        let Some(current_tab) = active_tab.clone() else {
            return;
        };
        let Some(connection) = session_connection(current_tab.session_id) else {
            return;
        };

        let existing_keys = cache_snapshot.keys().cloned().collect::<HashSet<_>>();
        for relation in relations_to_prefetch.clone() {
            if existing_keys.contains(&relation.qualified_name) {
                continue;
            }

            let mut column_cache = column_cache;
            let relation_key = relation.qualified_name.clone();
            let schema = relation.schema.clone();
            let table = relation.name.clone();
            let connection = connection.clone();
            spawn(async move {
                if let Ok(columns) = services::load_table_columns(connection, schema, table).await {
                    column_cache.with_mut(|cache| {
                        cache.insert(relation_key, columns);
                    });
                }
            });
        }
    });

    rsx! {
        div {
            class: "sql-editor",
            div {
                class: "sql-editor__viewport",
                pre {
                    class: "sql-editor__highlight",
                    style: "{editor_offset}",
                    aria_hidden: "true",
                    SqlHighlightContent {
                        sql: sql.clone(),
                        inline_cursor_position: inline_completion.as_ref().map(|completion| completion.cursor_position),
                        inline_suffix: inline_completion.as_ref().map(|completion| completion.suffix.clone()),
                    }
                }
            }
            textarea {
                id: SQL_EDITOR_TEXTAREA_ID,
                class: "sql-editor__input",
                value: "{sql}",
                rows: "16",
                cols: "80",
                spellcheck: "false",
                oninput: move |event| {
                    force_autocomplete.set(false);
                    autocomplete_open.set(true);
                    selected_suggestion.set(0);
                    update_active_tab_sql(
                        tabs,
                        active_tab_id(),
                        event.value(),
                        "Ready".to_string(),
                    );
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },
                onkeydown: {
                    let suggestions = suggestions.clone();
                    let current_sql = sql.clone();
                    let inline_completion = inline_completion.clone();
                    let completion_context = completion_context.clone();
                    move |event| {
                        if event.modifiers().contains(Modifiers::CONTROL)
                            && event.code() == Code::Space
                        {
                            event.prevent_default();
                            force_autocomplete.set(true);
                            autocomplete_open.set(true);
                            selected_suggestion.set(0);
                            sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                            return;
                        }

                        if !popup_visible {
                            match event.key() {
                                Key::Tab | Key::ArrowRight => {
                                    if let Some(inline_completion) = inline_completion.as_ref() {
                                        event.prevent_default();
                                        autocomplete_open.set(false);
                                        force_autocomplete.set(false);
                                        pending_cursor_position
                                            .set(Some(inline_completion.cursor_position));
                                        let (next_sql, _) = apply_suggestion(
                                            &current_sql,
                                            selection,
                                            &inline_completion.replacement,
                                        );
                                        update_active_tab_sql(
                                            tabs,
                                            active_tab_id(),
                                            next_sql,
                                            "Inline autocomplete accepted".to_string(),
                                        );
                                    }
                                }
                                _ => {}
                            }
                            return;
                        }

                        match event.key() {
                            Key::ArrowDown => {
                                event.prevent_default();
                                if !suggestions.is_empty() {
                                    let next = (selected_suggestion() + 1)
                                        .min(suggestions.len().saturating_sub(1));
                                    selected_suggestion.set(next);
                                }
                            }
                            Key::ArrowUp => {
                                event.prevent_default();
                                selected_suggestion.set(selected_suggestion().saturating_sub(1));
                            }
                            Key::Tab | Key::Enter => {
                                if let Some(suggestion) =
                                    suggestions.get(selected_suggestion()).cloned()
                                {
                                    event.prevent_default();
                                    autocomplete_open.set(false);
                                    force_autocomplete.set(false);
                                    let replacement =
                                        resolved_replacement(&suggestion, &completion_context);
                                    let (next_sql, next_cursor_position) =
                                        apply_suggestion(&current_sql, selection, &replacement);
                                    pending_cursor_position.set(Some(next_cursor_position));
                                    update_active_tab_sql(
                                        tabs,
                                        active_tab_id(),
                                        next_sql,
                                        "Autocomplete accepted".to_string(),
                                    );
                                }
                            }
                            Key::Escape => {
                                force_autocomplete.set(false);
                                autocomplete_open.set(false);
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
                    force_autocomplete.set(false);
                    autocomplete_open.set(false);
                },
                onscroll: move |event| {
                    scroll_top.set(event.data().scroll_top());
                    scroll_left.set(event.data().scroll_left());
                }
            }

            if popup_visible {
                div { class: "sql-editor__autocomplete",
                    for (index, suggestion) in suggestions.iter().take(12).cloned().enumerate() {
                        button {
                            class: if index == selected_suggestion() {
                                "sql-editor__autocomplete-item sql-editor__autocomplete-item--active"
                            } else {
                                "sql-editor__autocomplete-item"
                            },
                            onclick: {
                                let replacement =
                                    resolved_replacement(&suggestion, &completion_context);
                                let current_sql = sql.clone();
                                move |_| {
                                    autocomplete_open.set(false);
                                    force_autocomplete.set(false);
                                    let (next_sql, next_cursor_position) =
                                        apply_suggestion(&current_sql, selection, &replacement);
                                    pending_cursor_position.set(Some(next_cursor_position));
                                    update_active_tab_sql(
                                        tabs,
                                        active_tab_id(),
                                        next_sql,
                                        "Autocomplete accepted".to_string(),
                                    );
                                }
                            },
                            div {
                                class: "sql-editor__autocomplete-copy",
                                span { class: "sql-editor__autocomplete-label", "{suggestion.label}" }
                                if !suggestion.detail.is_empty() {
                                    span { class: "sql-editor__autocomplete-detail", "{suggestion.detail}" }
                                }
                            }
                            span {
                                class: "sql-editor__autocomplete-kind",
                                "{suggestion.kind_label}"
                            }
                        }
                    }
                }
            }
        }
    }
}
