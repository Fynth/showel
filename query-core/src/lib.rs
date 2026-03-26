mod build;
mod editable;
mod rows;

use driver_clickhouse::{execute_json_query, execute_text_query};
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

pub fn is_read_only_sql(sql: &str) -> bool {
    matches!(
        leading_sql_keyword(sql).as_deref(),
        Some("select" | "with" | "show" | "describe" | "explain" | "pragma")
    )
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
            let schema_name = source
                .schema
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let pk_result =
                clickhouse_get_primary_key_columns(&config, &schema_name, &source.table_name)
                    .await?;

            let (response, row_locators) = if let Some((ref pk_columns, _)) = pk_result {
                let pk_select = pk_columns
                    .iter()
                    .map(|c| quote_identifier_clickhouse(c))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = build_outer_paginated_query(
                    format!("select {pk_select}, * from {}", source.qualified_name),
                    page_size,
                    offset,
                    filter.as_ref(),
                    sort.as_ref(),
                    CLICKHOUSE_DIALECT,
                );
                let response = execute_json_query(&config, &sql)
                    .await
                    .map_err(DatabaseError::ClickHouse)?;

                let pk_count = pk_columns.len();
                let row_locators: Vec<String> = response
                    .data
                    .iter()
                    .map(|row| build_clickhouse_locator(pk_columns, &row[..pk_count]))
                    .collect();
                (response, row_locators)
            } else {
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
                (response, vec![])
            };

            let editable = if !row_locators.is_empty() {
                Some(models::EditableTableContext {
                    source,
                    row_locators,
                })
            } else {
                None
            };

            let (columns, rows) = if let Some((ref pk_columns, _)) = pk_result {
                let pk_count = pk_columns.len();
                let columns: Vec<String> = response.meta[pk_count..]
                    .iter()
                    .map(|m| m.name.clone())
                    .collect();
                let rows: Vec<Vec<String>> = response
                    .data
                    .iter()
                    .map(|row| {
                        row[pk_count..]
                            .iter()
                            .map(clickhouse_json_value_to_string)
                            .collect()
                    })
                    .collect();
                (columns, rows)
            } else {
                let columns: Vec<String> = response.meta.iter().map(|m| m.name.clone()).collect();
                let rows: Vec<Vec<String>> = response
                    .data
                    .iter()
                    .map(|row| row.iter().map(clickhouse_json_value_to_string).collect())
                    .collect();
                (columns, rows)
            };

            let has_next = response.data.len() > page_size as usize;
            Ok(QueryOutput::Table(models::QueryPage {
                columns,
                rows,
                editable,
                offset,
                page_size,
                has_previous: offset > 0,
                has_next,
            }))
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
        DatabaseConnection::ClickHouse(config) => {
            let schema_name = source
                .schema
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let pk_result =
                clickhouse_get_primary_key_columns(&config, &schema_name, &source.table_name)
                    .await?;

            let Some((pk_columns, _)) = pk_result else {
                return Err(DatabaseError::UnsupportedDriver(
                    "ClickHouse table must have a primary key for updates".to_string(),
                ));
            };

            let conditions = parse_clickhouse_locator(&locator, &pk_columns);
            if conditions.is_empty() {
                return Err(DatabaseError::UnsupportedDriver(
                    "Invalid row locator".to_string(),
                ));
            }

            let where_clause = conditions
                .iter()
                .map(|(col, val)| format!("{} = {}", quote_identifier_clickhouse(col), val))
                .collect::<Vec<_>>()
                .join(" AND ");

            let column = quote_identifier_clickhouse(&column_name);
            let value_literal = sql_literal(&value);

            let sql = format!(
                "ALTER TABLE {} UPDATE {} = {} WHERE {}",
                source.qualified_name, column, value_literal, where_clause
            );

            execute_text_query(&config, &sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            Ok(())
        }
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
        DatabaseConnection::ClickHouse(config) => {
            let sql = build_insert_row_sql(&source, &column_values);
            let sql = sql.replace('"', "`");

            execute_text_query(&config, &sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            Ok(())
        }
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
        DatabaseConnection::ClickHouse(config) => {
            let schema_name = source
                .schema
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let pk_result =
                clickhouse_get_primary_key_columns(&config, &schema_name, &source.table_name)
                    .await?;

            let Some((pk_columns, data_type)) = pk_result else {
                return Ok(None);
            };

            if !clickhouse_type_supports_auto_id(&data_type) {
                return Ok(None);
            }

            let column = quote_identifier_clickhouse(&pk_columns[0]);
            let sql = format!(
                "SELECT toString(COALESCE(MAX({}), 0) + 1) AS next_id FROM {}",
                column, source.qualified_name
            );
            let response = execute_json_query(&config, &sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;

            if let Some(row) = response.data.first()
                && let Some(val) = row.first()
            {
                let next_id = match val {
                    serde_json::Value::String(s) => s.parse::<i64>().unwrap_or(1),
                    serde_json::Value::Number(n) => n.as_i64().unwrap_or(1),
                    _ => 1,
                };
                Ok(Some((pk_columns[0].clone(), next_id)))
            } else {
                Ok(None)
            }
        }
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
        DatabaseConnection::ClickHouse(config) => {
            let schema_name = source
                .schema
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let pk_result =
                clickhouse_get_primary_key_columns(&config, &schema_name, &source.table_name)
                    .await?;

            let Some((pk_columns, _)) = pk_result else {
                return Err(DatabaseError::UnsupportedDriver(
                    "ClickHouse table must have a primary key for deletes".to_string(),
                ));
            };

            let conditions = parse_clickhouse_locator(&locator, &pk_columns);
            if conditions.is_empty() {
                return Err(DatabaseError::UnsupportedDriver(
                    "Invalid row locator".to_string(),
                ));
            }

            let where_clause = conditions
                .iter()
                .map(|(col, val)| format!("{} = {}", quote_identifier_clickhouse(col), val))
                .collect::<Vec<_>>()
                .join(" AND ");

            let sql = format!(
                "ALTER TABLE {} DELETE WHERE {}",
                source.qualified_name, where_clause
            );

            execute_text_query(&config, &sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            Ok(())
        }
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
    is_read_only_sql(sql)
}

fn is_paginated_query(sql: &str) -> bool {
    matches!(leading_sql_keyword(sql).as_deref(), Some("select" | "with"))
}

fn leading_sql_keyword(sql: &str) -> Option<String> {
    sql.split_whitespace()
        .next()
        .map(|keyword| keyword.trim_matches(|ch: char| matches!(ch, '(' | ';')))
        .filter(|keyword| !keyword.is_empty())
        .map(str::to_ascii_lowercase)
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

#[cfg(test)]
mod tests {
    use super::{
        execute_query_page, is_read_only_sql, parse_clickhouse_primary_key_expression,
        reorder_clickhouse_primary_key_columns,
    };
    use models::{DatabaseConnection, QueryOutput};
    use sqlx::SqlitePool;

    #[test]
    fn read_only_sql_detection_matches_supported_queries() {
        assert!(is_read_only_sql("select * from products"));
        assert!(is_read_only_sql(
            "WITH recent AS (select 1) select * from recent"
        ));
        assert!(is_read_only_sql("show tables"));
        assert!(is_read_only_sql("pragma table_info(products)"));
        assert!(!is_read_only_sql("update products set price = 10"));
        assert!(!is_read_only_sql("delete from products"));
    }

    #[tokio::test]
    async fn execute_query_page_supports_quoted_sqlite_table_names() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        sqlx::query(
            r#"
            create table "products" (
                id integer primary key,
                name text not null,
                price real not null
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            insert into "products" (name, price)
            values
                ('Wireless Mouse', 29.99),
                ('Mechanical Keyboard', 89.99);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = execute_query_page(
            DatabaseConnection::Sqlite(pool),
            r#"select * from "products" limit 100;"#.to_string(),
            100,
            0,
            None,
            None,
        )
        .await
        .unwrap();

        match result {
            QueryOutput::Table(page) => {
                assert_eq!(page.columns, vec!["id", "name", "price"]);
                assert_eq!(page.rows.len(), 2);
                assert_eq!(page.rows[0][1], "Wireless Mouse");
                assert!(page.editable.is_some());
            }
            other => panic!("expected table result, got {other:?}"),
        }
    }

    #[test]
    fn parses_clickhouse_primary_key_expression_in_declared_order() {
        assert_eq!(
            parse_clickhouse_primary_key_expression("tuple(created_at, id)"),
            vec!["created_at", "id"]
        );
        assert_eq!(
            parse_clickhouse_primary_key_expression("`event id`, shard"),
            vec!["event id", "shard"]
        );
    }

    #[test]
    fn reorders_clickhouse_primary_key_columns_from_primary_key_expression() {
        let pk_columns = vec![
            ("id".to_string(), "UInt64".to_string()),
            ("created_at".to_string(), "DateTime".to_string()),
        ];

        assert_eq!(
            reorder_clickhouse_primary_key_columns(pk_columns, "tuple(created_at, id)"),
            vec![
                ("created_at".to_string(), "DateTime".to_string()),
                ("id".to_string(), "UInt64".to_string()),
            ]
        );
    }

    #[test]
    fn keeps_table_order_when_clickhouse_primary_key_expression_is_not_plain_columns() {
        let pk_columns = vec![
            ("created_at".to_string(), "DateTime".to_string()),
            ("id".to_string(), "UInt64".to_string()),
        ];

        assert_eq!(
            reorder_clickhouse_primary_key_columns(
                pk_columns.clone(),
                "tuple(toDate(created_at), id)"
            ),
            pk_columns
        );
    }
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

fn clickhouse_type_supports_auto_id(data_type: &str) -> bool {
    matches!(
        data_type.to_ascii_lowercase().as_str(),
        "int8" | "int16" | "int32" | "int64" | "uint8" | "uint16" | "uint32" | "uint64"
    )
}

async fn clickhouse_get_primary_key_columns(
    config: &models::ClickHouseFormData,
    schema_name: &str,
    table_name: &str,
) -> Result<Option<(Vec<String>, String)>, DatabaseError> {
    let primary_key_expression_sql = format!(
        "SELECT primary_key FROM system.tables \
         WHERE database = {} AND name = {} \
         LIMIT 1",
        sql_literal(schema_name),
        sql_literal(table_name)
    );
    let primary_key_expression = execute_json_query(config, &primary_key_expression_sql)
        .await
        .map_err(DatabaseError::ClickHouse)?
        .data
        .into_iter()
        .next()
        .and_then(|row| row.into_iter().next())
        .map(|value| clickhouse_json_value_to_string(&value))
        .unwrap_or_default();

    let columns_sql = format!(
        "SELECT name, type, is_in_primary_key FROM system.columns \
         WHERE database = {} AND table = {} \
         ORDER BY position",
        sql_literal(schema_name),
        sql_literal(table_name)
    );
    let response = execute_json_query(config, &columns_sql)
        .await
        .map_err(DatabaseError::ClickHouse)?;

    let mut pk_columns = Vec::new();
    for row in response.data {
        if row.len() < 3 {
            continue;
        }

        let is_in_primary_key = clickhouse_json_value_to_string(&row[2]) == "1";
        if is_in_primary_key {
            pk_columns.push((
                clickhouse_json_value_to_string(&row[0]),
                clickhouse_json_value_to_string(&row[1]),
            ));
        }
    }

    if pk_columns.is_empty() {
        return Ok(None);
    }

    let pk_columns = reorder_clickhouse_primary_key_columns(pk_columns, &primary_key_expression);
    let first_type = pk_columns
        .first()
        .map(|(_, data_type)| data_type.clone())
        .unwrap_or_default();
    let pk_column_names = pk_columns
        .into_iter()
        .map(|(column_name, _)| column_name)
        .collect();

    Ok(Some((pk_column_names, first_type)))
}

fn reorder_clickhouse_primary_key_columns(
    pk_columns: Vec<(String, String)>,
    primary_key_expression: &str,
) -> Vec<(String, String)> {
    let parsed_order = parse_clickhouse_primary_key_expression(primary_key_expression);
    if parsed_order.len() != pk_columns.len() {
        return pk_columns;
    }

    let original = pk_columns.clone();
    let mut remaining = pk_columns;
    let mut ordered = Vec::with_capacity(remaining.len());

    for column_name in parsed_order {
        let Some(index) = remaining.iter().position(|(name, _)| *name == column_name) else {
            return original;
        };
        ordered.push(remaining.remove(index));
    }

    ordered.extend(remaining);
    ordered
}

fn parse_clickhouse_primary_key_expression(expression: &str) -> Vec<String> {
    let expression = expression.trim();
    if expression.is_empty() || expression.eq_ignore_ascii_case("tuple()") {
        return Vec::new();
    }

    let expression = strip_clickhouse_tuple_wrapper(expression).unwrap_or(expression);
    split_clickhouse_expression_list(expression)
        .into_iter()
        .filter_map(parse_clickhouse_identifier_expression)
        .collect()
}

fn strip_clickhouse_tuple_wrapper(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    if !expression
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("tuple"))
    {
        return None;
    }

    let open_index = expression.find('(')?;
    if open_index != 5 || !expression.ends_with(')') {
        return None;
    }

    Some(&expression[(open_index + 1)..(expression.len() - 1)])
}

fn split_clickhouse_expression_list(expression: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut in_backticks = false;
    let mut in_double_quotes = false;

    for (index, ch) in expression.char_indices() {
        match ch {
            '`' if !in_double_quotes => in_backticks = !in_backticks,
            '"' if !in_backticks => in_double_quotes = !in_double_quotes,
            '(' if !in_backticks && !in_double_quotes => depth += 1,
            ')' if !in_backticks && !in_double_quotes && depth > 0 => depth -= 1,
            ',' if !in_backticks && !in_double_quotes && depth == 0 => {
                segments.push(expression[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    segments.push(expression[start..].trim());
    segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn parse_clickhouse_identifier_expression(expression: &str) -> Option<String> {
    let expression = expression.trim();
    if expression.is_empty() {
        return None;
    }

    if expression.starts_with('`') && expression.ends_with('`') && expression.len() >= 2 {
        return Some(expression[1..(expression.len() - 1)].replace("``", "`"));
    }

    if expression.starts_with('"') && expression.ends_with('"') && expression.len() >= 2 {
        return Some(expression[1..(expression.len() - 1)].replace("\"\"", "\""));
    }

    expression
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .then(|| expression.to_string())
}

fn clickhouse_json_value_to_string_for_pk(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Number(v) => v.to_string(),
        serde_json::Value::String(v) => format!("'{}'", v.replace('\'', "''")),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "NULL".to_string()),
    }
}

fn build_clickhouse_locator(pk_columns: &[String], row_values: &[serde_json::Value]) -> String {
    let mut parts = Vec::new();
    for (col, val) in pk_columns.iter().zip(row_values.iter()) {
        let encoded_col = col.replace('`', "``");
        let encoded_val = clickhouse_json_value_to_string_for_pk(val);
        parts.push(format!("{}={}", encoded_col, encoded_val));
    }
    parts.join("|")
}

fn parse_clickhouse_locator(locator: &str, _pk_columns: &[String]) -> Vec<(String, String)> {
    let parts: Vec<&str> = locator.split('|').collect();
    let mut result = Vec::new();
    for part in parts {
        if let Some((col, val)) = part.split_once('=') {
            let col_decoded = col.replace("``", "`");
            result.push((col_decoded, val.to_string()));
        }
    }
    result
}

fn clickhouse_json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "<unsupported>".to_string()),
    }
}
