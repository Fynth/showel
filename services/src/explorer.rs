use models::{DatabaseConnection, DatabaseError, ExplorerNode, ExplorerNodeKind, QueryOutput};
use sqlx::Row;

use crate::query::{postgres_rows_to_page, sqlite_rows_to_page};

pub async fn describe_table(
    connection: DatabaseConnection,
    schema: Option<String>,
    table: String,
) -> Result<QueryOutput, DatabaseError> {
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

            Ok(QueryOutput::Table(sqlite_rows_to_page(rows)))
        }
        DatabaseConnection::Postgres(pool) => {
            let schema_name = schema.unwrap_or_else(|| "public".to_string());
            let rows = sqlx::query(
                r#"
                select column_name, data_type, is_nullable
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

            Ok(QueryOutput::Table(postgres_rows_to_page(rows)))
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
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
