#[path = "sql_editor/highlight.rs"]
mod highlight;
#[path = "sql_editor/selection.rs"]
mod selection;

use crate::app_state::APP_UI_SETTINGS;
use crate::codestral::CodeStralClient;
use crate::screens::workspace::actions::{replace_active_tab_sql, sync_active_tab_sql_draft};
use crate::screens::workspace::components::explorer::ExplorerConnectionSection;
use dioxus::prelude::*;
use models::{ExplorerNodeKind, QueryTabState};
use std::time::Duration;

use self::{
    highlight::SqlHighlightContent,
    selection::{
        EditorSelection, current_token_range, editor_value_and_selection_query_script,
        set_editor_value_script, sync_editor_selection, sync_editor_selection_debounced,
    },
};

const SQL_EDITOR_TEXTAREA_ID: &str = "workspace-sql-editor";
const COMPLETION_DEBOUNCE_MS: u64 = 180;
const HIGHLIGHT_IDLE_MS: u64 = 90;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InlineCompletion {
    cursor: usize,
    source_sql: String,
    text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CompletionRuntime {
    request_id: u64,
    pending_snapshot: Option<usize>,
    last_completed_snapshot: Option<usize>,
    active: Option<InlineCompletion>,
}

impl CompletionRuntime {
    fn invalidate(&mut self) {
        self.request_id = self.request_id.wrapping_add(1);
        self.pending_snapshot = None;
        self.active = None;
    }

    fn reset_to_snapshot(&mut self, snapshot: usize) {
        self.invalidate();
        self.last_completed_snapshot = Some(snapshot);
    }

    fn begin_request(&mut self, snapshot: usize) -> u64 {
        self.request_id = self.request_id.wrapping_add(1);
        self.pending_snapshot = Some(snapshot);
        self.active = None;
        self.request_id
    }

    fn finish_request(&mut self, request_id: u64, snapshot: usize) -> bool {
        if self.request_id != request_id {
            return false;
        }
        self.pending_snapshot = None;
        self.last_completed_snapshot = Some(snapshot);
        true
    }

    fn set_active(
        &mut self,
        request_id: u64,
        snapshot: usize,
        cursor: usize,
        source_sql: String,
        text: String,
    ) {
        if self.finish_request(request_id, snapshot) {
            self.active = Some(InlineCompletion {
                cursor,
                source_sql,
                text,
            });
        }
    }
}

fn invalidate_completion(mut completion: Signal<CompletionRuntime>) {
    completion.with_mut(CompletionRuntime::invalidate);
}

fn invalidate_active_completion(mut completion: Signal<CompletionRuntime>) {
    completion.with_mut(|state| {
        if state.active.is_some() || state.pending_snapshot.is_some() {
            state.invalidate();
        }
    });
}

fn reset_completion_to_snapshot(mut completion: Signal<CompletionRuntime>, snapshot: usize) {
    completion.with_mut(|state| state.reset_to_snapshot(snapshot));
}

fn hash_sql(sql: &str) -> usize {
    sql.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    })
}

fn hash_completion_snapshot(sql: &str, cursor: usize) -> usize {
    hash_sql(sql).wrapping_mul(31).wrapping_add(cursor)
}

fn log_completion(_msg: &str) {}

fn is_completion_accept_key(event: &KeyboardEvent) -> bool {
    event.key() == Key::Tab || event.code() == Code::Tab
}

#[cfg(test)]
mod tests {
    use super::selection::EditorSelection;
    use super::{completion_request_parts, trim_completion_for_cursor};

    #[test]
    fn completion_request_parts_split_sql_at_cursor() {
        let sql = "select  from users";
        let cursor = "select ".len();
        let (position, prefix, suffix) =
            completion_request_parts(sql, EditorSelection::collapsed(cursor)).unwrap();

        assert_eq!(position, cursor);
        assert_eq!(prefix, "select ");
        assert_eq!(suffix.as_deref(), Some(" from users"));
    }

