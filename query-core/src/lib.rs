mod build;
mod editable;
mod rows;

use driver_clickhouse::execute_json_query;
use models::{
    DatabaseConnection, DatabaseError, QueryFilter, QueryOutput, QuerySort, TablePreviewSource,
};
use sqlx::Row;

use self::{
    build::{
        SqlBuildDialect, build_editable_paginated_query, build_outer_paginated_query,
        build_paginated_query, clickhouse_filter_expression, postgres_filter_expression,
        quote_identifier, quote_identifier_clickhouse, sql_literal, sqlite_filter_expression,
    },
    editable::editable_select_plan,
    rows::{
        clickhouse_rows_to_page, clickhouse_rows_to_paginated_page, invalid_sqlite_locator,
        postgres_preview_rows_to_paginated_page, postgres_rows_to_paginated_page,
        sqlite_preview_rows_to_paginated_page, sqlite_rows_to_paginated_page,
    },
};

const LOCATOR_COLUMN: &str = "__showel_locator";
const SQLITE_DIALECT: SqlBuildDialect = SqlBuildDialect {
    quote_identifier,
    filter_expression: sqlite_filter_expression,
};
const POSTGRES_DIALECT: SqlBuildDialect = SqlBuildDialect {
    quote_identifier,
    filter_expression: postgres_filter_expression,
};
const CLICKHOUSE_DIALECT: SqlBuildDialect = SqlBuildDialect {
    quote_identifier: quote_identifier_clickhouse,
    filter_expression: clickhouse_filter_expression,
};

pub(crate) use self::rows::{postgres_rows_to_page, sqlite_rows_to_page};

pub async fn execute_query(
    connection: DatabaseConnection,
    sql: String,
) -> Result<QueryOutput, DatabaseError> {
    execute_query_page(connection, sql, 100, 0, None, None).await
}

pub async fn load_table_preview_page(
    connection: DatabaseConnection,
    source: TablePreviewSource,
    page_size: u32,
    offset: u64,
    filter: Option<QueryFilter>,
    sort: Option<QuerySort>,
) -> Result<QueryOutput, DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let sql = build_outer_paginated_query(
                format!(
                    r#"select rowid as "{LOCATOR_COLUMN}", * from {}"#,
                    source.qualified_name
                ),
                page_size,
                offset,
                filter.as_ref(),
                sort.as_ref(),
                SQLITE_DIALECT,
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
            let sql = build_outer_paginated_query(
                format!(
                    r#"select ctid::text as "{LOCATOR_COLUMN}", * from {}"#,
                    source.qualified_name
                ),
                page_size,
                offset,
                filter.as_ref(),
                sort.as_ref(),
                POSTGRES_DIALECT,
            );
            let rows = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(QueryOutput::Table(postgres_preview_rows_to_paginated_page(
                rows, source, page_size, offset,
            )))
        }
        DatabaseConnection::ClickHouse(config) => {
            let sql = build_outer_paginated_query(
                format!("select * from {}", source.qualified_name),
                page_size,
                offset,
                filter.as_ref(),
                sort.as_ref(),
                CLICKHOUSE_DIALECT,
            );
            let response = execute_json_query(&config, &sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            Ok(QueryOutput::Table(clickhouse_rows_to_paginated_page(
                response, page_size, offset,
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
            let rowid = locator
                .parse::<i64>()
                .map_err(|_| invalid_sqlite_locator())?;
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
        DatabaseConnection::ClickHouse(_) => Err(DatabaseError::UnsupportedDriver(
            "ClickHouse cell updates are not supported".to_string(),
        )),
    }
}

pub async fn insert_table_row(
    connection: DatabaseConnection,
    source: TablePreviewSource,
) -> Result<(), DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let sql = format!("insert into {} default values", source.qualified_name);
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(())
        }
        DatabaseConnection::Postgres(pool) => {
            let sql = format!("insert into {} default values", source.qualified_name);
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(())
        }
        DatabaseConnection::ClickHouse(_) => Err(DatabaseError::UnsupportedDriver(
            "ClickHouse row inserts are not supported yet".to_string(),
        )),
    }
}

pub async fn insert_table_row_with_values(
    connection: DatabaseConnection,
    source: TablePreviewSource,
    column_values: Vec<(String, String)>,
) -> Result<(), DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let sql = build_insert_row_sql(&source, &column_values);
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(())
        }
        DatabaseConnection::Postgres(pool) => {
            let sql = build_insert_row_sql(&source, &column_values);
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(())
        }
        DatabaseConnection::ClickHouse(_) => Err(DatabaseError::UnsupportedDriver(
            "ClickHouse row inserts are not supported yet".to_string(),
        )),
    }
}

