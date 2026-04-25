use crate::app_state::{APP_READ_ONLY_MODE, APP_UI_SETTINGS, activate_session, session_connection};
use dioxus::prelude::*;
use models::{
    DatabaseConnection, PendingTableChanges, QueryFilter, QueryFilterMode, QueryHistoryItem,
    QueryOutput, QuerySort, QueryTabState, TablePreviewSource, WorkspaceTabKind,
};
use std::time::Instant;

fn redact_sql(sql: &str) -> String {
    let lower = sql.to_lowercase();
    if lower.contains("password") || lower.contains("secret") || lower.contains("token") {
        let mut result = sql.to_string();
        for sensitive in ["password", "secret", "token"] {
            if lower.contains(sensitive) {
                result = result
                    .lines()
                    .map(|line| {
                        let line_lower = line.to_lowercase();
                        if line_lower.contains(sensitive) {
                            if let Some(eq_pos) = line.find('=') {
                                let (before, _) = line.split_at(eq_pos + 1);
                                format!("{} [REDACTED]", before.trim_end())
                            } else {
                                line.to_string()
                            }
                        } else {
                            line.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
        result
    } else {
        sql.to_string()
    }
}

fn get_connection_type(connection: &DatabaseConnection) -> String {
    match connection {
        DatabaseConnection::Sqlite(_) => "sqlite".to_string(),
        DatabaseConnection::Postgres(_) => "postgres".to_string(),
        DatabaseConnection::MySql(_) => "mysql".to_string(),
        DatabaseConnection::ClickHouse(_) => "clickhouse".to_string(),
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

type QueryHistorySignals = (Signal<Vec<QueryHistoryItem>>, Signal<u64>, String, String);

pub fn read_only_mode_enabled() -> bool {
    APP_READ_ONLY_MODE()
}

pub fn read_only_mode_blocks_sql(sql: &str) -> bool {
    read_only_mode_enabled() && !query::is_read_only_sql(sql)
}

pub fn read_only_mode_block_status(action: &str) -> String {
    format!("Read-only mode blocked {action}. Disable read-only mode in Settings to allow writes.")
}

pub fn new_query_tab(id: u64, session_id: u64, title: String, sql: String) -> QueryTabState {
    QueryTabState {
        id,
        session_id,
        title,
        sql,
        status: "Ready".to_string(),
        result: None,
        current_offset: 0,
        page_size: APP_UI_SETTINGS().default_page_size,
        last_run_sql: None,
        preview_source: None,
        filter: None,
        sort: None,
        tab_kind: WorkspaceTabKind::Query,
        is_loading_more: false,
        pending_table_changes: PendingTableChanges::default(),
        execution_plan: None,
        show_execution_plan: false,
    }
}

pub fn ensure_tab_for_session(
    mut tabs: Signal<Vec<QueryTabState>>,
    mut active_tab_id: Signal<u64>,
    mut next_tab_id: Signal<u64>,
    session_id: u64,
) -> u64 {
    activate_session(session_id);

    if let Some(existing_tab_id) = tabs
        .read()
        .iter()
        .find(|tab| tab.session_id == session_id && tab.tab_kind == WorkspaceTabKind::Query)
        .map(|tab| tab.id)
    {
        active_tab_id.set(existing_tab_id);
        return existing_tab_id;
    }

    let tab_id = next_tab_id();
    next_tab_id += 1;
    tabs.with_mut(|all_tabs| {
        all_tabs.push(new_query_tab(
            tab_id,
            session_id,
            format!("Query {tab_id}"),
            "select 1 as id;".to_string(),
        ));
    });
    active_tab_id.set(tab_id);
    tab_id
}

pub fn update_active_tab_sql(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: String,
    status: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
            if tab.sql != sql {
                tab.show_execution_plan = false;
            }
            tab.sql = sql;
            tab.status = status.clone();
            tab.result = None;
            tab.current_offset = 0;
            tab.last_run_sql = None;
            tab.preview_source = None;
            tab.filter = None;
            tab.sort = None;
            tab.tab_kind = WorkspaceTabKind::Query;
            tab.is_loading_more = false;
            tab.pending_table_changes = PendingTableChanges::default();
        }
    });
}

fn sync_tab_sql_draft(tab: &mut QueryTabState, sql: &str) {
    if tab.sql == sql {
        return;
    }

    tab.sql = sql.to_string();
    tab.show_execution_plan = false;
}

pub fn sync_active_tab_sql_draft(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
            sync_tab_sql_draft(tab, &sql);
        }
    });
}

pub fn set_active_tab_sql(
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: String,
    status: String,
) {
    update_active_tab_sql(tabs, active_tab_id, sql, status);
}

pub fn append_to_tab_sql(
    mut tabs: Signal<Vec<QueryTabState>>,
    tab_id: u64,
    sql_fragment: String,
    status: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == tab_id) {
            if tab.sql.trim().is_empty() {
                tab.sql = sql_fragment;
            } else if sql_fragment.trim().is_empty() {
                return;
            } else if tab.sql.ends_with('\n') {
                tab.sql.push_str(&sql_fragment);
            } else {
                tab.sql.push_str("\n\n");
                tab.sql.push_str(&sql_fragment);
            }

            tab.status = status.clone();
            tab.result = None;
            tab.current_offset = 0;
            tab.last_run_sql = None;
            tab.preview_source = None;
            tab.filter = None;
            tab.sort = None;
            tab.tab_kind = WorkspaceTabKind::Query;
            tab.is_loading_more = false;
            tab.pending_table_changes = PendingTableChanges::default();
        }
    });
}

