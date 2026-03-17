use dioxus::prelude::*;
use models::{AcpMessageKind, AcpPanelState, QueryTabState};

use crate::{app_state::session_connection, screens::workspace::actions::update_active_tab_sql};

use super::state::push_message;

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
) -> String {
    let mut prompt = format!(
        "You are generating SQL for the active database connection.\n\
Database context: {connection_label}\n"
    );
    if let Some(db_context) = db_context {
        prompt.push_str("Use this live database snapshot:\n");
        prompt.push_str(&db_context);
        prompt.push('\n');
    }
    prompt.push_str(
        "When creating tables, always define an auto-generated primary key `id`.\n\
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
) -> String {
    let mut message = format!(
        "You are helping with the active database connection.\n\
Database context: {connection_label}\n"
    );
    if let Some(db_context) = db_context {
        message.push_str("Use this live database snapshot when answering:\n");
        message.push_str(&db_context);
        message.push('\n');
    }
    message.push_str(
        "If you propose schema creation, always use an auto-generated primary key `id`.\n\
If you propose inserts, omit `id` unless the user explicitly asks for manual ids.\n",
    );
    message.push_str(&format!("User request: {prompt}"));
    message
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
