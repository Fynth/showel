use drivers::clickhouse::{execute_json_query, execute_text_query};
use models::{DatabaseConnection, DatabaseError, ExplorerNode, ExplorerNodeKind, QueryOutput};
use sqlx::Row;

pub async fn describe_table(
    connection: DatabaseConnection,
    schema: Option<String>,
    table: String,
) -> Result<QueryOutput, DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let schema_name = schema.unwrap_or_else(|| "main".to_string());
            let mut rows = Vec::new();

            let table_sql = format!(
                "select sql from {}.sqlite_master where type in ('table', 'view') and name = ?1",
                quote_identifier(&schema_name)
            );
            if let Some(create_sql) = sqlx::query_scalar::<_, Option<String>>(&table_sql)
                .bind(&table)
                .fetch_optional(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?
                .flatten()
            {
                rows.push(structure_row(
                    "table",
                    table.clone(),
                    "definition",
                    String::new(),
                    create_sql,
                ));
            }

            let columns_sql = format!(
                "PRAGMA {}.table_info({})",
                quote_identifier(&schema_name),
                quote_identifier(&table)
            );
            let column_rows = sqlx::query(&columns_sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            for row in column_rows {
                let column_name = row
                    .try_get::<String, _>("name")
                    .map_err(DatabaseError::Sqlite)?;
                let data_type = row
                    .try_get::<String, _>("type")
                    .unwrap_or_else(|_| "TEXT".to_string());
                let not_null = row.try_get::<i64, _>("notnull").unwrap_or(0) == 1;
                let default_value = row
                    .try_get::<Option<String>, _>("dflt_value")
                    .ok()
                    .flatten();
                let pk_position = row.try_get::<i64, _>("pk").unwrap_or(0);
                rows.push(structure_row(
                    "column",
                    column_name,
                    data_type,
                    if pk_position > 0 {
                        format!("pk#{pk_position}")
                    } else {
                        String::new()
                    },
                    sqlite_column_details(not_null, default_value),
                ));
            }

            let index_sql = format!(
                "PRAGMA {}.index_list({})",
                quote_identifier(&schema_name),
                quote_identifier(&table)
            );
            let index_rows = sqlx::query(&index_sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            for row in index_rows {
                let index_name = row
                    .try_get::<String, _>("name")
                    .map_err(DatabaseError::Sqlite)?;
                let unique = row.try_get::<i64, _>("unique").unwrap_or(0) == 1;
                let origin = row
                    .try_get::<String, _>("origin")
                    .unwrap_or_else(|_| String::new());
                let partial = row.try_get::<i64, _>("partial").unwrap_or(0) == 1;
                let index_columns =
                    load_sqlite_index_columns(&pool, &schema_name, &index_name).await?;
                let create_sql = sqlx::query_scalar::<_, Option<String>>(&format!(
                    "select sql from {}.sqlite_master where type = 'index' and name = ?1",
                    quote_identifier(&schema_name)
                ))
                .bind(&index_name)
                .fetch_optional(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?
                .flatten()
                .unwrap_or_default();

                rows.push(structure_row(
                    "index",
                    index_name,
                    if unique {
                        "UNIQUE".to_string()
                    } else {
                        "INDEX".to_string()
                    },
                    index_columns.join(", "),
                    join_non_empty([
                        (!origin.is_empty()).then(|| format!("origin: {origin}")),
                        partial.then(|| "partial".to_string()),
                        (!create_sql.is_empty()).then_some(create_sql),
                    ]),
                ));
            }

            let foreign_key_sql = format!(
                "PRAGMA {}.foreign_key_list({})",
                quote_identifier(&schema_name),
                quote_identifier(&table)
            );
            let foreign_key_rows = sqlx::query(&foreign_key_sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            for row in foreign_key_rows {
                let id = row.try_get::<i64, _>("id").unwrap_or_default();
                let from_column = row
                    .try_get::<String, _>("from")
                    .unwrap_or_else(|_| String::new());
                let target_table = row
                    .try_get::<String, _>("table")
                    .unwrap_or_else(|_| String::new());
                let target_column = row
                    .try_get::<String, _>("to")
                    .unwrap_or_else(|_| String::new());
                let on_update = row
                    .try_get::<String, _>("on_update")
                    .unwrap_or_else(|_| String::new());
                let on_delete = row
                    .try_get::<String, _>("on_delete")
                    .unwrap_or_else(|_| String::new());

                rows.push(structure_row(
                    "constraint",
                    format!("fk_{id}_{from_column}"),
                    "FOREIGN KEY",
                    format!("{from_column} -> {target_table}.{target_column}"),
                    join_non_empty([
                        (!on_update.is_empty()).then(|| format!("on update {on_update}")),
                        (!on_delete.is_empty()).then(|| format!("on delete {on_delete}")),
                    ]),
                ));
            }

            let trigger_sql = format!(
                "select name, sql from {}.sqlite_master where type = 'trigger' and tbl_name = ?1 order by name",
                quote_identifier(&schema_name)
            );
            let trigger_rows = sqlx::query(&trigger_sql)
                .bind(&table)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;
            for row in trigger_rows {
                let trigger_name = row
                    .try_get::<String, _>("name")
                    .map_err(DatabaseError::Sqlite)?;
                let sql = row
                    .try_get::<Option<String>, _>("sql")
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                rows.push(structure_row(
                    "trigger",
                    trigger_name,
                    "TRIGGER",
                    String::new(),
                    sql,
                ));
            }

            Ok(QueryOutput::Table(structure_page(rows)))
        }
        DatabaseConnection::Postgres(pool) => {
            let schema_name = schema.unwrap_or_else(|| "public".to_string());
            let mut rows = Vec::new();

            let column_rows = sqlx::query(
                r#"
                select
                  ordinal_position,
                  column_name,
                  data_type,
                  is_nullable,
                  column_default
                from information_schema.columns
                where table_schema = $1
                  and table_name = $2
                order by ordinal_position
                "#,
            )
            .bind(&schema_name)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Postgres)?;
            for row in column_rows {
                let column_name = row
                    .try_get::<String, _>("column_name")
                    .map_err(DatabaseError::Postgres)?;
                let data_type = row
                    .try_get::<String, _>("data_type")
                    .unwrap_or_else(|_| "text".to_string());
                let is_nullable = row
                    .try_get::<String, _>("is_nullable")
                    .unwrap_or_else(|_| "YES".to_string());
                let default_value = row
                    .try_get::<Option<String>, _>("column_default")
                    .ok()
                    .flatten();

                rows.push(structure_row(
                    "column",
                    column_name,
                    data_type,
                    String::new(),
                    postgres_column_details(&is_nullable, default_value),
                ));
            }

            let index_rows = sqlx::query(
                r#"
                select indexname, indexdef
                from pg_indexes
                where schemaname = $1
                  and tablename = $2
                order by indexname
                "#,
            )
            .bind(&schema_name)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Postgres)?;
            for row in index_rows {
                let index_name = row
                    .try_get::<String, _>("indexname")
                    .map_err(DatabaseError::Postgres)?;
                let index_definition = row
                    .try_get::<String, _>("indexdef")
                    .unwrap_or_else(|_| String::new());
                rows.push(structure_row(
                    "index",
                    index_name,
                    if index_definition.contains(" UNIQUE INDEX ") {
                        "UNIQUE".to_string()
                    } else {
                        "INDEX".to_string()
                    },
                    String::new(),
                    index_definition,
                ));
            }

            let constraint_rows = sqlx::query(
                r#"
                select
                  c.conname as constraint_name,
                  case c.contype
                    when 'p' then 'PRIMARY KEY'
                    when 'f' then 'FOREIGN KEY'
                    when 'u' then 'UNIQUE'
                    when 'c' then 'CHECK'
                    when 'x' then 'EXCLUDE'
                    else c.contype::text
                  end as constraint_type,
                  pg_get_constraintdef(c.oid, true) as definition
                from pg_constraint c
                join pg_class t on t.oid = c.conrelid
                join pg_namespace n on n.oid = t.relnamespace
                where n.nspname = $1
                  and t.relname = $2
                order by c.conname
                "#,
            )
            .bind(&schema_name)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Postgres)?;
            for row in constraint_rows {
                let constraint_name = row
                    .try_get::<String, _>("constraint_name")
                    .map_err(DatabaseError::Postgres)?;
                let constraint_type = row
                    .try_get::<String, _>("constraint_type")
                    .unwrap_or_else(|_| "CONSTRAINT".to_string());
                let definition = row
                    .try_get::<String, _>("definition")
                    .unwrap_or_else(|_| String::new());

                rows.push(structure_row(
                    "constraint",
                    constraint_name,
                    constraint_type,
                    String::new(),
                    definition,
                ));
            }

            let trigger_rows = sqlx::query(
                r#"
                select
                  trigger_name,
                  action_timing,
                  string_agg(distinct event_manipulation, ', ' order by event_manipulation) as events,
                  action_statement
                from information_schema.triggers
                where event_object_schema = $1
                  and event_object_table = $2
                group by trigger_name, action_timing, action_statement
                order by trigger_name
                "#,
            )
            .bind(&schema_name)
            .bind(&table)
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Postgres)?;
            for row in trigger_rows {
                let trigger_name = row
                    .try_get::<String, _>("trigger_name")
                    .map_err(DatabaseError::Postgres)?;
                let timing = row
                    .try_get::<String, _>("action_timing")
                    .unwrap_or_else(|_| String::new());
                let events = row
                    .try_get::<String, _>("events")
                    .unwrap_or_else(|_| String::new());
                let action = row
                    .try_get::<String, _>("action_statement")
                    .unwrap_or_else(|_| String::new());

                rows.push(structure_row(
                    "trigger",
                    trigger_name,
                    join_non_empty([
                        (!timing.is_empty()).then_some(timing),
                        (!events.is_empty()).then_some(events),
                    ]),
                    String::new(),
                    action,
                ));
            }

            Ok(QueryOutput::Table(structure_page(rows)))
        }
        DatabaseConnection::ClickHouse(config) => {
            let schema_name = schema.unwrap_or_else(|| config.database.clone());
            let mut rows = Vec::new();

            let overview_sql = format!(
                r#"
                select
                  engine,
                  partition_key,
                  sorting_key,
                  primary_key,
                  sampling_key
                from system.tables
                where database = {}
                  and name = {}
                limit 1
                "#,
                clickhouse_string_literal(&schema_name),
                clickhouse_string_literal(&table)
            );
            let overview = execute_json_query(&config, &overview_sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            if let Some(row) = overview.data.first() {
                let engine = clickhouse_value_to_string(row.first());
                let partition_key = clickhouse_value_to_string(row.get(1));
                let sorting_key = clickhouse_value_to_string(row.get(2));
                let primary_key = clickhouse_value_to_string(row.get(3));
                let sampling_key = clickhouse_value_to_string(row.get(4));

                rows.push(structure_row(
                    "table",
                    table.clone(),
                    engine,
                    String::new(),
                    join_non_empty([
                        meaningful_clickhouse_value(&partition_key)
                            .then(|| format!("partition key: {partition_key}")),
                        meaningful_clickhouse_value(&sorting_key)
                            .then(|| format!("sorting key: {sorting_key}")),
                        meaningful_clickhouse_value(&primary_key)
                            .then(|| format!("primary key: {primary_key}")),
                        meaningful_clickhouse_value(&sampling_key)
                            .then(|| format!("sampling key: {sampling_key}")),
                    ]),
                ));
            }

            let create_sql = if schema_name.is_empty() {
                format!("SHOW CREATE TABLE {}", quote_clickhouse_identifier(&table))
            } else {
                format!(
                    "SHOW CREATE TABLE {}.{}",
                    quote_clickhouse_identifier(&schema_name),
                    quote_clickhouse_identifier(&table)
                )
            };
            let create_statement = execute_text_query(&config, &create_sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            rows.push(structure_row(
                "table",
                table.clone(),
                "definition",
                String::new(),
                create_statement.trim().to_string(),
            ));

            let columns_sql = format!(
                r#"
                select
                  name,
                  type,
                  default_kind,
                  default_expression,
                  is_in_partition_key,
                  is_in_sorting_key,
                  is_in_primary_key
                from system.columns
                where database = {}
                  and table = {}
                order by position
                "#,
                clickhouse_string_literal(&schema_name),
                clickhouse_string_literal(&table)
            );
            let columns = execute_json_query(&config, &columns_sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;
            for row in columns.data {
                let column_name = clickhouse_value_to_string(row.first());
                let column_type = clickhouse_value_to_string(row.get(1));
                let default_kind = clickhouse_value_to_string(row.get(2));
                let default_expression = clickhouse_value_to_string(row.get(3));
                let in_partition_key = clickhouse_value_to_string(row.get(4)) == "1";
                let in_sorting_key = clickhouse_value_to_string(row.get(5)) == "1";
                let in_primary_key = clickhouse_value_to_string(row.get(6)) == "1";

                rows.push(structure_row(
                    "column",
                    column_name,
                    column_type,
                    String::new(),
                    join_non_empty([
                        meaningful_clickhouse_value(&default_kind)
                            .then(|| format!("default kind: {default_kind}")),
                        meaningful_clickhouse_value(&default_expression)
                            .then(|| format!("default: {default_expression}")),
                        in_partition_key.then(|| "partition key".to_string()),
                        in_sorting_key.then(|| "sorting key".to_string()),
                        in_primary_key.then(|| "primary key".to_string()),
                    ]),
                ));
            }

            Ok(QueryOutput::Table(structure_page(rows)))
        }
    }
}

pub async fn load_table_columns(
    connection: DatabaseConnection,
    schema: Option<String>,
    table: String,
) -> Result<Vec<String>, DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let schema_name = schema.unwrap_or_else(|| "main".to_string());
            let sql = format!(
                "PRAGMA {}.table_info({})",
                quote_identifier(&schema_name),
                quote_identifier(&table)
            );

            let rows = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(DatabaseError::Sqlite)?;

            rows.into_iter()
                .map(|row| {
                    row.try_get::<String, _>("name")
                        .map_err(DatabaseError::Sqlite)
                })
                .collect()
        }
        DatabaseConnection::Postgres(pool) => {
            let schema_name = schema.unwrap_or_else(|| "public".to_string());
            let rows = sqlx::query(
                r#"
                select column_name
                from information_schema.columns
                where table_schema = $1
                  and table_name = $2
                order by ordinal_position
                "#,
            )
            .bind(schema_name)
            .bind(table)
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Postgres)?;

            rows.into_iter()
                .map(|row| {
                    row.try_get::<String, _>("column_name")
                        .map_err(DatabaseError::Postgres)
                })
                .collect()
        }
        DatabaseConnection::ClickHouse(config) => {
            let schema_name = schema.unwrap_or_else(|| config.database.clone());
            let sql = format!(
                "select name from system.columns where database = {} and table = {} order by position",
                clickhouse_string_literal(&schema_name),
                clickhouse_string_literal(&table)
            );
            let response = execute_json_query(&config, &sql)
                .await
                .map_err(DatabaseError::ClickHouse)?;

            Ok(response
                .data
                .into_iter()
                .filter_map(|row| row.first().map(clickhouse_json_value_to_string))
                .collect())
        }
    }
}

