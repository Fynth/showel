#[path = "sql_editor/highlight.rs"]
mod highlight;
#[path = "sql_editor/selection.rs"]
mod selection;

use crate::app_state::APP_UI_SETTINGS;
use crate::codestral::CodeStralClient;
use crate::screens::workspace::actions::replace_active_tab_sql;
use crate::screens::workspace::components::explorer::ExplorerConnectionSection;
use dioxus::prelude::*;
use models::{ExplorerNodeKind, QueryTabState};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use self::{
    highlight::SqlHighlightContent,
    selection::{EditorSelection, set_editor_selection_script, sync_editor_selection},
};

const SQL_EDITOR_TEXTAREA_ID: &str = "workspace-sql-editor";
const COMPLETION_DEBOUNCE_MS: u64 = 500;

static COMPLETION_ACTIVE: AtomicBool = AtomicBool::new(false);
static COMPLETION_TEXT: Mutex<String> = Mutex::new(String::new());
static COMPLETION_REQUEST_ID: AtomicUsize = AtomicUsize::new(0);
static LAST_SQL_HASH: AtomicUsize = AtomicUsize::new(0);
static PENDING_SQL: Mutex<Option<String>> = Mutex::new(None);

fn hash_sql(sql: &str) -> usize {
    sql.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    })
}

