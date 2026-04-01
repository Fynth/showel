#[path = "sql_editor/highlight.rs"]
mod highlight;
#[path = "sql_editor/selection.rs"]
mod selection;

use crate::screens::workspace::actions::replace_active_tab_sql;
use dioxus::prelude::*;
use models::QueryTabState;
use std::time::Duration;

use self::{
    highlight::SqlHighlightContent,
    selection::{EditorSelection, sync_editor_selection},
};

const SQL_EDITOR_TEXTAREA_ID: &str = "workspace-sql-editor";

#[component]
pub fn SqlEditor(
    sql: String,
    active_tab: QueryTabState,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let active_tab_id_value = active_tab.id;
    let synced_tab_sql = active_tab.sql.clone();
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let mut draft_sql = use_signal(|| sql.clone());
    let mut editor_selection = use_signal(|| EditorSelection::collapsed(sql.len()));
    let mut sync_revision = use_signal(|| 0_u64);
    let current_sql = draft_sql();

    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    use_effect(move || {
        let _ = active_tab_id();
        let next_sql = sql.clone();
        let next_len = next_sql.len();
        draft_sql.set(next_sql);
        editor_selection.set(EditorSelection::collapsed(next_len));
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
                        inline_cursor_position: None,
                        inline_suffix: None,
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
                    draft_sql.set(event.value());
                    sync_revision += 1;
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
                }
            }
        }
    }
}
