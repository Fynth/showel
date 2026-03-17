use models::{
    DatabaseConnection, DatabaseError, EditableTableContext, QueryOutput, QueryPage,
    TablePreviewSource,
};
use sqlx::{Column, Row};

const LOCATOR_COLUMN: &str = "__showel_locator";

#[derive(Clone)]
struct EditableSelectPlan {
    source: TablePreviewSource,
    select_list: String,
    tail: String,
}

pub async fn execute_query(
    connection: DatabaseConnection,
    sql: String,
) -> Result<QueryOutput, DatabaseError> {
    execute_query_page(connection, sql, 100, 0).await
}

pub async fn load_table_preview_page(
    connection: DatabaseConnection,
    source: TablePreviewSource,
    page_size: u32,
    offset: u64,
) -> Result<QueryOutput, DatabaseError> {
    let limit = page_size as u64 + 1;

    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let sql = format!(
                r#"select rowid as "{LOCATOR_COLUMN}", * from {} limit {limit} offset {offset}"#,
                source.qualified_name
            );
            let rows = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(QueryOutput::Table(sqlite_preview_rows_to_paginated_page(
                rows, source, page_size, offset,
            )))
        }
        DatabaseConnection::Postgres(pool) => {
            let sql = format!(
                r#"select ctid::text as "{LOCATOR_COLUMN}", * from {} limit {limit} offset {offset}"#,
                source.qualified_name
            );
            let rows = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(QueryOutput::Table(postgres_preview_rows_to_paginated_page(
                rows, source, page_size, offset,
            )))
        }
    }
}

pub async fn update_table_cell(
    connection: DatabaseConnection,
    source: TablePreviewSource,
    locator: String,
    column_name: String,
    value: String,
) -> Result<(), DatabaseError> {
    let column = quote_identifier(&column_name);
    let value_literal = sql_literal(&value);

    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let rowid = locator.parse::<i64>().map_err(|_| {
                DatabaseError::UnsupportedDriver("invalid SQLite row locator".to_string())
            })?;
            let sql = format!(
                "update {} set {} = {} where rowid = {}",
                source.qualified_name, column, value_literal, rowid
            );
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(())
        }
        DatabaseConnection::Postgres(pool) => {
            let sql = format!(
                "update {} set {} = {} where ctid = {}::tid",
                source.qualified_name,
                column,
                value_literal,
                sql_literal(&locator)
            );
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(())
        }
    }
}

pub async fn execute_query_page(
    connection: DatabaseConnection,
    sql: String,
    page_size: u32,
    offset: u64,
) -> Result<QueryOutput, DatabaseError> {
    let normalized = sql.trim().to_lowercase();

    match connection {
        DatabaseConnection::Sqlite(pool) => {
            if let Some(plan) = editable_select_plan(&sql) {
                let query = build_editable_paginated_query(&plan, page_size, offset, "rowid");
                let rows = sqlx::query(&query)
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::Sqlite)?;
                Ok(QueryOutput::Table(sqlite_preview_rows_to_paginated_page(
                    rows,
                    plan.source,
                    page_size,
                    offset,
                )))
            } else if is_paginated_query(&normalized) {
                let rows = sqlx::query(&build_paginated_query(&sql, page_size, offset))
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::Sqlite)?;
                Ok(QueryOutput::Table(sqlite_rows_to_paginated_page(
                    rows, page_size, offset,
                )))
            } else if is_tabular_query(&normalized) {
                let rows = sqlx::query(&sql)
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::Sqlite)?;
                Ok(QueryOutput::Table(sqlite_rows_to_page(rows)))
            } else {
                let result = sqlx::query(&sql)
                    .execute(&pool)
                    .await
                    .map_err(DatabaseError::Sqlite)?;
                Ok(QueryOutput::AffectedRows(result.rows_affected()))
            }
        }
        DatabaseConnection::Postgres(pool) => {
            if let Some(plan) = editable_select_plan(&sql) {
                let query = build_editable_paginated_query(&plan, page_size, offset, "ctid::text");
                let rows = sqlx::query(&query)
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::Postgres)?;
                Ok(QueryOutput::Table(postgres_preview_rows_to_paginated_page(
                    rows,
                    plan.source,
                    page_size,
                    offset,
                )))
            } else if is_paginated_query(&normalized) {
                let rows = sqlx::query(&build_paginated_query(&sql, page_size, offset))
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::Postgres)?;
                Ok(QueryOutput::Table(postgres_rows_to_paginated_page(
                    rows, page_size, offset,
                )))
            } else if is_tabular_query(&normalized) {
                let rows = sqlx::query(&sql)
                    .fetch_all(&pool)
                    .await
                    .map_err(DatabaseError::Postgres)?;
                Ok(QueryOutput::Table(postgres_rows_to_page(rows)))
            } else {
                let result = sqlx::query(&sql)
                    .execute(&pool)
                    .await
                    .map_err(DatabaseError::Postgres)?;
                Ok(QueryOutput::AffectedRows(result.rows_affected()))
            }
        }
    }
}