fn log_completion(_msg: &str) {}

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
    let active_tab_id_value = active_tab.id;
    let active_session_id = active_tab.session_id;
    let synced_tab_sql = active_tab.sql.clone();
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let mut draft_sql = use_signal(|| sql.clone());
    let mut editor_selection = use_signal(|| EditorSelection::collapsed(sql.len()));
    let mut sync_revision = use_signal(|| 0_u64);
    let mut completion_version = use_signal(|| 0_u64);

    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    let completion_text = use_memo(move || {
        let _ = completion_version();
        COMPLETION_TEXT.lock().unwrap().clone()
    });

    let has_completion = use_memo(move || {
        let _ = completion_version();
        COMPLETION_ACTIVE.load(Ordering::SeqCst) && !COMPLETION_TEXT.lock().unwrap().is_empty()
    });

    use_effect(move || {
        let _ = active_tab_id();
        draft_sql.set(sql.clone());
        editor_selection.set(EditorSelection::collapsed(sql.len()));
        COMPLETION_ACTIVE.store(false, Ordering::SeqCst);
        COMPLETION_TEXT.lock().unwrap().clear();
        LAST_SQL_HASH.store(hash_sql(&sql), Ordering::SeqCst);
        PENDING_SQL.lock().unwrap().take();
        completion_version.set(0);
    });

    use_effect(move || {
        let next_sql = draft_sql();
        let revision = sync_revision();
        if next_sql == synced_tab_sql {
            return;
        }

        COMPLETION_ACTIVE.store(false, Ordering::SeqCst);
        COMPLETION_TEXT.lock().unwrap().clear();
        completion_version.set(completion_version() + 1);

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

    let draft_for_completion = draft_sql;
    let mut completion_version_for_effect = completion_version;
    let sections_for_completion = explorer_sections;

    use_effect(move || {
        let sql_text = draft_for_completion();
        let settings = APP_UI_SETTINGS();

        if sql_text.len() < 3 {
            if COMPLETION_ACTIVE.load(Ordering::SeqCst) {
                COMPLETION_ACTIVE.store(false, Ordering::SeqCst);
                COMPLETION_TEXT.lock().unwrap().clear();
                completion_version_for_effect.set(completion_version_for_effect() + 1);
            }
            PENDING_SQL.lock().unwrap().take();
            return;
        }

        if !settings.codestral.enabled || settings.codestral.api_key.is_empty() {
            return;
        }

        let sql_hash = hash_sql(&sql_text);
        if sql_hash == LAST_SQL_HASH.load(Ordering::SeqCst) && PENDING_SQL.lock().unwrap().is_none()
        {
            return;
        }

        let expected_id = COMPLETION_REQUEST_ID.load(Ordering::SeqCst);
        *PENDING_SQL.lock().unwrap() = Some(sql_text.clone());

        let schema_ctx = build_schema_context(&sections_for_completion(), active_session_id);
        let sql_for_api = sql_text.clone();

        spawn(async move {
            tokio::time::sleep(Duration::from_millis(COMPLETION_DEBOUNCE_MS)).await;

            if COMPLETION_REQUEST_ID.load(Ordering::SeqCst) != expected_id {
                *PENDING_SQL.lock().unwrap() = None;
                return;
            }

            if !APP_UI_SETTINGS().codestral.enabled
                || APP_UI_SETTINGS().codestral.api_key.is_empty()
            {
                *PENDING_SQL.lock().unwrap() = None;
                return;
            }

            let prompt = format!("{}{}", schema_ctx, sql_for_api);
            log_completion(&format!(
                "calling API with schema context ({} chars), sql: {}",
                schema_ctx.len(),
                sql_for_api
            ));
            let client = CodeStralClient::new(APP_UI_SETTINGS().codestral);
            match client.get_completion(&prompt, None).await {
                Ok(Some(completion)) if !completion.is_empty() => {
                    if COMPLETION_REQUEST_ID.load(Ordering::SeqCst) != expected_id {
                        *PENDING_SQL.lock().unwrap() = None;
                        return;
                    }
                    log_completion(&format!("got completion: {}", completion));
                    *COMPLETION_TEXT.lock().unwrap() = completion.clone();
                    COMPLETION_ACTIVE.store(true, Ordering::SeqCst);
                    LAST_SQL_HASH.store(hash_sql(&sql_for_api), Ordering::SeqCst);
                    *PENDING_SQL.lock().unwrap() = None;
                    completion_version_for_effect.set(completion_version_for_effect() + 1);
                }
                Ok(None) => {
                    log_completion("API returned None");
                    *PENDING_SQL.lock().unwrap() = None;
                }
                Ok(Some(empty)) => {
                    log_completion(&format!("API returned empty: {:?}", empty));
                    *PENDING_SQL.lock().unwrap() = None;
                }
                Err(e) => {
                    log_completion(&format!("API error: {:?}", e));
                    *PENDING_SQL.lock().unwrap() = None;
                }
            }
        });
    });

    let current_sql = draft_sql();

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
                        inline_cursor_position: if has_completion() { Some(current_sql.len()) } else { None },
                        inline_suffix: if has_completion() { Some(completion_text()) } else { None },
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
                    let new_sql = event.value();
                    draft_sql.set(new_sql.clone());
                    COMPLETION_ACTIVE.store(false, Ordering::SeqCst);
                    COMPLETION_TEXT.lock().unwrap().clear();
                    PENDING_SQL.lock().unwrap().take();
                    COMPLETION_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
                    completion_version.set(completion_version() + 1);
                    sync_revision += 1;
                    sync_editor_selection(editor_selection, SQL_EDITOR_TEXTAREA_ID);
                },

                onkeydown: move |event| {
                    if event.key() == Key::Tab {
                        let is_active = COMPLETION_ACTIVE.load(Ordering::SeqCst);
                        let completion = COMPLETION_TEXT.lock().unwrap().clone();
                        if is_active && !completion.is_empty() {
                            event.prevent_default();
                            let actual_sql = draft_sql();
                            let new_sql = format!("{}{}", actual_sql, completion);
                            let cursor = new_sql.len();
                            draft_sql.set(new_sql.clone());
                            COMPLETION_ACTIVE.store(false, Ordering::SeqCst);
                            COMPLETION_TEXT.lock().unwrap().clear();
                            LAST_SQL_HASH.store(hash_sql(&new_sql), Ordering::SeqCst);
                            PENDING_SQL.lock().unwrap().take();
                            COMPLETION_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
                            completion_version.set(completion_version() + 1);
                            sync_revision += 1;
                            replace_active_tab_sql(tabs, active_tab_id_value, new_sql, "Ready".to_string());
                            spawn(async move {
                                let _ = document::eval(&set_editor_selection_script(SQL_EDITOR_TEXTAREA_ID, cursor)).join::<bool>().await;
                            });
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

                onscroll: move |event| {
                    scroll_top.set(event.data().scroll_top());
                    scroll_left.set(event.data().scroll_left());
                },
            }
        }
    }
}
