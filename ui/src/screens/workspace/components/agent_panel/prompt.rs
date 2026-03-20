use dioxus::prelude::*;
use models::{AcpMessageKind, AcpPanelState, QueryOutput, QueryPage, QueryTabState};

use crate::{app_state::session_connection, screens::workspace::actions::update_active_tab_sql};

use super::state::push_message;

const MAX_ACTIVE_RESULT_ROWS: usize = 5;

pub(crate) fn extract_sql_candidate(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(sql) = extract_fenced_block(trimmed, "sql") {
        return Some(sql);
    }
    if let Some(sql) = extract_any_fenced_block(trimmed) {
        return Some(sql);
    }

    let lowered = trimmed.to_ascii_lowercase();
    [
        "select", "with", "insert", "update", "delete", "create", "alter", "drop", "truncate",
    ]
    .iter()
    .any(|keyword| lowered.starts_with(keyword))
    .then(|| trimmed.to_string())
}

fn extract_fenced_block(text: &str, language: &str) -> Option<String> {
    let needle = format!("```{language}");
    let start = text.find(&needle)?;
    let rest = &text[start + needle.len()..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

fn extract_any_fenced_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let rest = match rest.find('\n') {
        Some(newline) => &rest[newline + 1..],
        None => rest,
    };
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

pub(super) fn build_sql_generation_prompt(
    connection_label: &str,
    request: &str,
    db_context: Option<String>,
    active_tab_context: Option<String>,
) -> String {
    let mut prompt = format!(
        "You are generating SQL for the active database connection.\n\
Database context: {connection_label}\n"
    );
    if let Some(active_tab_context) = active_tab_context {
        prompt.push_str("Use this active editor context too:\n");
        prompt.push_str(&active_tab_context);
        prompt.push('\n');
    }
    if let Some(db_context) = db_context {
        prompt.push_str("Use this live database snapshot:\n");
        prompt.push_str(&db_context);
        prompt.push('\n');
    }
    prompt.push_str(
        "Snapshot rows are previews only. Never infer total row counts, aggregates, or full-table statistics unless a query result explicitly provides them.\n\
If the available context is insufficient, generate SQL that verifies the answer instead of guessing.\n\
When creating tables, always define an auto-generated primary key `id`.\n\
For SQLite use `id INTEGER PRIMARY KEY AUTOINCREMENT`.\n\
For PostgreSQL use `id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY`.\n\
When inserting rows, omit the `id` column unless the user explicitly asks to provide it manually.\n\
Return exactly one SQL query inside a single ```sql``` block with no explanation.\n",
    );
    prompt.push_str(&format!("User request: {request}"));
    prompt
}

pub(super) fn insert_sql_into_editor(
    mut panel_state: Signal<AcpPanelState>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    mut show_sql_editor: Signal<bool>,
    sql: String,
) {
    if active_tab_id == 0 {
        panel_state.with_mut(|state| {
            state.status = "No active SQL tab to insert into.".to_string();
            push_message(
                state,
                AcpMessageKind::Error,
                "No active SQL tab to insert into.".to_string(),
            );
        });
        return;
    }

    show_sql_editor.set(true);
    update_active_tab_sql(
        tabs,
        active_tab_id,
        sql,
        "SQL inserted from ACP agent".to_string(),
    );
    panel_state.with_mut(|state| {
        state.pending_sql_insert = false;
        state.status = "Inserted agent SQL into the active editor.".to_string();
    });
}

pub(super) fn build_chat_prompt(
    connection_label: &str,
    prompt: &str,
    db_context: Option<String>,
    active_tab_context: Option<String>,
) -> String {
    let mut message = format!(
        "You are helping with the active database connection.\n\
Database context: {connection_label}\n"
    );
    if let Some(active_tab_context) = active_tab_context {
        message.push_str("Use this active editor context when it is relevant:\n");
        message.push_str(&active_tab_context);
        message.push('\n');
    }
    if let Some(db_context) = db_context {
        message.push_str("Use this live database snapshot when answering:\n");
        message.push_str(&db_context);
        message.push('\n');
    }
    message.push_str(
        "Always answer in English.\n\
Snapshot rows are previews only. Never infer total row counts, aggregates, or full-table statistics unless a query result explicitly provides them.\n\
If the available context is insufficient, say exactly what is unknown and give the SQL needed to verify it.\n\
Prefer facts from the active editor context over generic assumptions.\n\
If you propose schema creation, always use an auto-generated primary key `id`.\n\
If you propose inserts, omit `id` unless the user explicitly asks for manual ids.\n",
    );
    message.push_str(&format!("User request: {prompt}"));
    message
}

pub(super) fn active_editor_prompt_context(
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
) -> Option<String> {
    let tabs = tabs.read();
    let tab = tabs.iter().find(|tab| tab.id == active_tab_id)?;
    build_active_tab_context(tab)
}

fn build_active_tab_context(tab: &QueryTabState) -> Option<String> {
    let mut sections = Vec::new();
    let sql = tab.sql.trim();
    if !sql.is_empty() {
        sections.push(format!("Active editor SQL:\n```sql\n{sql}\n```"));
    }

    let status = tab.status.trim();
    if !status.is_empty() {
        sections.push(format!("Active tab status: {status}"));
    }

    if let Some(result) = &tab.result {
        sections.push(build_result_context(result));
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

fn build_result_context(result: &QueryOutput) -> String {
    match result {
        QueryOutput::AffectedRows(rows) => {
            format!("Active tab result: the last statement affected {rows} row(s).")
        }
        QueryOutput::Table(page) => build_page_result_context(page),
    }
}

fn build_page_result_context(page: &QueryPage) -> String {
    if page.columns.is_empty() && page.rows.is_empty() {
        return "Active tab result: the query returned no rows.".to_string();
    }

    let mut lines = Vec::new();
    if page.rows.is_empty() {
        lines.push("Active tab result preview: no rows on the current page.".to_string());
    } else {
        let first_row = page.offset + 1;
        let last_row = page.offset + page.rows.len() as u64;
        let preview_scope = if page.has_next {
            " More rows exist beyond this preview."
        } else {
            ""
        };
        lines.push(format!(
            "Active tab result preview: rows {first_row}-{last_row} from the current page.{preview_scope}"
        ));
    }

    if !page.columns.is_empty() {
        lines.push(format!("Result columns: {}", page.columns.join(", ")));
    }

    for row in page.rows.iter().take(MAX_ACTIVE_RESULT_ROWS) {
        let cells = page
            .columns
            .iter()
            .zip(row.iter())
            .map(|(column, value)| format!("{column}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("result row: {cells}"));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_ACTIVE_RESULT_ROWS, build_active_tab_context, build_chat_prompt,
        build_sql_generation_prompt,
    };
    use models::{PendingTableChanges, QueryOutput, QueryPage, QueryTabState, WorkspaceTabKind};

    #[test]
    fn chat_prompt_requires_english_and_preview_safety() {
        let prompt = build_chat_prompt(
            "SQLite",
            "Summarize products",
            Some("preview".to_string()),
            Some("editor".to_string()),
        );
        assert!(prompt.contains("Always answer in English."));
        assert!(prompt.contains("Never infer total row counts"));
        assert!(prompt.contains("Use this active editor context"));
    }

    #[test]
    fn sql_prompt_warns_about_preview_only_rows() {
        let prompt = build_sql_generation_prompt(
            "SQLite",
            "Count products",
            Some("preview".to_string()),
            Some("editor".to_string()),
        );
        assert!(prompt.contains("Snapshot rows are previews only."));
        assert!(prompt.contains("Never infer total row counts"));
        assert!(prompt.contains("generate SQL that verifies"));
    }

    #[test]
    fn active_tab_context_includes_sql_status_and_result_preview() {
        let tab = QueryTabState {
            id: 1,
            session_id: 1,
            title: "Query 1".to_string(),
            sql: "select * from products limit 100;".to_string(),
            status: "Loaded rows 1-10 from products".to_string(),
            result: Some(QueryOutput::Table(QueryPage {
                columns: vec!["id".to_string(), "name".to_string()],
                rows: (1..=MAX_ACTIVE_RESULT_ROWS as u64)
                    .map(|id| vec![id.to_string(), format!("Product {id}")])
                    .collect(),
                editable: None,
                offset: 0,
                page_size: 100,
                has_previous: false,
                has_next: true,
            })),
            current_offset: 0,
            page_size: 100,
            last_run_sql: None,
            preview_source: None,
            filter: None,
            sort: None,
            tab_kind: WorkspaceTabKind::Query,
            is_loading_more: false,
            pending_table_changes: PendingTableChanges::default(),
        };

        let context = build_active_tab_context(&tab).expect("expected active tab context");
        assert!(context.contains("Active editor SQL"));
        assert!(context.contains("Loaded rows 1-10 from products"));
        assert!(context.contains("More rows exist beyond this preview."));
        assert!(context.contains("result row: id=1, name=Product 1"));
    }
}

pub(super) fn active_editor_connection(
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
) -> Option<models::DatabaseConnection> {
    let session_id = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id)
        .map(|tab| tab.session_id)?;
    session_connection(session_id)
}