pub fn set_active_tab_status(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    status: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
            tab.status = status.clone();
        }
    });
}

fn toggle_cached_execution_plan(tab: &mut QueryTabState, sql: &str) -> bool {
    if tab.show_execution_plan && tab.execution_plan.is_some() {
        tab.show_execution_plan = false;
        return true;
    }

    let normalized_sql = sql.trim();
    let can_reopen_cached_plan = tab.execution_plan.as_ref().is_some_and(|plan| {
        !normalized_sql.is_empty() && plan.explained_sql.trim() == normalized_sql
    });
    if can_reopen_cached_plan {
        tab.show_execution_plan = true;
        return true;
    }

    false
}

pub fn toggle_execution_plan_for_tab(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: &str,
) -> bool {
    let mut handled = false;
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
            handled = toggle_cached_execution_plan(tab, sql);
        }
    });
    handled
}

pub fn replace_active_tab_sql(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: String,
    status: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
            if tab.sql != sql {
                tab.show_execution_plan = false;
            }
            tab.sql = sql;
            tab.status = status.clone();
        }
    });
}

pub fn open_structure_tab(
    mut tabs: Signal<Vec<QueryTabState>>,
    mut active_tab_id: Signal<u64>,
    mut next_tab_id: Signal<u64>,
    session_id: u64,
    connection: DatabaseConnection,
    source: TablePreviewSource,
) {
    let tab_id = next_tab_id();
    next_tab_id += 1;

    let title = format!("Structure · {}", source.table_name);

    tabs.with_mut(|all_tabs| {
        let mut tab = new_query_tab(tab_id, session_id, title, String::new());
        tab.tab_kind = WorkspaceTabKind::Structure;
        tab.status = format!("Loading structure for {}...", source.table_name);
        all_tabs.push(tab);
    });
    active_tab_id.set(tab_id);

    spawn(async move {
        match explorer::describe_table(connection, source.schema.clone(), source.table_name.clone())
            .await
        {
            Ok(output) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        tab.result = Some(output);
                        tab.status = format!("Loaded structure for {}", source.table_name);
                        tab.current_offset = 0;
                        tab.last_run_sql = None;
                        tab.preview_source = None;
                        tab.filter = None;
                        tab.sort = None;
                        tab.is_loading_more = false;
                        tab.pending_table_changes = PendingTableChanges::default();
                    }
                });
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        tab.result = None;
                        tab.status = format!("Structure error: {err}");
                    }
                });
            }
        }
    });
}

pub fn tab_connection_or_error(
    tabs: Signal<Vec<QueryTabState>>,
    tab_id: u64,
    session_id: u64,
) -> Option<DatabaseConnection> {
    match session_connection(session_id) {
        Some(connection) => Some(connection),
        None => {
            set_active_tab_status(tabs, tab_id, "The bound connection was closed".to_string());
            None
        }
    }
}