pub async fn next_table_primary_key_id(
    connection: DatabaseConnection,
    source: TablePreviewSource,
) -> Result<Option<(String, i64)>, DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let schema_name = source.schema.clone().unwrap_or_else(|| "main".to_string());
            let Some((column_name, data_type)) =
                sqlite_single_primary_key_column(&pool, &schema_name, &source.table_name).await?
            else {
                return Ok(None);
            };
            if !sqlite_type_supports_auto_id(&data_type) {
                return Ok(None);
            }

            let column = quote_identifier(&column_name);
            let sql = format!(
                "select cast(coalesce(max({column}), 0) + 1 as text) from {}",
                source.qualified_name
            );
            let row = sqlx::query(&sql)
                .fetch_one(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(Some((
                column_name.clone(),
                parse_next_numeric_id(
                    row.try_get::<String, _>(0).map_err(DatabaseError::Sqlite)?,
                    &column_name,
                )?,
            )))
        }
        DatabaseConnection::Postgres(pool) => {
            let schema_name = source
                .schema
                .clone()
                .unwrap_or_else(|| "public".to_string());
            let Some((column_name, data_type)) =
                postgres_single_primary_key_column(&pool, &schema_name, &source.table_name).await?
            else {
                return Ok(None);
            };
            if !postgres_type_supports_auto_id(&data_type) {
                return Ok(None);
            }

            let column = quote_identifier(&column_name);
            let sql = format!(
                "select cast(coalesce(max({column})::bigint, 0) + 1 as text) from {}",
                source.qualified_name
            );
            let row = sqlx::query(&sql)
                .fetch_one(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(Some((
                column_name.clone(),
                parse_next_numeric_id(
                    row.try_get::<String, _>(0)
                        .map_err(DatabaseError::Postgres)?,
                    &column_name,
                )?,
            )))
        }
        DatabaseConnection::ClickHouse(_) => Ok(None),
    }
}

pub async fn delete_table_row(
    connection: DatabaseConnection,
    source: TablePreviewSource,
    locator: String,
) -> Result<(), DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let rowid = locator
                .parse::<i64>()
                .map_err(|_| invalid_sqlite_locator())?;
            let sql = format!(
                "delete from {} where rowid = {}",
                source.qualified_name, rowid
            );
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(())
        }
        DatabaseConnection::Postgres(pool) => {
            let sql = format!(
                "delete from {} where ctid = {}::tid",
                source.qualified_name,
                sql_literal(&locator)
            );
            sqlx::query(&sql)
                .execute(&pool)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(())
        }
        DatabaseConnection::ClickHouse(_) => Err(DatabaseError::UnsupportedDriver(
            "ClickHouse row deletes are not supported yet".to_string(),
        )),
    }
}

