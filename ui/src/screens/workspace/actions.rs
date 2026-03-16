use crate::app_state::session_connection;
use dioxus::prelude::*;
use models::{
    DatabaseConnection, QueryHistoryItem, QueryOutput, QueryTabState, TablePreviewSource,
};

pub const DEFAULT_PAGE_SIZE: u32 = 100;

pub fn new_query_tab(id: u64, session_id: u64, title: String, sql: String) -> QueryTabState {
    QueryTabState {
        id,
        session_id,
        title,
        sql,
        status: "Ready".to_string(),
        result: None,
        current_offset: 0,
        page_size: DEFAULT_PAGE_SIZE,
        last_run_sql: None,
        preview_source: None,
    }
}

pub fn update_active_tab_sql(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: String,
    status: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
            tab.sql = sql;
            tab.status = status.clone();
            tab.result = None;
            tab.current_offset = 0;
            tab.last_run_sql = None;
            tab.preview_source = None;
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
    history: Option<(Signal<Vec<QueryHistoryItem>>, Signal<u64>, String, String)>,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.status = format!("Running query at offset {offset}...");
            tab.preview_source = None;
        }
    });

    spawn(async move {
        match services::execute_query_page(connection, sql.clone(), page_size, offset).await {
            Ok(output) => {
                let (status, current_offset) = match &output {
                    QueryOutput::Table(page) => (
                        format!(
                            "Loaded rows {}-{}",
                            page.offset + 1,
                            page.offset + page.rows.len() as u64
                        ),
                        page.offset,
                    ),
                    QueryOutput::AffectedRows(rows) => (format!("Rows affected: {rows}"), 0),
                };

                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = Some(output);
                        tab.status = status.clone();
                        tab.current_offset = current_offset;
                        tab.page_size = page_size;
                        tab.last_run_sql = Some(sql.clone());
                        tab.preview_source = None;
                    }
                });

                if let Some((mut history, mut next_history_id, tab_title, connection_name)) =
                    history
                {
                    let history_id = next_history_id();
                    next_history_id += 1;
                    let history_item = QueryHistoryItem {
                        id: history_id,
                        tab_title,
                        connection_name,
                        sql: sql.clone(),
                        outcome: "Success".to_string(),
                    };
                    history.with_mut(|items| {
                        items.insert(0, history_item.clone());
                        if items.len() > 20 {
                            items.truncate(20);
                        }
                    });
                    let _ = services::append_query_history(history_item).await;
                }
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = None;
                        tab.status = format!("Error: {err:?}");
                        tab.preview_source = None;
                    }
                });

                if let Some((mut history, mut next_history_id, tab_title, connection_name)) =
                    history
                {
                    let history_id = next_history_id();
                    next_history_id += 1;
                    let history_item = QueryHistoryItem {
                        id: history_id,
                        tab_title,
                        connection_name,
                        sql,
                        outcome: format!("Error: {err:?}"),
                    };
                    history.with_mut(|items| {
                        items.insert(0, history_item.clone());
                        if items.len() > 20 {
                            items.truncate(20);
                        }
                    });
                    let _ = services::append_query_history(history_item).await;
                }
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
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.status = format!("Loading rows from {}...", source.table_name);
            tab.preview_source = Some(source.clone());
        }
    });

    let preview_sql = format!(
        "select * from {} limit {};",
        source.qualified_name, page_size
    );

    spawn(async move {
        match services::load_table_preview_page(connection, source.clone(), page_size, offset).await
        {
            Ok(output) => {
                let status = match &output {
                    QueryOutput::Table(page) => format!(
                        "Loaded rows {}-{} from {}",
                        page.offset + 1,
                        page.offset + page.rows.len() as u64,
                        source.table_name
                    ),
                    QueryOutput::AffectedRows(rows) => format!("Rows affected: {rows}"),
                };

                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.sql = preview_sql.clone();
                        tab.result = Some(output);
                        tab.status = status;
                        tab.current_offset = offset;
                        tab.page_size = page_size;
                        tab.last_run_sql = Some(preview_sql.clone());
                        tab.preview_source = Some(source.clone());
                    }
                });
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                        tab.result = None;
                        tab.status = format!("Preview error: {err:?}");
                        tab.preview_source = Some(source.clone());
                    }
                });
            }
        }
    });
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