pub fn run_query_for_tab(
    mut tabs: Signal<Vec<QueryTabState>>,
    current_id: u64,
    connection: DatabaseConnection,
    sql: String,
    offset: u64,
    page_size: u32,
    history: Option<QueryHistorySignals>,
) {
    if read_only_mode_blocks_sql(&sql) {
        set_active_tab_status(tabs, current_id, read_only_mode_block_status("write SQL"));
        return;
    }

    let filter = tabs
        .read()
        .iter()
        .find(|tab| tab.id == current_id)
        .and_then(|tab| tab.filter.clone());
    let sort = tabs
        .read()
        .iter()
        .find(|tab| tab.id == current_id)
        .and_then(|tab| tab.sort.clone());

    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.status = format!("Running query at offset {offset}...");
            tab.preview_source = None;
            tab.is_loading_more = false;
            tab.pending_table_changes = PendingTableChanges::default();
            tab.show_execution_plan = false;
        }
    });

    let connection_type = get_connection_type(&connection);

    spawn(async move {
        let start_time = Instant::now();
        match query::execute_query_page(connection, sql.clone(), page_size, offset, filter, sort)
            .await
        {
            Ok(output) => {
                let (status, current_offset) = match &output {
                    QueryOutput::Table(page) => (
                        format_loaded_rows_status(page.offset, page.rows.len()),
                        page.offset,
                    ),
                    QueryOutput::AffectedRows(rows) => (format!("Rows affected: {rows}"), 0),
                };
                let rows_returned = match &output {
                    QueryOutput::Table(page) => Some(page.rows.len()),
                    QueryOutput::AffectedRows(count) => Some(*count as usize),
                };

                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = Some(output);
                        tab.status = status.clone();
                        tab.current_offset = current_offset;
                        tab.page_size = page_size;
                        tab.last_run_sql = Some(sql.clone());
                        tab.preview_source = None;
                        tab.is_loading_more = false;
                        tab.pending_table_changes = PendingTableChanges::default();
                    }
                });

                if let Some((mut history, mut next_history_id, tab_title, connection_name)) =
                    history
                {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let history_id = next_history_id();
                    next_history_id += 1;
                    let history_item = QueryHistoryItem {
                        id: history_id,
                        tab_title,
                        connection_name,
                        sql: redact_sql(&sql),
                        duration_ms,
                        rows_returned,
                        executed_at: unix_timestamp(),
                        connection_type: connection_type.clone(),
                        outcome: "Success".to_string(),
                        error_message: None,
                    };
                    history.with_mut(|items| {
                        items.insert(0, history_item.clone());
                        if items.len() > 20 {
                            items.truncate(20);
                        }
                    });
                    let _ = storage::append_query_history(history_item).await;
                }
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = None;
                        tab.status = format!("Error: {err}");
                        tab.preview_source = None;
                        tab.is_loading_more = false;
                        tab.pending_table_changes = PendingTableChanges::default();
                    }
                });

                if let Some((mut history, mut next_history_id, tab_title, connection_name)) =
                    history
                {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let history_id = next_history_id();
                    next_history_id += 1;
                    let history_item = QueryHistoryItem {
                        id: history_id,
                        tab_title,
                        connection_name,
                        sql: redact_sql(&sql),
                        duration_ms,
                        rows_returned: None,
                        executed_at: unix_timestamp(),
                        connection_type: connection_type.clone(),
                        outcome: format!("Error: {err}"),
                        error_message: Some(err.to_string()),
                    };
                    history.with_mut(|items| {
                        items.insert(0, history_item.clone());
                        if items.len() > 20 {
                            items.truncate(20);
                        }
                    });
                    let _ = storage::append_query_history(history_item).await;
                }
            }
        }
    });
}

