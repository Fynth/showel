#[path = "sql_editor/acp_inline_completion.rs"]
mod acp_inline_completion;
#[path = "sql_editor/autocomplete.rs"]
mod autocomplete;
#[path = "sql_editor/highlight.rs"]
mod highlight;
#[path = "sql_editor/selection.rs"]
mod selection;

use crate::{app_state::session_connection, screens::workspace::actions::replace_active_tab_sql};
use dioxus::prelude::*;
use models::{AcpPanelState, ExplorerNode, QueryTabState};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use self::{
    acp_inline_completion::{
        AcpInlineCompletionState, build_inline_completion_prompt,
        build_schema_hint, extract_editor_context,
    },
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
const ACP_DEBOUNCE_MS: u64 = 125;
const ACP_CONTEXT_WINDOW: usize = 500;

#[component]
pub fn SqlEditor(
    sql: String,
    active_tab: QueryTabState,
    explorer_nodes: Vec<ExplorerNode>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    acp_panel_state: Signal<AcpPanelState>,
    ai_features_enabled: Signal<bool>,
) -> Element {
    let active_tab_id_value = active_tab.id;
    let synced_tab_sql = active_tab.sql.clone();
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let mut autocomplete_open = use_signal(|| false);
    let mut force_autocomplete = use_signal(|| false);
    let mut selected_suggestion = use_signal(|| 0usize);
    let mut suggestion_key = use_signal(String::new);
    let mut draft_sql = use_signal(|| sql.clone());
    let mut editor_selection = use_signal(|| EditorSelection::collapsed(sql.len()));
    let mut pending_cursor_position = use_signal(|| None::<usize>);
    let mut sync_revision = use_signal(|| 0_u64);
    let column_cache = use_signal(HashMap::<String, Vec<String>>::new);
    let mut acp_inline_completion = use_signal(AcpInlineCompletionState::new);
    let mut acp_request_revision = use_signal(|| 0_u64);
    let current_sql = draft_sql();

    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    let selection = editor_selection().clamped(&current_sql);
    let completion_context = completion_context(&current_sql, selection);
    let autocomplete_active = force_autocomplete()
        || autocomplete_open()
        || should_open_autocomplete(&completion_context);
    let catalog = if autocomplete_active {
        flatten_catalog(&explorer_nodes)
    } else {
        autocomplete::AutocompleteCatalog::default()
    };
    let relation_bindings = if autocomplete_active {
        extract_relation_bindings(&current_sql, &catalog)
    } else {
        Vec::new()
    };
    let relations_to_prefetch = if autocomplete_active {
        relations_to_prefetch(
            Some(&active_tab),
            &catalog,
            &relation_bindings,
            &completion_context,
        )
    } else {
        Vec::new()
    };
    let cache_snapshot = column_cache();
    let suggestions = if autocomplete_active {
        build_suggestions(
            &current_sql,
            selection,
            Some(&active_tab),
            &catalog,
            &relation_bindings,
            &cache_snapshot,
            force_autocomplete(),
        )
    } else {
        Vec::new()
    };
    let inline_completion = if autocomplete_active {
        build_inline_completion(
            &current_sql,
            selection,
            &completion_context,
            if force_autocomplete() {
                suggestions
                    .get(selected_suggestion())
                    .or_else(|| suggestions.first())
            } else {
                suggestions.first()
            },
        )
    } else {
        None
    };
    let popup_visible = force_autocomplete() && autocomplete_open() && !suggestions.is_empty();
    let sql_len = current_sql.len();
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
        let next_sql = sql.clone();
        let next_len = next_sql.len();
        draft_sql.set(next_sql);
        editor_selection.set(EditorSelection::collapsed(next_len));
        autocomplete_open.set(false);
        force_autocomplete.set(false);
    });

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
        let current_tab = active_tab.clone();
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
                if let Ok(columns) = explorer::load_table_columns(connection, schema, table).await {
                    column_cache.with_mut(|cache| {
                        cache.insert(relation_key, columns);
                    });
                }
            });
        }
    });

    // ACP inline completion effect - debounced trigger
    use_effect(move || {
        let _ = draft_sql();
        let _ = editor_selection();
        let revision = sync_revision();
        
        // Check if ACP is connected and AI features are enabled
        let acp_connected = acp_panel_state().connected;
        let ai_enabled = ai_features_enabled();
        if !acp_connected || !ai_enabled {
            return;
        }
        
        // Cancel any pending ACP request
        acp_inline_completion.with_mut(|state| {
            state.cancel_pending();
            state.clear_suggestion();
        });
        
        // Increment revision to track this request
        acp_request_revision += 1;
        let current_revision = acp_request_revision();
        
        // Get context for ACP prompt
        let cursor_pos = selection.start;
        let sql_text = draft_sql();
        let (text_before, text_after, cursor_offset) = extract_editor_context(
            &sql_text,
            cursor_pos,
            ACP_CONTEXT_WINDOW,
        );
        
        // Build schema hint from catalog
        let schema_hint = if autocomplete_active {
            Some(build_schema_hint(&catalog.schemas, &catalog.relations, 10))
        } else {
            None
        };
        
        // Spawn debounced ACP request
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(ACP_DEBOUNCE_MS)).await;
            
            // Check if revision is still current (not stale)
            if acp_request_revision() != current_revision {
                return;
            }
            
            // Build prompt
            let prompt = build_inline_completion_prompt(
                &text_before,
                &text_after,
                cursor_offset,
                schema_hint.as_deref(),
            );
            
            // Send ACP prompt
            if let Err(_err) = acp::send_acp_prompt(prompt) {
                return;
            }
            
            // Mark as loading
            acp_inline_completion.with_mut(|state| {
                state.is_loading = true;
                state.is_discarded = false;
            });
        });
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
                        sql: current_sql.clone(),
                        inline_cursor_position: inline_completion.as_ref().map(|completion| completion.cursor_position),
                        inline_suffix: inline_completion.as_ref().map(|completion| completion.suffix.clone()),
                    }
                }
            }
            textarea {
                id: SQL_EDITOR_TEXTAREA_ID,
                class: "sql-editor__input",
                value: "{current_sql}",
                rows: "16",
                cols: "80",
                spellcheck: "false",
                oninput: move |event| {
                    force_autocomplete.set(false);
                    autocomplete_open.set(true);
                    selected_suggestion.set(0);
                    // Cancel pending ACP request and clear completion on new keystroke
                    acp_inline_completion.with_mut(|state| {
                        state.cancel_pending();
                        state.clear_suggestion();
                    });
                    draft_sql.set(event.value());
                    sync_revision += 1;
                },
                onkeydown: {
                    let suggestions = suggestions.clone();
                    let current_sql = current_sql.clone();
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

                        // Handle ACP inline completion
                        let acp_has_completion = acp_inline_completion.with(|state| state.has_completion());
                        if acp_has_completion {
                            match event.key() {
                                Key::Tab | Key::ArrowRight => {
                                    event.prevent_default();
                                    if let Some((suggestion, cursor_pos)) = acp_inline_completion.with_mut(|state| state.accept_completion()) {
                                        let insert_position = selection.start;
                                        let next_sql = format!(
                                            "{}{}{}",
                                            &current_sql[..insert_position],
                                            suggestion,
                                            &current_sql[insert_position..]
                                        );
                                        pending_cursor_position.set(Some(cursor_pos));
                                        draft_sql.set(next_sql.clone());
                                        sync_revision += 1;
                                        replace_active_tab_sql(
                                            tabs,
                                            active_tab_id(),
                                            next_sql,
                                            "ACP inline completion accepted".to_string(),
                                        );
                                    }
                                    return;
                                }
                                Key::Escape => {
                                    event.prevent_default();
                                    acp_inline_completion.with_mut(|state| state.dismiss_completion());
                                    return;
                                }
                                _ => {}
                            }
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
                                        draft_sql.set(next_sql.clone());
                                        sync_revision += 1;
                                        replace_active_tab_sql(
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
                                    draft_sql.set(next_sql.clone());
                                    sync_revision += 1;
                                    replace_active_tab_sql(
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
                                let current_sql = current_sql.clone();
                                move |_| {
                                    autocomplete_open.set(false);
                                    force_autocomplete.set(false);
                                    let (next_sql, next_cursor_position) =
                                        apply_suggestion(&current_sql, selection, &replacement);
                                    pending_cursor_position.set(Some(next_cursor_position));
                                    draft_sql.set(next_sql.clone());
                                    sync_revision += 1;
                                    replace_active_tab_sql(
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
