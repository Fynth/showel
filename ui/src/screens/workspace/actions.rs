use crate::app_state::{activate_session, session_connection};
use dioxus::prelude::*;
use models::{
    DatabaseConnection, PendingTableChanges, QueryFilter, QueryFilterMode, QueryHistoryItem,
    QueryOutput, QuerySort, QueryTabState, TablePreviewSource,
};

pub const DEFAULT_PAGE_SIZE: u32 = 100;
type QueryHistorySignals = (Signal<Vec<QueryHistoryItem>>, Signal<u64>, String, String);

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
        filter: None,
        sort: None,
        pending_table_changes: PendingTableChanges::default(),
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
        .find(|tab| tab.session_id == session_id)
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
            tab.sql = sql;
            tab.status = status.clone();
            tab.result = None;
            tab.current_offset = 0;
            tab.last_run_sql = None;
            tab.preview_source = None;
            tab.filter = None;
            tab.sort = None;
            tab.pending_table_changes = PendingTableChanges::default();
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

pub fn replace_active_tab_sql(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: u64,
    sql: String,
    status: String,
) {
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == active_tab_id) {
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
    let comment = format!("-- Structure for {}", source.qualified_name);

    tabs.with_mut(|all_tabs| {
        let mut tab = new_query_tab(tab_id, session_id, title, comment);
        tab.status = format!("Loading structure for {}...", source.table_name);
        all_tabs.push(tab);
    });
    active_tab_id.set(tab_id);

    spawn(async move {
        match services::describe_table(connection, source.schema.clone(), source.table_name.clone())
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
                        tab.pending_table_changes = PendingTableChanges::default();
                    }
                });
            }
            Err(err) => {
                tabs.with_mut(|all_tabs| {
                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == tab_id) {
                        tab.result = None;
                        tab.status = format!("Structure error: {err:?}");
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
            tab.pending_table_changes = PendingTableChanges::default();
        }
    });

    spawn(async move {
        match services::execute_query_page(connection, sql.clone(), page_size, offset, filter, sort)
            .await
        {
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
                        tab.pending_table_changes = PendingTableChanges::default();
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
                        tab.pending_table_changes = PendingTableChanges::default();
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
                tab.pending_table_changes = PendingTableChanges::default();
            }
            tab.preview_source = Some(source.clone());
        }
    });

    let preview_sql = format!(
        "select * from {} limit {};",
        source.qualified_name, page_size
    );

    spawn(async move {
        match services::load_table_preview_page(
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
