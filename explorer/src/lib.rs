use driver_clickhouse::{execute_json_query, execute_text_query};
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
        DatabaseConnection::MySql(pool) => describe_mysql_table(pool, schema, table).await,
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
        DatabaseConnection::MySql(pool) => load_mysql_table_columns(pool, schema, table).await,
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
        DatabaseConnection::MySql(pool) => load_mysql_connection_tree(pool).await,
        DatabaseConnection::ClickHouse(config) => {
            let response = execute_json_query(
                &config,
                r#"
                select database, name, engine, create_table_query
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
                let create_table_query = clickhouse_value_to_string(row.get(3));
                if !clickhouse_relation_supports_preview(&engine, &create_table_query) {
                    continue;
                }
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

async fn describe_mysql_table(
    pool: sqlx::MySqlPool,
    schema: Option<String>,
    table: String,
) -> Result<QueryOutput, DatabaseError> {
    let schema_name = mysql_effective_schema_name(&pool, schema.as_deref()).await?;
    let mut rows = Vec::new();

    let overview_rows = sqlx::query(
        r#"
        select table_type, engine
        from information_schema.tables
        where table_schema = ?
          and table_name = ?
        limit 1
        "#,
    )
    .bind(&schema_name)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;
    if let Some(row) = overview_rows.first() {
        let table_type = row
            .try_get::<String, _>("table_type")
            .unwrap_or_else(|_| "TABLE".to_string());
        let engine = row
            .try_get::<Option<String>, _>("engine")
            .ok()
            .flatten()
            .unwrap_or_else(|| table_type.clone());
        rows.push(structure_row(
            "table",
            table.clone(),
            engine,
            String::new(),
            format!("schema: {schema_name}"),
        ));
    }

    let create_sql = format!(
        "show create table {}",
        qualified_mysql_table_name(&schema_name, &table)
    );
    if let Some(row) = sqlx::query(&create_sql)
        .fetch_optional(&pool)
        .await
        .map_err(DatabaseError::MySql)?
    {
        let create_statement = row
            .try_get::<String, _>(1)
            .or_else(|_| row.try_get::<String, _>("Create Table"))
            .or_else(|_| row.try_get::<String, _>("Create View"))
            .unwrap_or_default();
        if !create_statement.trim().is_empty() {
            rows.push(structure_row(
                "table",
                table.clone(),
                "definition",
                String::new(),
                create_statement,
            ));
        }
    }

    let column_rows = sqlx::query(
        r#"
        select column_name, column_type, is_nullable, column_default, extra
        from information_schema.columns
        where table_schema = ?
          and table_name = ?
        order by ordinal_position
        "#,
    )
    .bind(&schema_name)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;
    for row in column_rows {
        let column_name = row
            .try_get::<String, _>("column_name")
            .map_err(DatabaseError::MySql)?;
        let column_type = row
            .try_get::<String, _>("column_type")
            .unwrap_or_else(|_| "text".to_string());
        let is_nullable = row
            .try_get::<String, _>("is_nullable")
            .unwrap_or_else(|_| "YES".to_string());
        let default_value = row
            .try_get::<Option<String>, _>("column_default")
            .ok()
            .flatten();
        let extra = row
            .try_get::<Option<String>, _>("extra")
            .ok()
            .flatten()
            .unwrap_or_default();
        rows.push(structure_row(
            "column",
            column_name,
            column_type,
            String::new(),
            mysql_column_details(&is_nullable, default_value, &extra),
        ));
    }

    let index_rows = sqlx::query(
        r#"
        select index_name, non_unique, index_type, seq_in_index, column_name
        from information_schema.statistics
        where table_schema = ?
          and table_name = ?
        order by index_name, seq_in_index
        "#,
    )
    .bind(&schema_name)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;
    let mut grouped_indexes: std::collections::BTreeMap<String, (bool, String, Vec<String>)> =
        std::collections::BTreeMap::new();
    for row in index_rows {
        let index_name = row
            .try_get::<String, _>("index_name")
            .map_err(DatabaseError::MySql)?;
        let non_unique = row.try_get::<i64, _>("non_unique").unwrap_or(1) != 0;
        let index_type = row
            .try_get::<Option<String>, _>("index_type")
            .ok()
            .flatten()
            .unwrap_or_else(|| "INDEX".to_string());
        let column_name = row
            .try_get::<Option<String>, _>("column_name")
            .ok()
            .flatten()
            .unwrap_or_default();
        let entry =
            grouped_indexes
                .entry(index_name)
                .or_insert((non_unique, index_type, Vec::new()));
        if !column_name.is_empty() {
            entry.2.push(column_name);
        }
    }
    for (index_name, (non_unique, index_type, columns)) in grouped_indexes {
        rows.push(structure_row(
            "index",
            index_name,
            if non_unique {
                index_type
            } else {
                format!("UNIQUE {index_type}")
            },
            columns.join(", "),
            String::new(),
        ));
    }

    let constraint_rows = sqlx::query(
        r#"
        select
          tc.constraint_name,
          tc.constraint_type,
          kcu.column_name,
          kcu.referenced_table_schema,
          kcu.referenced_table_name,
          kcu.referenced_column_name,
          kcu.ordinal_position
        from information_schema.table_constraints tc
        left join information_schema.key_column_usage kcu
          on tc.constraint_name = kcu.constraint_name
         and tc.table_schema = kcu.table_schema
         and tc.table_name = kcu.table_name
        where tc.table_schema = ?
          and tc.table_name = ?
        order by tc.constraint_name, kcu.ordinal_position
        "#,
    )
    .bind(&schema_name)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;
    let mut grouped_constraints: std::collections::BTreeMap<
        String,
        (String, Vec<String>, Vec<String>),
    > = std::collections::BTreeMap::new();
    for row in constraint_rows {
        let constraint_name = row
            .try_get::<String, _>("constraint_name")
            .map_err(DatabaseError::MySql)?;
        let constraint_type = row
            .try_get::<String, _>("constraint_type")
            .unwrap_or_else(|_| "CONSTRAINT".to_string());
        let column_name = row
            .try_get::<Option<String>, _>("column_name")
            .ok()
            .flatten();
        let referenced_schema = row
            .try_get::<Option<String>, _>("referenced_table_schema")
            .ok()
            .flatten();
        let referenced_table = row
            .try_get::<Option<String>, _>("referenced_table_name")
            .ok()
            .flatten();
        let referenced_column = row
            .try_get::<Option<String>, _>("referenced_column_name")
            .ok()
            .flatten();

        let entry = grouped_constraints.entry(constraint_name).or_insert((
            constraint_type,
            Vec::new(),
            Vec::new(),
        ));
        if let Some(column_name) = column_name {
            entry.1.push(column_name);
        }
        if let Some(referenced_table) = referenced_table {
            let referenced_schema = referenced_schema.unwrap_or_default();
            let referenced_column = referenced_column.unwrap_or_default();
            entry.2.push(if referenced_schema.is_empty() {
                format!("{referenced_table}.{referenced_column}")
            } else {
                format!("{referenced_schema}.{referenced_table}.{referenced_column}")
            });
        }
    }
    for (constraint_name, (constraint_type, columns, references)) in grouped_constraints {
        rows.push(structure_row(
            "constraint",
            constraint_name,
            constraint_type,
            columns.join(", "),
            references.join(", "),
        ));
    }

    let trigger_rows = sqlx::query(
        r#"
        select trigger_name, action_timing, event_manipulation, action_statement
        from information_schema.triggers
        where trigger_schema = ?
          and event_object_schema = ?
          and event_object_table = ?
        order by trigger_name
        "#,
    )
    .bind(&schema_name)
    .bind(&schema_name)
    .bind(&table)
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;
    for row in trigger_rows {
        let trigger_name = row
            .try_get::<String, _>("trigger_name")
            .map_err(DatabaseError::MySql)?;
        let timing = row
            .try_get::<String, _>("action_timing")
            .unwrap_or_else(|_| String::new());
        let event = row
            .try_get::<String, _>("event_manipulation")
            .unwrap_or_else(|_| String::new());
        let action = row
            .try_get::<String, _>("action_statement")
            .unwrap_or_else(|_| String::new());
        rows.push(structure_row(
            "trigger",
            trigger_name,
            join_non_empty([
                (!timing.is_empty()).then_some(timing),
                (!event.is_empty()).then_some(event),
            ]),
            String::new(),
            action,
        ));
    }

    Ok(QueryOutput::Table(structure_page(rows)))
}

async fn load_mysql_table_columns(
    pool: sqlx::MySqlPool,
    schema: Option<String>,
    table: String,
) -> Result<Vec<String>, DatabaseError> {
    let schema_name = mysql_effective_schema_name(&pool, schema.as_deref()).await?;
    let rows = sqlx::query(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = ?
          and table_name = ?
        order by ordinal_position
        "#,
    )
    .bind(schema_name)
    .bind(table)
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;

    rows.into_iter()
        .map(|row| {
            row.try_get::<String, _>("column_name")
                .map_err(DatabaseError::MySql)
        })
        .collect()
}

async fn load_mysql_connection_tree(
    pool: sqlx::MySqlPool,
) -> Result<Vec<ExplorerNode>, DatabaseError> {
    let rows = sqlx::query(
        r#"
        select table_schema, table_name, table_type
        from information_schema.tables
        where table_schema not in ('information_schema', 'performance_schema', 'sys')
        order by table_schema, table_type, table_name
        "#,
    )
    .fetch_all(&pool)
    .await
    .map_err(DatabaseError::MySql)?;

    let mut grouped: std::collections::BTreeMap<String, Vec<ExplorerNode>> =
        std::collections::BTreeMap::new();

    for row in rows {
        let schema = row
            .try_get::<String, _>("table_schema")
            .map_err(DatabaseError::MySql)?;
        let name = row
            .try_get::<String, _>("table_name")
            .map_err(DatabaseError::MySql)?;
        let table_type = row
            .try_get::<String, _>("table_type")
            .map_err(DatabaseError::MySql)?;

        let kind = if table_type.eq_ignore_ascii_case("view") {
            ExplorerNodeKind::View
        } else {
            ExplorerNodeKind::Table
        };
        let qualified_name = qualified_mysql_table_name(&schema, &name);

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

async fn mysql_effective_schema_name(
    pool: &sqlx::MySqlPool,
    schema: Option<&str>,
) -> Result<String, DatabaseError> {
    if let Some(schema) = schema.map(str::trim).filter(|schema| !schema.is_empty()) {
        return Ok(schema.to_string());
    }

    sqlx::query_scalar::<_, Option<String>>("select database()")
        .fetch_one(pool)
        .await
        .map_err(DatabaseError::MySql)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DatabaseError::UnsupportedDriver(
                "No MySQL database selected. Set a default database or use a qualified table name."
                    .to_string(),
            )
        })
}

fn qualified_mysql_table_name(schema_name: &str, table_name: &str) -> String {
    format!(
        "{}.{}",
        quote_clickhouse_identifier(schema_name),
        quote_clickhouse_identifier(table_name)
    )
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

fn mysql_column_details(is_nullable: &str, default_value: Option<String>, extra: &str) -> String {
    join_non_empty([
        is_nullable
            .eq_ignore_ascii_case("NO")
            .then(|| "NOT NULL".to_string()),
        default_value.map(|value| format!("default {value}")),
        (!extra.trim().is_empty()).then(|| extra.trim().to_string()),
    ])
}

fn clickhouse_relation_supports_preview(engine: &str, create_table_query: &str) -> bool {
    let normalized_engine = engine.trim().to_ascii_lowercase();
    if matches!(
        normalized_engine.as_str(),
        "kafka" | "rabbitmq" | "nats" | "s3queue" | "azurequeue" | "redis"
    ) {
        return false;
    }

    if normalized_engine == "materializedview" {
        return !clickhouse_materialized_view_targets_table(create_table_query);
    }

    true
}

fn clickhouse_materialized_view_targets_table(create_table_query: &str) -> bool {
    let normalized = create_table_query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();
    if !normalized.starts_with("CREATE MATERIALIZED VIEW ") {
        return false;
    }

    let definition_head = normalized
        .split_once(" AS ")
        .map(|(head, _)| head)
        .unwrap_or(&normalized);
    definition_head.contains(" TO ")
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

#[cfg(test)]
mod tests {
    use super::{clickhouse_materialized_view_targets_table, clickhouse_relation_supports_preview};

    #[test]
    fn hides_stream_like_clickhouse_engines_from_preview_tree() {
        assert!(!clickhouse_relation_supports_preview(
            "Kafka",
            "CREATE TABLE dwh_ogs.source_statistics_kafka ENGINE = Kafka"
        ));
        assert!(!clickhouse_relation_supports_preview(
            "RabbitMQ",
            "CREATE TABLE dwh_ogs.source_statistics_queue ENGINE = RabbitMQ"
        ));
    }

    #[test]
    fn hides_materialized_views_with_to_target_from_preview_tree() {
        assert!(clickhouse_materialized_view_targets_table(
            "CREATE MATERIALIZED VIEW dwh_ogs.mv TO dwh_ogs.target AS SELECT * FROM dwh_ogs.src"
        ));
        assert!(!clickhouse_relation_supports_preview(
            "MaterializedView",
            "CREATE MATERIALIZED VIEW dwh_ogs.mv TO dwh_ogs.target AS SELECT * FROM dwh_ogs.src"
        ));
    }

    #[test]
    fn keeps_regular_clickhouse_views_and_tables_previewable() {
        assert!(clickhouse_relation_supports_preview(
            "MergeTree",
            "CREATE TABLE dwh_ogs.source_statistics ENGINE = MergeTree ORDER BY tuple()"
        ));
        assert!(clickhouse_relation_supports_preview(
            "View",
            "CREATE VIEW dwh_ogs.source_statistics_view AS SELECT * FROM dwh_ogs.source_statistics"
        ));
        assert!(clickhouse_relation_supports_preview(
            "MaterializedView",
            "CREATE MATERIALIZED VIEW dwh_ogs.mv ENGINE = MergeTree ORDER BY tuple() AS SELECT 1"
        ));
    }
}