pub async fn execute_query_page(
    connection: DatabaseConnection,
    sql: String,
    page_size: u32,
    offset: u64,
    filter: Option<QueryFilter>,
    sort: Option<QuerySort>,
) -> Result<QueryOutput, DatabaseError> {
    let normalized = sql.trim().to_lowercase();

    match connection {
        DatabaseConnection::Sqlite(pool) => {
            if let Some(plan) = editable_select_plan(&sql) {
                let query = build_editable_paginated_query(
                    &plan,
                    page_size,
                    offset,
                    "rowid",
                    filter.as_ref(),
                    sort.as_ref(),
                    SQLITE_DIALECT,
                );
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
                let rows = sqlx::query(&build_paginated_query(
                    &sql,
                    page_size,
                    offset,
                    filter.as_ref(),
                    sort.as_ref(),
                    SQLITE_DIALECT,
                ))
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
                let query = build_editable_paginated_query(
                    &plan,
                    page_size,
                    offset,
                    "ctid::text",
                    filter.as_ref(),
                    sort.as_ref(),
                    POSTGRES_DIALECT,
                );
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
                let rows = sqlx::query(&build_paginated_query(
                    &sql,
                    page_size,
                    offset,
                    filter.as_ref(),
                    sort.as_ref(),
                    POSTGRES_DIALECT,
                ))
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
        DatabaseConnection::ClickHouse(config) => {
            if is_paginated_query(&normalized) {
                let response = execute_json_query(
                    &config,
                    &build_paginated_query(
                        &sql,
                        page_size,
                        offset,
                        filter.as_ref(),
                        sort.as_ref(),
                        CLICKHOUSE_DIALECT,
                    ),
                )
                .await
                .map_err(DatabaseError::ClickHouse)?;
                Ok(QueryOutput::Table(clickhouse_rows_to_paginated_page(
                    response, page_size, offset,
                )))
            } else if is_tabular_query(&normalized) {
                let response = execute_json_query(&config, &sql)
                    .await
                    .map_err(DatabaseError::ClickHouse)?;
                Ok(QueryOutput::Table(clickhouse_rows_to_page(response)))
            } else {
                driver_clickhouse::execute_text_query(&config, &sql)
                    .await
                    .map_err(DatabaseError::ClickHouse)?;
                Ok(QueryOutput::AffectedRows(0))
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

fn build_insert_row_sql(source: &TablePreviewSource, column_values: &[(String, String)]) -> String {
    if column_values.is_empty() {
        return format!("insert into {} default values", source.qualified_name);
    }

    let columns = column_values
        .iter()
        .map(|(column_name, _)| quote_identifier(column_name))
        .collect::<Vec<_>>()
        .join(", ");
    let values = column_values
        .iter()
        .map(|(_, value)| sql_literal(value))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "insert into {} ({columns}) values ({values})",
        source.qualified_name
    )
}

fn parse_next_numeric_id(value: String, column_name: &str) -> Result<i64, DatabaseError> {
    value.trim().parse::<i64>().map_err(|_| {
        DatabaseError::UnsupportedDriver(format!(
            "Built-in auto id requires a numeric `{column_name}` column"
        ))
    })
}

async fn sqlite_single_primary_key_column(
    pool: &sqlx::SqlitePool,
    schema_name: &str,
    table_name: &str,
) -> Result<Option<(String, String)>, DatabaseError> {
    let sql = format!(
        "PRAGMA {}.table_info({})",
        quote_identifier(schema_name),
        quote_identifier(table_name)
    );
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(DatabaseError::Sqlite)?;

    let mut primary_key_columns = Vec::new();
    for row in rows {
        let pk_position = row.try_get::<i64, _>("pk").unwrap_or(0);
        if pk_position <= 0 {
            continue;
        }

        let column_name = row
            .try_get::<String, _>("name")
            .map_err(DatabaseError::Sqlite)?;
        let data_type = row
            .try_get::<String, _>("type")
            .unwrap_or_else(|_| String::new());
        primary_key_columns.push((pk_position, column_name, data_type));
    }

    primary_key_columns.sort_by_key(|(pk_position, _, _)| *pk_position);
    if primary_key_columns.len() != 1 {
        return Ok(None);
    }

    let (_, column_name, data_type) = primary_key_columns.remove(0);
    Ok(Some((column_name, data_type)))
}

async fn postgres_single_primary_key_column(
    pool: &sqlx::PgPool,
    schema_name: &str,
    table_name: &str,
) -> Result<Option<(String, String)>, DatabaseError> {
    let rows = sqlx::query(
        r#"
        select
          kcu.column_name,
          cols.data_type
        from information_schema.table_constraints tc
        join information_schema.key_column_usage kcu
          on tc.constraint_name = kcu.constraint_name
         and tc.table_schema = kcu.table_schema
         and tc.table_name = kcu.table_name
        join information_schema.columns cols
          on cols.table_schema = kcu.table_schema
         and cols.table_name = kcu.table_name
         and cols.column_name = kcu.column_name
        where tc.constraint_type = 'PRIMARY KEY'
          and tc.table_schema = $1
          and tc.table_name = $2
        order by kcu.ordinal_position
        "#,
    )
    .bind(schema_name)
    .bind(table_name)
    .fetch_all(pool)
    .await
    .map_err(DatabaseError::Postgres)?;

    if rows.len() != 1 {
        return Ok(None);
    }

    let row = &rows[0];
    let column_name = row
        .try_get::<String, _>("column_name")
        .map_err(DatabaseError::Postgres)?;
    let data_type = row
        .try_get::<String, _>("data_type")
        .unwrap_or_else(|_| String::new());
    Ok(Some((column_name, data_type)))
}

fn sqlite_type_supports_auto_id(data_type: &str) -> bool {
    data_type.to_ascii_lowercase().contains("int")
}

fn postgres_type_supports_auto_id(data_type: &str) -> bool {
    matches!(
        data_type.to_ascii_lowercase().as_str(),
        "smallint" | "integer" | "bigint"
    )
}