pub async fn load_connection_tree(
    connection: DatabaseConnection,
) -> Result<Vec<ExplorerNode>, DatabaseError> {
    match connection {
        DatabaseConnection::Sqlite(pool) => {
            let rows = sqlx::query(
                r#"
                select name, type
                from sqlite_master
                where type in ('table', 'view')
                  and name not like 'sqlite_%'
                order by type, name
                "#,
            )
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Sqlite)?;

            let mut tables = Vec::new();
            let mut views = Vec::new();

            for row in rows {
                let name = row
                    .try_get::<String, _>("name")
                    .map_err(DatabaseError::Sqlite)?;
                let kind = row
                    .try_get::<String, _>("type")
                    .map_err(DatabaseError::Sqlite)?;

                match kind.as_str() {
                    "table" => tables.push(ExplorerNode {
                        qualified_name: quote_identifier(&name),
                        schema: Some("main".to_string()),
                        name,
                        kind: ExplorerNodeKind::Table,
                        children: Vec::new(),
                    }),
                    "view" => views.push(ExplorerNode {
                        qualified_name: quote_identifier(&name),
                        schema: Some("main".to_string()),
                        name,
                        kind: ExplorerNodeKind::View,
                        children: Vec::new(),
                    }),
                    _ => {}
                }
            }

            Ok(vec![ExplorerNode {
                name: "main".to_string(),
                kind: ExplorerNodeKind::Schema,
                schema: Some("main".to_string()),
                qualified_name: "main".to_string(),
                children: tables.into_iter().chain(views).collect(),
            }])
        }
        DatabaseConnection::Postgres(pool) => {
            let rows = sqlx::query(
                r#"
                select table_schema, table_name, table_type
                from information_schema.tables
                where table_schema not in ('pg_catalog', 'information_schema')
                order by table_schema, table_type, table_name
                "#,
            )
            .fetch_all(&pool)
            .await
            .map_err(DatabaseError::Postgres)?;

            let mut grouped: std::collections::BTreeMap<String, Vec<ExplorerNode>> =
                std::collections::BTreeMap::new();

            for row in rows {
                let schema = row
                    .try_get::<String, _>("table_schema")
                    .map_err(DatabaseError::Postgres)?;
                let name = row
                    .try_get::<String, _>("table_name")
                    .map_err(DatabaseError::Postgres)?;
                let table_type = row
                    .try_get::<String, _>("table_type")
                    .map_err(DatabaseError::Postgres)?;

                let kind = if table_type.eq_ignore_ascii_case("view") {
                    ExplorerNodeKind::View
                } else {
                    ExplorerNodeKind::Table
                };
                let qualified_name =
                    format!("{}.{}", quote_identifier(&schema), quote_identifier(&name));

                grouped
                    .entry(schema.clone())
                    .or_default()
                    .push(ExplorerNode {
                        qualified_name,
                        schema: Some(schema.clone()),
                        name,
                        kind,
                        children: Vec::new(),
                    });
            }

            Ok(grouped
                .into_iter()
                .map(|(schema, children)| ExplorerNode {
                    qualified_name: quote_identifier(&schema),
                    schema: Some(schema.clone()),
                    name: schema,
                    kind: ExplorerNodeKind::Schema,
                    children,
                })
                .collect())
        }
        DatabaseConnection::ClickHouse(config) => {
            let response = execute_json_query(
                &config,
                r#"
                select database, name, engine
                from system.tables
                where database not in ('system', 'INFORMATION_SCHEMA', 'information_schema')
                order by database, name
                "#,
            )
            .await
            .map_err(DatabaseError::ClickHouse)?;

            let mut grouped: std::collections::BTreeMap<String, Vec<ExplorerNode>> =
                std::collections::BTreeMap::new();

            for row in response.data {
                let schema = clickhouse_value_to_string(row.first());
                let name = clickhouse_value_to_string(row.get(1));
                let engine = clickhouse_value_to_string(row.get(2));
                let kind = if engine.to_ascii_lowercase().contains("view") {
                    ExplorerNodeKind::View
                } else {
                    ExplorerNodeKind::Table
                };
                let qualified_name = format!(
                    "{}.{}",
                    quote_clickhouse_identifier(&schema),
                    quote_clickhouse_identifier(&name)
                );

                grouped
                    .entry(schema.clone())
                    .or_default()
                    .push(ExplorerNode {
                        qualified_name,
                        schema: Some(schema.clone()),
                        name,
                        kind,
                        children: Vec::new(),
                    });
            }

            Ok(grouped
                .into_iter()
                .map(|(schema, children)| ExplorerNode {
                    qualified_name: quote_clickhouse_identifier(&schema),
                    schema: Some(schema.clone()),
                    name: schema,
                    kind: ExplorerNodeKind::Schema,
                    children,
                })
                .collect())
        }
    }
}