pub fn run_explain_for_tab(
    mut tabs: Signal<Vec<QueryTabState>>,
    current_id: u64,
    connection: DatabaseConnection,
    sql: String,
) {
    if sql.trim().is_empty() {
        tabs.with_mut(|all_tabs| {
            if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                tab.status = "Query is empty".to_string();
            }
        });
        return;
    }

    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.status = "Running EXPLAIN...".to_string();
            tab.execution_plan = None;
        }
    });

    spawn(async move {
        match query::execute_explain(connection, &sql, false).await {
            Ok(plan) => {
                let node_count = plan.flattened_with_depth().len();
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.execution_plan = Some(plan);
                        tab.show_execution_plan = true;
                        tab.status = format!("Execution plan loaded ({} operations)", node_count);
                    }
                });
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.status = format!("EXPLAIN error: {err}");
                    }
                });
            }
        }
    });
}

pub fn run_table_preview_for_tab(
    mut tabs: Signal<Vec<QueryTabState>>,
    current_id: u64,
    connection: DatabaseConnection,
    source: TablePreviewSource,
    offset: u64,
    page_size: u32,
) {
    let filter = tabs
        .read()
        .iter()
        .find(|tab| tab.id == current_id)
        .and_then(|tab| {
            if tab.preview_source.as_ref() == Some(&source) {
                tab.filter.clone()
            } else {
                None
            }
        });
    let sort = tabs
        .read()
        .iter()
        .find(|tab| tab.id == current_id)
        .and_then(|tab| {
            if tab.preview_source.as_ref() == Some(&source) {
                tab.sort.clone()
            } else {
                None
            }
        });

    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.status = format!("Loading rows from {}...", source.table_name);
            if tab.preview_source.as_ref() != Some(&source) {
                tab.filter = None;
                tab.sort = None;
                tab.is_loading_more = false;
                tab.pending_table_changes = PendingTableChanges::default();
            }
            tab.preview_source = Some(source.clone());
        }
    });

    spawn(async move {
        match query::load_table_preview_page(
            connection,
            source.clone(),
            page_size,
            offset,
            filter,
            sort,
        )
        .await
        {
            Ok(output) => {
                let status = match &output {
                    QueryOutput::Table(page) => format_loaded_rows_from_source_status(
                        page.offset,
                        page.rows.len(),
                        &source.table_name,
                    ),
                    QueryOutput::AffectedRows(rows) => format!("Rows affected: {rows}"),
                };

                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = Some(output);
                        tab.status = status;
                        tab.current_offset = offset;
                        tab.page_size = page_size;
                        tab.last_run_sql = Some(format!(
                            "select * from {} limit {};",
                            source.qualified_name, page_size
                        ));
                        tab.preview_source = Some(source.clone());
                        tab.is_loading_more = false;
                    }
                });
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = None;
                        tab.status = format!("Preview error: {err}");
                        tab.preview_source = Some(source.clone());
                        tab.is_loading_more = false;
                    }
                });
            }
        }
    });
}

/// Maximum number of rows that can accumulate via infinite-scroll append.
/// Beyond this cap the user must use explicit pagination (Previous/Next) instead.
const MAX_ACCUMULATED_ROWS: usize = 10_000;

fn append_query_page(existing_page: &mut models::QueryPage, next_page: models::QueryPage) {
    let next_editable = next_page.editable;

    existing_page.rows.extend(next_page.rows);
    existing_page.has_next = next_page.has_next;
    existing_page.has_previous = existing_page.has_previous || next_page.has_previous;

    existing_page.editable = match (existing_page.editable.take(), next_editable) {
        (Some(mut existing_editable), Some(next_editable))
            if existing_editable.source == next_editable.source =>
        {
            existing_editable
                .row_locators
                .extend(next_editable.row_locators);
            Some(existing_editable)
        }
        (None, None) => None,
        _ => None,
    };

    // Cap accumulated rows to prevent unbounded memory growth and DOM freeze.
    if existing_page.rows.len() > MAX_ACCUMULATED_ROWS {
        let excess = existing_page.rows.len() - MAX_ACCUMULATED_ROWS;
        existing_page.rows.drain(..excess);
        existing_page.offset += excess as u64;
        if let Some(editable) = existing_page.editable.as_mut() {
            if editable.row_locators.len() >= excess {
                editable.row_locators.drain(..excess);
            } else {
                existing_page.editable = None;
            }
        }
    }

    if existing_page
        .editable
        .as_ref()
        .is_some_and(|editable| editable.row_locators.len() != existing_page.rows.len())
    {
        existing_page.editable = None;
    }
}