    #[test]
    fn trim_completion_removes_repeated_token_and_suffix_overlap() {
        let sql = "sel from users";
        let cursor = "sel".len();

        assert_eq!(
            trim_completion_for_cursor(sql, cursor, "select from users"),
            "ect"
        );
    }
}

fn completion_request_parts(
    sql: &str,
    selection: EditorSelection,
) -> Option<(usize, String, Option<String>)> {
    let selection = selection.clamped(sql);
    if selection.start != selection.end {
        return None;
    }

    let cursor = selection.end;
    Some((
        cursor,
        sql[..cursor].to_string(),
        (!sql[cursor..].is_empty()).then(|| sql[cursor..].to_string()),
    ))
}

fn trim_completion_for_cursor(sql: &str, cursor: usize, completion: &str) -> String {
    let mut completion = completion
        .trim_matches(|ch| matches!(ch, '\r' | '\n'))
        .to_string();
    if completion.is_empty() {
        return completion;
    }

    let token_range = current_token_range(sql, EditorSelection::collapsed(cursor));
    let typed_token = &sql[token_range.start..cursor];
    if !typed_token.is_empty() && completion.starts_with(typed_token) {
        completion = completion[typed_token.len()..].to_string();
    }

    let suffix = &sql[cursor..];
    let prefix_overlap = common_prefix_byte_len(suffix, &completion);
    if prefix_overlap > 0 {
        completion = completion[prefix_overlap..].to_string();
    }

    let suffix_overlap = suffix_prefix_overlap_byte_len(suffix, &completion);
    if suffix_overlap > 0 {
        completion.truncate(completion.len() - suffix_overlap);
    }

    completion
}

fn common_prefix_byte_len(left: &str, right: &str) -> usize {
    let mut byte_len = 0;
    for (left_ch, right_ch) in left.chars().zip(right.chars()) {
        if left_ch != right_ch {
            break;
        }
        byte_len += right_ch.len_utf8();
    }
    byte_len
}

fn suffix_prefix_overlap_byte_len(suffix: &str, completion: &str) -> usize {
    let mut best_overlap = 0;
    let mut suffix_prefix_len = 0;
    for ch in suffix.chars() {
        suffix_prefix_len += ch.len_utf8();
        if completion.ends_with(&suffix[..suffix_prefix_len]) {
            best_overlap = suffix_prefix_len;
        }
    }
    best_overlap
}

fn build_schema_context(sections: &[ExplorerConnectionSection], session_id: u64) -> String {
    let section = match sections.iter().find(|s| s.session_id == session_id) {
        Some(s) => s,
        None => return String::new(),
    };

    let mut parts: Vec<String> = Vec::new();

    for node in &section.nodes {
        if node.kind == ExplorerNodeKind::Schema {
            let schema_name = &node.name;
            for table in &node.children {
                if table.kind == ExplorerNodeKind::Table || table.kind == ExplorerNodeKind::View {
                    let columns: Vec<String> =
                        table.children.iter().map(|col| col.name.clone()).collect();
                    if columns.is_empty() {
                        parts.push(format!("{}.{}", schema_name, table.name));
                    } else {
                        parts.push(format!(
                            "{}.{}({})",
                            schema_name,
                            table.name,
                            columns.join(", ")
                        ));
                    }
                }
            }
        } else if node.kind == ExplorerNodeKind::Table || node.kind == ExplorerNodeKind::View {
            let columns: Vec<String> = node.children.iter().map(|col| col.name.clone()).collect();
            if columns.is_empty() {
                parts.push(node.name.clone());
            } else {
                parts.push(format!("{}({})", node.name, columns.join(", ")));
            }
        }
    }

    if parts.is_empty() {
        return String::new();
    }

    format!("-- Database schema: {}\n", parts.join(", "))
}