async fn load_sqlite_index_columns(
    pool: &sqlx::SqlitePool,
    schema_name: &str,
    index_name: &str,
) -> Result<Vec<String>, DatabaseError> {
    let sql = format!(
        "PRAGMA {}.index_info({})",
        quote_identifier(schema_name),
        quote_identifier(index_name)
    );
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(DatabaseError::Sqlite)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect())
}

fn structure_page(rows: Vec<Vec<String>>) -> models::QueryPage {
    models::QueryPage {
        columns: vec![
            "section".to_string(),
            "name".to_string(),
            "type".to_string(),
            "target".to_string(),
            "details".to_string(),
        ],
        rows,
        editable: None,
        offset: 0,
        page_size: 0,
        has_previous: false,
        has_next: false,
    }
}

fn structure_row(
    section: impl Into<String>,
    name: impl Into<String>,
    row_type: impl Into<String>,
    target: impl Into<String>,
    details: impl Into<String>,
) -> Vec<String> {
    vec![
        section.into(),
        name.into(),
        row_type.into(),
        target.into(),
        details.into(),
    ]
}

fn sqlite_column_details(not_null: bool, default_value: Option<String>) -> String {
    join_non_empty([
        not_null.then(|| "NOT NULL".to_string()),
        default_value.map(|value| format!("default {value}")),
    ])
}

fn postgres_column_details(is_nullable: &str, default_value: Option<String>) -> String {
    join_non_empty([
        is_nullable
            .eq_ignore_ascii_case("NO")
            .then(|| "NOT NULL".to_string()),
        default_value.map(|value| format!("default {value}")),
    ])
}

fn meaningful_clickhouse_value(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed != "NULL"
}

fn join_non_empty(parts: impl IntoIterator<Item = Option<String>>) -> String {
    parts
        .into_iter()
        .flatten()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_clickhouse_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}

fn clickhouse_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn clickhouse_value_to_string(value: Option<&serde_json::Value>) -> String {
    value
        .map(clickhouse_json_value_to_string)
        .unwrap_or_else(|| "NULL".to_string())
}

fn clickhouse_json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "<unsupported>".to_string())
        }
    }
}