pub fn append_next_tab_page(mut tabs: Signal<Vec<QueryTabState>>, current_tab: QueryTabState) {
    let Some(QueryOutput::Table(current_page)) = current_tab.result.clone() else {
        return;
    };

    if current_tab.is_loading_more || !current_tab.pending_table_changes.is_empty() {
        return;
    }

    if !current_page.has_next {
        return;
    }

    let next_offset = current_page.offset + current_page.rows.len() as u64;
    let expected_sql = current_tab.last_run_sql.clone();
    let expected_preview_source = current_tab.preview_source.clone();
    let expected_filter = current_tab.filter.clone();
    let expected_sort = current_tab.sort.clone();

    let Some(connection) = tab_connection_or_error(tabs, current_tab.id, current_tab.session_id)
    else {
        return;
    };

    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_tab.id) {
            tab.is_loading_more = true;
            tab.status = format!("Loading more rows from {}...", next_offset + 1);
        }
    });

    spawn(async move {
        let next_page_result = if let Some(source) = expected_preview_source.clone() {
            query::load_table_preview_page(
                connection,
                source,
                current_tab.page_size,
                next_offset,
                expected_filter.clone(),
                expected_sort.clone(),
            )
            .await
        } else if let Some(sql) = expected_sql.clone() {
            query::execute_query_page(
                connection,
                sql,
                current_tab.page_size,
                next_offset,
                expected_filter.clone(),
                expected_sort.clone(),
            )
            .await
        } else {
            tabs.with_mut(|all_tabs| {
                if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_tab.id) {
                    tab.is_loading_more = false;
                }
            });
            return;
        };

        match next_page_result {
            Ok(QueryOutput::Table(next_page)) => {
                tabs.with_mut(|all_tabs| {
                    let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_tab.id) else {
                        return;
                    };

                    let same_request = tab.last_run_sql == expected_sql
                        && tab.preview_source == expected_preview_source
                        && tab.filter == expected_filter
                        && tab.sort == expected_sort;

                    if !same_request {
                        tab.is_loading_more = false;
                        return;
                    }

                    let mut loaded_range = None;
                    if let Some(QueryOutput::Table(existing_page)) = tab.result.as_mut() {
                        append_query_page(existing_page, next_page);
                        loaded_range = Some((
                            existing_page.offset,
                            existing_page.offset + existing_page.rows.len() as u64,
                        ));
                    }

                    if let Some((offset, last_row)) = loaded_range {
                        tab.current_offset = offset;
                        tab.status = format_loaded_rows_status(
                            offset,
                            last_row.saturating_sub(offset) as usize,
                        );
                    }

                    tab.is_loading_more = false;
                });
            }
            Ok(other_output) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_tab.id) {
                        tab.result = Some(other_output);
                        tab.is_loading_more = false;
                        tab.status = "Loaded additional result".to_string();
                    }
                });
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_tab.id) {
                        tab.is_loading_more = false;
                        tab.status = format!("Load more error: {err}");
                    }
                });
            }
        }
    });
}

fn loaded_rows_range(offset: u64, row_count: usize) -> Option<(u64, u64)> {
    if row_count == 0 {
        None
    } else {
        Some((offset + 1, offset + row_count as u64))
    }
}

fn format_loaded_rows_status(offset: u64, row_count: usize) -> String {
    match loaded_rows_range(offset, row_count) {
        Some((start, end)) => format!("Loaded rows {start}-{end}"),
        None => "Loaded 0 rows".to_string(),
    }
}

fn format_loaded_rows_from_source_status(
    offset: u64,
    row_count: usize,
    source_name: &str,
) -> String {
    match loaded_rows_range(offset, row_count) {
        Some((start, end)) => format!("Loaded rows {start}-{end} from {source_name}"),
        None => format!("Loaded 0 rows from {source_name}"),
    }
}

pub(crate) fn rows_toolbar_summary(offset: u64, row_count: usize, page_size: u32) -> String {
    match loaded_rows_range(offset, row_count) {
        Some((start, end)) => format!("Rows {start}-{end} · page size {page_size}"),
        None => format!("0 rows · page size {page_size}"),
    }
}