fn is_tabular_query(sql: &str) -> bool {
    matches!(
        sql.split_whitespace().next(),
        Some("select" | "with" | "show" | "describe" | "explain" | "pragma")
    )
}

fn is_paginated_query(sql: &str) -> bool {
    matches!(sql.split_whitespace().next(), Some("select" | "with"))
}

fn build_paginated_query(sql: &str, page_size: u32, offset: u64) -> String {
    let base_sql = sql.trim().trim_end_matches(';');
    let limit = page_size as u64 + 1;
    format!("select * from ({base_sql}) as showel_page limit {limit} offset {offset}")
}

fn build_editable_paginated_query(
    plan: &EditableSelectPlan,
    page_size: u32,
    offset: u64,
    locator_expr: &str,
) -> String {
    let limit = page_size as u64 + 1;
    if plan.tail.is_empty() {
        format!(
            r#"select {locator_expr} as "{LOCATOR_COLUMN}", {} from {} limit {limit} offset {offset}"#,
            plan.select_list, plan.source.qualified_name
        )
    } else {
        format!(
            r#"select {locator_expr} as "{LOCATOR_COLUMN}", {} from {} {} limit {limit} offset {offset}"#,
            plan.select_list, plan.source.qualified_name, plan.tail
        )
    }
}

pub(crate) fn sqlite_rows_to_page(rows: Vec<sqlx::sqlite::SqliteRow>) -> QueryPage {
    let columns = rows
        .first()
        .map(|row| row.columns().iter().map(|c| c.name().to_string()).collect())
        .unwrap_or_default();

    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|idx| sqlite_cell_to_string(&row, idx))
                .collect()
        })
        .collect();

    QueryPage {
        columns,
        page_size: rows.len() as u32,
        rows,
        editable: None,
        offset: 0,
        has_previous: false,
        has_next: false,
    }
}

pub(crate) fn postgres_rows_to_page(rows: Vec<sqlx::postgres::PgRow>) -> QueryPage {
    let columns = rows
        .first()
        .map(|row| row.columns().iter().map(|c| c.name().to_string()).collect())
        .unwrap_or_default();

    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|idx| postgres_cell_to_string(&row, idx))
                .collect()
        })
        .collect();

    QueryPage {
        columns,
        page_size: rows.len() as u32,
        rows,
        editable: None,
        offset: 0,
        has_previous: false,
        has_next: false,
    }
}

fn sqlite_rows_to_paginated_page(
    mut rows: Vec<sqlx::sqlite::SqliteRow>,
    page_size: u32,
    offset: u64,
) -> QueryPage {
    let columns = rows
        .first()
        .map(|row| row.columns().iter().map(|c| c.name().to_string()).collect())
        .unwrap_or_default();
    let has_next = rows.len() > page_size as usize;
    if has_next {
        rows.truncate(page_size as usize);
    }
    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|idx| sqlite_cell_to_string(&row, idx))
                .collect()
        })
        .collect();

    QueryPage {
        columns,
        rows,
        editable: None,
        offset,
        page_size,
        has_previous: offset > 0,
        has_next,
    }
}