#[component]
pub fn SqlEditor(
    sql: String,
    active_tab: QueryTabState,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    explorer_sections: Signal<Vec<ExplorerConnectionSection>>,
) -> Element {
    let _active_tab_id_signal = active_tab_id;
    let active_tab_id_value = active_tab.id;
    let active_session_id = active_tab.session_id;
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let mut draft_sql = use_signal(|| sql.clone());
    let mut editor_selection = use_signal(|| EditorSelection::collapsed(sql.len()));
    let mut editor_revision = use_signal(|| 0_u64);
    let mut is_typing = use_signal(|| false);
    let mut completion_runtime = use_signal(CompletionRuntime::default);
    let mut has_synced_editor_dom = use_signal(|| false);
    let mut synced_editor_tab_id = use_signal(|| active_tab_id_value);

    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    let schema_context = use_memo(use_reactive((&active_session_id,), move |(session_id,)| {
        build_schema_context(&explorer_sections(), session_id)
    }));

    use_effect(use_reactive(
        (&active_tab_id_value, &sql),
        move |(tab_id, next_sql)| {
            let first_sync = !*has_synced_editor_dom.peek();
            let tab_changed = *synced_editor_tab_id.peek() != tab_id;
            let draft_matches = {
                let current_sql = draft_sql.peek();
                current_sql.as_str() == next_sql.as_str()
            };
            if !first_sync && !tab_changed && draft_matches {
                return;
            }

            has_synced_editor_dom.set(true);
            synced_editor_tab_id.set(tab_id);
            draft_sql.set(next_sql.clone());
            editor_selection.set(EditorSelection::collapsed(next_sql.len()));
            is_typing.set(false);
            reset_completion_to_snapshot(
                completion_runtime,
                hash_completion_snapshot(&next_sql, next_sql.len()),
            );
            let cursor = next_sql.len();
            spawn(async move {
                let _ = document::eval(&set_editor_value_script(
                    SQL_EDITOR_TEXTAREA_ID,
                    &next_sql,
                    cursor,
                    false,
                ))
                .join::<bool>()
                .await;
            });
        },
    ));

    use_effect(move || {
        if !is_typing() {
            return;
        }

        let revision = editor_revision();
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(HIGHLIGHT_IDLE_MS)).await;
            if editor_revision() == revision {
                is_typing.set(false);
            }
        });
    });

    use_effect(move || {
        let revision = editor_revision();

        spawn(async move {
            tokio::time::sleep(Duration::from_millis(90)).await;
            if editor_revision() != revision {
                return;
            }

            let Ok((next_sql, start, end)) = document::eval(
                &editor_value_and_selection_query_script(SQL_EDITOR_TEXTAREA_ID),
            )
            .join::<(String, usize, usize)>()
            .await
            else {
                return;
            };
            let draft_changed = {
                let current_sql = draft_sql.peek();
                current_sql.as_str() != next_sql.as_str()
            };
            if draft_changed {
                draft_sql.set(next_sql.clone());
            }
            let next_selection = EditorSelection { start, end };
            let selection_changed = {
                let current_selection = editor_selection.peek();
                *current_selection != next_selection
            };
            if selection_changed {
                editor_selection.set(next_selection);
            }
            let already_synced = tabs
                .read()
                .iter()
                .find(|tab| tab.id == active_tab_id_value)
                .is_some_and(|tab| tab.sql == next_sql);
            if already_synced {
                return;
            }

            sync_active_tab_sql_draft(tabs, active_tab_id_value, next_sql);
        });
    });

    use_effect(move || {
        let revision = editor_revision();
        let settings = APP_UI_SETTINGS();

        if !settings.codestral.enabled || settings.codestral.api_key.is_empty() {
            invalidate_completion(completion_runtime);
            return;
        }

        spawn(async move {
            tokio::time::sleep(Duration::from_millis(COMPLETION_DEBOUNCE_MS)).await;
            if editor_revision() != revision {
                return;
            }

            let Ok((sql_text, start, end)) = document::eval(
                &editor_value_and_selection_query_script(SQL_EDITOR_TEXTAREA_ID),
            )
            .join::<(String, usize, usize)>()
            .await
            else {
                return;
            };
            if sql_text.len() < 3 {
                invalidate_completion(completion_runtime);
                return;
            }

            let selection = EditorSelection { start, end };
            let Some((cursor, prefix, suffix)) = completion_request_parts(&sql_text, selection)
            else {
                invalidate_completion(completion_runtime);
                return;
            };

            if !APP_UI_SETTINGS().codestral.enabled
                || APP_UI_SETTINGS().codestral.api_key.is_empty()
            {
                invalidate_completion(completion_runtime);
                return;
            }

            let sql_hash = hash_completion_snapshot(&sql_text, cursor);
            let completion_snapshot = completion_runtime.peek().clone();
            if completion_snapshot.last_completed_snapshot == Some(sql_hash)
                && completion_snapshot.pending_snapshot.is_none()
            {
                return;
            }

            let expected_id = completion_runtime.with_mut(|state| state.begin_request(sql_hash));
            let schema_ctx = schema_context();
            let sql_for_result = sql_text.clone();
            let suffix_for_api = suffix.clone();
            let prompt = format!("{}{}", schema_ctx, prefix);
            log_completion(&format!(
                "calling API with schema context ({} chars), sql: {}",
                schema_ctx.len(),
                sql_for_result
            ));
            let client = CodeStralClient::new(APP_UI_SETTINGS().codestral);
            match client
                .get_completion(&prompt, suffix_for_api.as_deref())
                .await
            {
                Ok(Some(completion)) if !completion.is_empty() => {
                    if completion_runtime.peek().request_id != expected_id {
                        return;
                    }

                    let completion =
                        trim_completion_for_cursor(&sql_for_result, cursor, &completion);
                    if completion.trim().is_empty() {
                        completion_runtime.with_mut(|state| {
                            state.finish_request(expected_id, sql_hash);
                        });
                        return;
                    }

                    log_completion(&format!("got completion: {}", completion));
                    completion_runtime.with_mut(|state| {
                        state.set_active(
                            expected_id,
                            sql_hash,
                            cursor,
                            sql_for_result.clone(),
                            completion,
                        );
                    });
                }
                Ok(None) => {
                    log_completion("API returned None");
                    completion_runtime.with_mut(|state| {
                        state.finish_request(expected_id, sql_hash);
                    });
                }
                Ok(Some(empty)) => {
                    log_completion(&format!("API returned empty: {:?}", empty));
                    completion_runtime.with_mut(|state| {
                        state.finish_request(expected_id, sql_hash);
                    });
                }
                Err(e) => {
                    log_completion(&format!("API error: {:?}", e));
                    completion_runtime.with_mut(|state| {
                        state.finish_request(expected_id, sql_hash);
                    });
                }
            }
        });
    });

    let typing_now = is_typing();
    let active_completion = completion_runtime().active;
    let render_completion = active_completion.as_ref().filter(|completion| {
        let cursor = completion.cursor.min(completion.source_sql.len());
        !completion.text.is_empty()
            && !trim_completion_for_cursor(&completion.source_sql, cursor, &completion.text)
                .is_empty()
    });
    let current_sql = render_completion
        .map(|completion| completion.source_sql.clone())
        .unwrap_or_else(|| {
            if typing_now {
                draft_sql.peek().clone()
            } else {
                draft_sql()
            }
        });
    let editor_class = if typing_now {
        "sql-editor sql-editor--typing"
    } else {
        "sql-editor"
    };
    let inline_cursor =
        render_completion.map_or(0, |completion| completion.cursor.min(current_sql.len()));
    let inline_suffix = render_completion.map(|completion| {
        trim_completion_for_cursor(&current_sql, inline_cursor, &completion.text)
    });
    let completion_active = inline_suffix
        .as_ref()
        .is_some_and(|completion| !completion.is_empty());
    let inline_cursor_position = completion_active.then_some(inline_cursor);

    rsx! {
        div {
            class: "{editor_class}",

            div {
                class: "sql-editor__viewport",
                pre {
                    class: "sql-editor__highlight",
                    style: "{editor_offset}",
                    aria_hidden: "true",
                    if !typing_now || completion_active {
                        SqlHighlightContent {
                            sql: current_sql.clone(),
                            inline_cursor_position,
                            inline_suffix,
                        }
                    }
                }
            }

            textarea {
                id: SQL_EDITOR_TEXTAREA_ID,
                class: "sql-editor__input",
                initial_value: "{current_sql}",
                rows: "16",
                cols: "80",
                spellcheck: "false",

                oninput: move |event| {
                    let next_sql = event.value();
                    let draft_changed = {
                        let current_sql = draft_sql.peek();
                        current_sql.as_str() != next_sql.as_str()
                    };
                    if draft_changed {
                        // Keep the render snapshot aligned with the live textarea so the
                        // highlight layer never wakes up with stale SQL after the typing debounce.
                        draft_sql.set(next_sql.clone());
                        sync_active_tab_sql_draft(tabs, active_tab_id_value, next_sql);
                    }
                    let already_typing = {
                        let typing = is_typing.peek();
                        *typing
                    };
                    if !already_typing {
                        is_typing.set(true);
                    }
                    invalidate_active_completion(completion_runtime);
                    editor_revision += 1;
                },

                onkeydown: move |event| {
                    let active_completion = {
                        let completion_state = completion_runtime.peek();
                        completion_state.active.clone()
                    };

                    if is_completion_accept_key(&event)
                        && let Some(completion_state) = active_completion.clone()
                        && !completion_state.text.is_empty()
                    {
                        event.prevent_default();
                        let actual_sql = completion_state.source_sql;
                        let cursor = completion_state.cursor.min(actual_sql.len());
                        let cursor = if actual_sql.is_char_boundary(cursor) {
                            cursor
                        } else {
                            EditorSelection::collapsed(cursor).clamped(&actual_sql).end
                        };
                        let completion_text = trim_completion_for_cursor(
                            &actual_sql,
                            cursor,
                            &completion_state.text,
                        );
                        if completion_text.is_empty() {
                            return;
                        }
                        let new_cursor = cursor + completion_text.len();
                        let new_sql = format!(
                            "{}{}{}",
                            &actual_sql[..cursor],
                            completion_text,
                            &actual_sql[cursor..]
                        );
                        draft_sql.set(new_sql.clone());
                        editor_selection.set(EditorSelection::collapsed(new_cursor));
                        is_typing.set(false);
                        reset_completion_to_snapshot(
                            completion_runtime,
                            hash_completion_snapshot(&new_sql, new_cursor),
                        );
                        editor_revision += 1;
                        let new_sql_for_dom = new_sql.clone();
                        replace_active_tab_sql(
                            tabs,
                            active_tab_id_value,
                            new_sql,
                            "Ready".to_string(),
                        );
                        spawn(async move {
                            let _ = document::eval(&set_editor_value_script(
                                SQL_EDITOR_TEXTAREA_ID,
                                &new_sql_for_dom,
                                new_cursor,
                                true,
                            ))
                            .join::<bool>()
                            .await;
                        });
                    }
                },

                onkeyup: move |event| {
                    match event.key() {
                        Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::ArrowUp
                        | Key::ArrowDown
                        | Key::Home
                        | Key::End
                        | Key::PageUp
                        | Key::PageDown => {
                            editor_revision += 1;
                            sync_editor_selection_debounced(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                        }
                        _ => {}
                    }
                },

                onmouseup: move |_| {
                    editor_revision += 1;
                    sync_editor_selection_debounced(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onclick: move |_| {
                    editor_revision += 1;
                    sync_editor_selection_debounced(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onfocus: move |_| {
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onscroll: move |event| {
                    scroll_top.set(event.data().scroll_top());
                    scroll_left.set(event.data().scroll_left());
                },
            }
        }
    }
}