pub fn load_tab_page(tabs: Signal<Vec<QueryTabState>>, current_tab: QueryTabState, offset: u64) {
    let Some(connection) = tab_connection_or_error(tabs, current_tab.id, current_tab.session_id)
    else {
        return;
    };

    if let Some(source) = current_tab.preview_source.clone() {
        run_table_preview_for_tab(
            tabs,
            current_tab.id,
            connection,
            source,
            offset,
            current_tab.page_size,
        );
        return;
    }

    if let Some(sql) = current_tab.last_run_sql.clone() {
        run_query_for_tab(
            tabs,
            current_tab.id,
            connection,
            sql,
            offset,
            current_tab.page_size,
            None,
        );
    }
}

pub fn refresh_tab_result(
    tabs: Signal<Vec<QueryTabState>>,
    current_tab: QueryTabState,
    fallback_source: Option<TablePreviewSource>,
) {
    if current_tab.preview_source.is_some() || current_tab.last_run_sql.is_some() {
        load_tab_page(tabs, current_tab.clone(), current_tab.current_offset);
        return;
    }

    let Some(connection) = tab_connection_or_error(tabs, current_tab.id, current_tab.session_id)
    else {
        return;
    };

    if let Some(source) = fallback_source {
        run_table_preview_for_tab(
            tabs,
            current_tab.id,
            connection,
            source,
            current_tab.current_offset,
            current_tab.page_size,
        );
    }
}

pub fn mark_table_deleted(
    mut tabs: Signal<Vec<QueryTabState>>,
    session_id: u64,
    source: TablePreviewSource,
) {
    tabs.with_mut(|all_tabs| {
        for tab in all_tabs
            .iter_mut()
            .filter(|tab| tab.session_id == session_id)
        {
            let matches_preview = tab.preview_source.as_ref() == Some(&source);
            let matches_sql = tab
                .last_run_sql
                .as_deref()
                .and_then(query::preview_source_for_sql)
                .as_ref()
                == Some(&source);

            if !matches_preview && !matches_sql {
                continue;
            }

            tab.result = None;
            tab.current_offset = 0;
            tab.preview_source = None;
            tab.filter = None;
            tab.sort = None;
            tab.is_loading_more = false;
            tab.pending_table_changes = PendingTableChanges::default();
            tab.status = if matches_preview {
                format!("Table {} was deleted", source.table_name)
            } else {
                format!(
                    "Referenced table {} was deleted. Update the SQL and run it again.",
                    source.table_name
                )
            };

            if matches_preview {
                tab.last_run_sql = None;
            }
        }
    });
}

pub fn mark_table_truncated(
    mut tabs: Signal<Vec<QueryTabState>>,
    session_id: u64,
    connection: DatabaseConnection,
    source: TablePreviewSource,
) {
    let mut preview_tabs = Vec::new();

    tabs.with_mut(|all_tabs| {
        for tab in all_tabs
            .iter_mut()
            .filter(|tab| tab.session_id == session_id)
        {
            let matches_preview = tab.preview_source.as_ref() == Some(&source);
            let matches_sql = tab
                .last_run_sql
                .as_deref()
                .and_then(query::preview_source_for_sql)
                .as_ref()
                == Some(&source);

            if !matches_preview && !matches_sql {
                continue;
            }

            tab.result = None;
            tab.current_offset = 0;
            tab.is_loading_more = false;
            tab.pending_table_changes = PendingTableChanges::default();

            if matches_preview {
                preview_tabs.push((tab.id, tab.page_size));
                continue;
            }

            tab.filter = None;
            tab.sort = None;
            tab.status = format!(
                "Referenced table {} was truncated. Run the SQL again to refresh.",
                source.table_name
            );
        }
    });

    for (tab_id, page_size) in preview_tabs {
        run_table_preview_for_tab(
            tabs,
            tab_id,
            connection.clone(),
            source.clone(),
            0,
            page_size,
        );
    }
}