fn postgres_rows_to_paginated_page(
    mut rows: Vec<sqlx::postgres::PgRow>,
    page_size: u32,
    offset: u64,
) -> QueryPage {
    let columns = rows
        .first()
        .map(|row| row.columns().iter().map(|c| c.name().to_string()).collect())
        .unwrap_or_default();
    let has_next = rows.len() > page_size as usize;
    if has_next {
        rows.truncate(page_size as usize);
    }
    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            (0..row.columns().len())
                .map(|idx| postgres_cell_to_string(&row, idx))
                .collect()
        })
        .collect();

    QueryPage {
        columns,
        rows,
        editable: None,
        offset,
        page_size,
        has_previous: offset > 0,
        has_next,
    }
}

fn sqlite_preview_rows_to_paginated_page(
    mut rows: Vec<sqlx::sqlite::SqliteRow>,
    source: TablePreviewSource,
    page_size: u32,
    offset: u64,
) -> QueryPage {
    let columns = rows
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .skip(1)
                .map(|c| c.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    let has_next = rows.len() > page_size as usize;
    if has_next {
        rows.truncate(page_size as usize);
    }
    let row_locators = rows
        .iter()
        .map(|row| {
            row.try_get::<i64, _>(0)
                .map(|v| v.to_string())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let rows = rows
        .into_iter()
        .map(|row| {
            (1..row.columns().len())
                .map(|idx| sqlite_cell_to_string(&row, idx))
                .collect()
        })
        .collect();

    QueryPage {
        columns,
        rows,
        editable: Some(EditableTableContext {
            source,
            row_locators,
        }),
        offset,
        page_size,
        has_previous: offset > 0,
        has_next,
    }
}

fn postgres_preview_rows_to_paginated_page(
    mut rows: Vec<sqlx::postgres::PgRow>,
    source: TablePreviewSource,
    page_size: u32,
    offset: u64,
) -> QueryPage {
    let columns = rows
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .skip(1)
                .map(|c| c.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    let has_next = rows.len() > page_size as usize;
    if has_next {
        rows.truncate(page_size as usize);
    }
    let row_locators = rows
        .iter()
        .map(|row| row.try_get::<String, _>(0).unwrap_or_default())
        .collect::<Vec<_>>();
    let rows = rows
        .into_iter()
        .map(|row| {
            (1..row.columns().len())
                .map(|idx| postgres_cell_to_string(&row, idx))
                .collect()
        })
        .collect();

    QueryPage {
        columns,
        rows,
        editable: Some(EditableTableContext {
            source,
            row_locators,
        }),
        offset,
        page_size,
        has_previous: offset > 0,
        has_next,
    }
}

fn sqlite_cell_to_string(row: &sqlx::sqlite::SqliteRow, idx: usize) -> String {
    if let Ok(value) = row.try_get::<Option<String>, _>(idx) {
        return value.unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<i64>, _>(idx) {
        return value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<f64>, _>(idx) {
        return value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<bool>, _>(idx) {
        return value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return value
            .map(|bytes| format!("<{} bytes>", bytes.len()))
            .unwrap_or_else(|| "NULL".to_string());
    }

    "<unsupported>".to_string()
}

fn postgres_cell_to_string(row: &sqlx::postgres::PgRow, idx: usize) -> String {
    if let Ok(value) = row.try_get::<Option<String>, _>(idx) {
        return value.unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<i64>, _>(idx) {
        return value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<f64>, _>(idx) {
        return value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<bool>, _>(idx) {
        return value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(value) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return value
            .map(|bytes| format!("<{} bytes>", bytes.len()))
            .unwrap_or_else(|| "NULL".to_string());
    }

    "<unsupported>".to_string()
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn sql_literal(value: &str) -> String {
    if value.eq_ignore_ascii_case("null") {
        "NULL".to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

fn editable_select_plan(sql: &str) -> Option<EditableSelectPlan> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("select ") || lower.starts_with("select distinct ") {
        return None;
    }

    let from_idx = find_top_level_keyword(&lower, " from ")?;
    let select_list = trimmed[6..from_idx].trim().to_string();
    if !is_simple_projection(&select_list) {
        return None;
    }

    let after_from = trimmed[from_idx + " from ".len()..].trim();
    let (table_ref, tail) = split_table_ref(after_from)?;
    let tail = strip_limit_offset(tail.trim());
    let tail_lower = tail.to_lowercase();
    if tail_lower.contains(" join ")
        || tail_lower.contains(" union ")
        || tail_lower.contains(" intersect ")
        || tail_lower.contains(" except ")
        || tail_lower.contains(" group by ")
        || tail_lower.contains(" having ")
    {
        return None;
    }

    let (schema, table_name) = split_qualified_name(&table_ref);
    Some(EditableSelectPlan {
        source: TablePreviewSource {
            schema,
            table_name,
            qualified_name: table_ref,
        },
        select_list,
        tail,
    })
}

fn find_top_level_keyword(sql: &str, needle: &str) -> Option<usize> {
    let bytes = sql.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut depth = 0i32;
    let mut idx = 0usize;

    while idx + needle_bytes.len() <= bytes.len() {
        let ch = bytes[idx] as char;
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => depth += 1,
            ')' if !in_single && !in_double && depth > 0 => depth -= 1,
            _ => {}
        }

        if !in_single
            && !in_double
            && depth == 0
            && &bytes[idx..idx + needle_bytes.len()] == needle_bytes
        {
            return Some(idx);
        }
        idx += 1;
    }

    None
}

fn split_table_ref(after_from: &str) -> Option<(String, String)> {
    let mut in_double = false;
    let mut depth = 0i32;

    for (idx, ch) in after_from.char_indices() {
        match ch {
            '"' => in_double = !in_double,
            '(' if !in_double => depth += 1,
            ')' if !in_double && depth > 0 => depth -= 1,
            ' ' | '\n' | '\t' if !in_double && depth == 0 => {
                let table = after_from[..idx].trim().to_string();
                let tail = after_from[idx..].trim().to_string();
                return Some((table, tail));
            }
            _ => {}
        }
    }

    if after_from.is_empty() {
        None
    } else {
        Some((after_from.trim().to_string(), String::new()))
    }
}

fn split_qualified_name(table_ref: &str) -> (Option<String>, String) {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_double = false;

    for (idx, ch) in table_ref.char_indices() {
        match ch {
            '"' => in_double = !in_double,
            '.' if !in_double => {
                parts.push(table_ref[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(table_ref[start..].trim().to_string());

    match parts.as_slice() {
        [table] => (None, unquote_identifier(table)),
        [schema, table] => (Some(unquote_identifier(schema)), unquote_identifier(table)),
        _ => (None, table_ref.to_string()),
    }
}

fn unquote_identifier(identifier: &str) -> String {
    let trimmed = identifier.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].replace("\"\"", "\"")
    } else {
        trimmed.to_string()
    }
}

fn is_simple_projection(select_list: &str) -> bool {
    let trimmed = select_list.trim();
    if trimmed == "*" || trimmed.ends_with(".*") {
        return true;
    }

    split_projection_items(trimmed)
        .into_iter()
        .all(|item| is_simple_column_ref(item.trim()))
}

fn split_projection_items(select_list: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut depth = 0i32;

    for (idx, ch) in select_list.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => depth += 1,
            ')' if !in_single && !in_double && depth > 0 => depth -= 1,
            ',' if !in_single && !in_double && depth == 0 => {
                parts.push(&select_list[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&select_list[start..]);
    parts
}

fn is_simple_column_ref(item: &str) -> bool {
    let lowered = item.to_lowercase();
    if lowered.contains(" as ")
        || item.contains('(')
        || item.contains(')')
        || item.contains('+')
        || item.contains('-')
        || item.contains('*')
        || item.contains('/')
    {
        return false;
    }

    item.split('.').all(|part| {
        let part = part.trim();
        if part.is_empty() {
            return false;
        }
        if part.starts_with('"') && part.ends_with('"') {
            return true;
        }
        part.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn strip_limit_offset(tail: &str) -> String {
    let lower = tail.to_lowercase();
    let limit_pos = find_top_level_keyword(&lower, " limit ");
    let offset_pos = find_top_level_keyword(&lower, " offset ");

    match (limit_pos, offset_pos) {
        (Some(limit), Some(offset)) => tail[..limit.min(offset)].trim().to_string(),
        (Some(limit), None) => tail[..limit].trim().to_string(),
        (None, Some(offset)) => tail[..offset].trim().to_string(),
        (None, None) => tail.trim().to_string(),
    }
}