pub fn toggle_active_tab_sort(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    column_name: String,
) {
    let mut tab_to_reload = None;

    tabs.with_mut(|all_tabs| {
        let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) else {
            return;
        };

        tab.sort = next_sort_state(tab.sort.as_ref(), &column_name);
        tab.current_offset = 0;
        tab.status = match &tab.sort {
            Some(sort) => format!(
                "Sorted by {} {}",
                sort.column_name,
                if sort.descending { "DESC" } else { "ASC" }
            ),
            None => "Sorting cleared".to_string(),
        };
        tab_to_reload = Some(tab.clone());
    });

    if let Some(tab) = tab_to_reload
        && (tab.last_run_sql.is_some() || tab.preview_source.is_some())
    {
        load_tab_page(tabs, tab, 0);
    }
}

fn next_sort_state(current: Option<&QuerySort>, column_name: &str) -> Option<QuerySort> {
    match current {
        Some(sort) if sort.column_name == column_name && !sort.descending => Some(QuerySort {
            column_name: column_name.to_string(),
            descending: true,
        }),
        Some(sort) if sort.column_name == column_name && sort.descending => None,
        _ => Some(QuerySort {
            column_name: column_name.to_string(),
            descending: false,
        }),
    }
}

pub fn apply_active_tab_filter(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    filter: QueryFilter,
) {
    let mut tab_to_reload = None;

    tabs.with_mut(|all_tabs| {
        let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) else {
            return;
        };

        let applied_rules = filter
            .rules
            .iter()
            .filter(|rule| {
                !rule.column_name.trim().is_empty()
                    && (!rule.value.trim().is_empty() || rule.operator.is_nullary())
            })
            .cloned()
            .collect::<Vec<_>>();

        tab.filter = if applied_rules.is_empty() {
            None
        } else {
            Some(QueryFilter {
                mode: filter.mode,
                rules: applied_rules,
            })
        };
        tab.current_offset = 0;
        tab.status = match &tab.filter {
            Some(filter) => format!(
                "Applied {} filter rule(s) with {}",
                filter.rules.len(),
                match filter.mode {
                    QueryFilterMode::And => "AND",
                    QueryFilterMode::Or => "OR",
                }
            ),
            None => "Filter cleared".to_string(),
        };
        tab_to_reload = Some(tab.clone());
    });

    if let Some(tab) = tab_to_reload
        && (tab.last_run_sql.is_some() || tab.preview_source.is_some())
    {
        load_tab_page(tabs, tab, 0);
    }
}

pub fn clear_active_tab_filter(mut tabs: Signal<Vec<QueryTabState>>, active_tab_id: u64) {
    let mut tab_to_reload = None;

    tabs.with_mut(|all_tabs| {
        let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) else {
            return;
        };

        tab.filter = None;
        tab.current_offset = 0;
        tab.status = "Filter cleared".to_string();
        tab_to_reload = Some(tab.clone());
    });

    if let Some(tab) = tab_to_reload
        && (tab.last_run_sql.is_some() || tab.preview_source.is_some())
    {
        load_tab_page(tabs, tab, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        append_query_page, format_loaded_rows_from_source_status, format_loaded_rows_status,
        redact_sql, rows_toolbar_summary, sync_tab_sql_draft, toggle_cached_execution_plan,
    };
    use models::{
        EditableTableContext, ExecutionPlan, PendingTableChanges, QueryPage, QueryTabState,
        TablePreviewSource, WorkspaceTabKind,
    };

    fn query_tab(sql: &str) -> QueryTabState {
        QueryTabState {
            id: 1,
            session_id: 7,
            title: "Query 1".to_string(),
            sql: sql.to_string(),
            status: "Ready".to_string(),
            result: None,
            current_offset: 0,
            page_size: 100,
            last_run_sql: None,
            preview_source: None,
            filter: None,
            sort: None,
            tab_kind: WorkspaceTabKind::Query,
            is_loading_more: false,
            pending_table_changes: PendingTableChanges::default(),
            execution_plan: None,
            show_execution_plan: false,
        }
    }

    fn test_source() -> TablePreviewSource {
        TablePreviewSource {
            schema: None,
            table_name: "products".to_string(),
            qualified_name: "products".to_string(),
        }
    }

    fn query_page(offset: u64, row_count: usize, has_next: bool) -> QueryPage {
        let rows = (0..row_count)
            .map(|index| vec![(offset + index as u64).to_string()])
            .collect::<Vec<_>>();
        let row_locators = (0..row_count)
            .map(|index| format!("row-{}", offset + index as u64))
            .collect::<Vec<_>>();

        QueryPage {
            columns: vec!["id".to_string()],
            rows,
            editable: Some(EditableTableContext {
                source: test_source(),
                row_locators,
            }),
            offset,
            page_size: row_count as u32,
            has_previous: offset > 0,
            has_next,
        }
    }

    #[test]
    fn formats_empty_result_status_without_invalid_range() {
        assert_eq!(format_loaded_rows_status(0, 0), "Loaded 0 rows");
        assert_eq!(
            format_loaded_rows_from_source_status(0, 0, "products"),
            "Loaded 0 rows from products"
        );
    }

    #[test]
    fn formats_empty_result_toolbar_summary_without_invalid_range() {
        assert_eq!(rows_toolbar_summary(0, 0, 100), "0 rows · page size 100");
    }

    #[test]
    fn second_explain_click_hides_visible_execution_plan() {
        let mut tab = query_tab("select 1");
        tab.execution_plan = Some(ExecutionPlan::new("select 1"));
        tab.show_execution_plan = true;

        assert!(toggle_cached_execution_plan(&mut tab, "select 1"));
        assert!(!tab.show_execution_plan);
    }

    #[test]
    fn explain_click_reopens_cached_plan_for_same_sql() {
        let mut tab = query_tab("select 1");
        tab.execution_plan = Some(ExecutionPlan::new("select 1"));
        tab.show_execution_plan = false;

        assert!(toggle_cached_execution_plan(&mut tab, "select 1"));
        assert!(tab.show_execution_plan);
    }

    #[test]
    fn explain_click_does_not_reopen_cached_plan_for_different_sql() {
        let mut tab = query_tab("select 1");
        tab.execution_plan = Some(ExecutionPlan::new("select 1"));
        tab.show_execution_plan = false;

        assert!(!toggle_cached_execution_plan(&mut tab, "select 2"));
        assert!(!tab.show_execution_plan);
    }

    #[test]
    fn syncing_editor_draft_updates_sql_and_hides_plan_without_resetting_result_state() {
        let mut tab = query_tab("select 1");
        tab.execution_plan = Some(ExecutionPlan::new("select 1"));
        tab.show_execution_plan = true;
        tab.status = "Loaded 1 rows".to_string();

        sync_tab_sql_draft(&mut tab, "select 2");

        assert_eq!(tab.sql, "select 2");
        assert!(!tab.show_execution_plan);
        assert_eq!(tab.status, "Loaded 1 rows");
    }

    #[test]
    fn append_query_page_caps_rows_and_keeps_edit_locators_aligned() {
        let mut existing = query_page(0, 100, true);
        let next = query_page(100, 11_000, false);

        append_query_page(&mut existing, next);

        assert_eq!(existing.rows.len(), 10_000);
        assert_eq!(existing.offset, 1_100);
        assert_eq!(existing.rows.first().unwrap()[0], "1100");
        assert_eq!(existing.rows.last().unwrap()[0], "11099");
        assert_eq!(
            existing.editable.as_ref().unwrap().row_locators.len(),
            10_000
        );
        assert_eq!(
            existing.editable.as_ref().unwrap().row_locators.first(),
            Some(&"row-1100".to_string())
        );
        assert!(!existing.has_next);
    }

    #[test]
    fn redacts_unquoted_secret_values_without_leaking_prefix() {
        let sql = "set password=abc123;\nselect 1;";

        let redacted = redact_sql(sql);

        assert_eq!(redacted, "set password= [REDACTED]\nselect 1;");
        assert!(!redacted.contains("abc123"));
    }

    #[test]
    fn redacts_quoted_secret_values_without_unwrapping_quote() {
        let sql = "alter user app with password = 'abc123';";

        let redacted = redact_sql(sql);

        assert_eq!(redacted, "alter user app with password = [REDACTED]");
        assert!(!redacted.contains("abc123"));
    }
}
