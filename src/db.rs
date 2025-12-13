use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_postgres::{Client, Config, NoTls, Row};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "postgres".to_string(),
            user: "postgres".to_string(),
            password: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct DatabaseConnection {
    client: Arc<Mutex<Option<Client>>>,
    config: Arc<Mutex<ConnectionConfig>>,
    pub cancelled: Arc<Mutex<bool>>,
}

impl DatabaseConnection {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(ConnectionConfig::default())),
            cancelled: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn connect(&self, config: ConnectionConfig) -> Result<()> {
        let mut pg_config = Config::new();
        pg_config
            .host(&config.host)
            .port(config.port)
            .dbname(&config.database)
            .user(&config.user)
            .password(&config.password);

        let (client, connection) = pg_config
            .connect(NoTls)
            .await
            .context("Failed to connect to database")?;

        // Spawn connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        *self.client.lock().await = Some(client);
        *self.config.lock().await = config;

        Ok(())
    }

    pub async fn disconnect(&self) {
        *self.client.lock().await = None;
    }

    pub async fn is_connected(&self) -> bool {
        self.client.lock().await.is_some()
    }

    pub async fn get_config(&self) -> ConnectionConfig {
        self.config.lock().await.clone()
    }

    pub async fn cancel(&self) {
        *self.cancelled.lock().await = true;
    }

    pub async fn reset_cancel(&self) {
        *self.cancelled.lock().await = false;
    }


    pub async fn get_databases(&self) -> Result<Vec<String>> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let rows = client
            .query(
                "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
                &[],
            )
            .await?;

        Ok(rows.iter().map(|row| row.get(0)).collect())
    }

    pub async fn get_schemas(&self) -> Result<Vec<String>> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let rows = client
            .query(
                "SELECT schema_name FROM information_schema.schemata
                 WHERE schema_name NOT IN ('pg_catalog', 'information_schema')
                 ORDER BY schema_name",
                &[],
            )
            .await?;

        Ok(rows.iter().map(|row| row.get(0)).collect())
    }

    pub async fn get_tables(&self, schema: &str) -> Result<Vec<String>> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let rows = client
            .query(
                "SELECT table_name FROM information_schema.tables
                 WHERE table_schema = $1 AND table_type = 'BASE TABLE'
                 ORDER BY table_name",
                &[&schema],
            )
            .await?;

        Ok(rows.iter().map(|row| row.get(0)).collect())
    }


    pub async fn get_column_types(&self, schema: &str, table: &str) -> Result<Vec<(String, String)>> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let rows = client
            .query(
                "SELECT column_name, data_type
                 FROM information_schema.columns
                 WHERE table_schema = $1 AND table_name = $2
                 ORDER BY ordinal_position",
                &[&schema, &table],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| (row.get::<_, String>(0), row.get::<_, String>(1)))
            .collect())
    }

    pub async fn execute_query(&self, query: &str) -> Result<QueryResult> {
        // Check if cancelled before starting
        if *self.cancelled.lock().await {
            return Err(anyhow::anyhow!("Query cancelled"));
        }

        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let query = query.trim();

        // Check if it's a SELECT query or other
        if query.to_uppercase().starts_with("SELECT")
            || query.to_uppercase().starts_with("WITH")
            || query.to_uppercase().starts_with("SHOW")
        {
            // For SELECT queries, we can't easily cancel mid-execution with tokio-postgres
            // But we can check cancellation before and after
            if *self.cancelled.lock().await {
                return Err(anyhow::anyhow!("Query cancelled"));
            }

            let rows = client.query(query, &[]).await?;

            if *self.cancelled.lock().await {
                return Err(anyhow::anyhow!("Query cancelled"));
            }

            if rows.is_empty() {
                return Ok(QueryResult {
                    columns: vec![],
                    rows: vec![],
                    affected_rows: 0,
                });
            }

            let columns: Vec<String> = rows[0]
                .columns()
                .iter()
                .map(|col| col.name().to_string())
                .collect();

            let data: Vec<Vec<String>> = rows
                .iter()
                .map(|row| (0..row.len()).map(|i| format_value(row, i)).collect())
                .collect();

            Ok(QueryResult {
                columns,
                rows: data,
                affected_rows: 0,
            })
        } else {
            // For non-SELECT queries, check cancellation before execution
            if *self.cancelled.lock().await {
                return Err(anyhow::anyhow!("Query cancelled"));
            }

            let affected = client.execute(query, &[]).await?;
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: affected as usize,
            })
        }
    }



    pub async fn get_table_data(
        &self,
        schema: &str,
        table: &str,
        limit: i64,
        offset: i64,
        sort_column: Option<&str>,
        sort_ascending: bool,
    ) -> Result<QueryResult> {
        let mut query = format!("SELECT * FROM {}.{}", escape_identifier(schema), escape_identifier(table));

        if let Some(col) = sort_column {
            let direction = if sort_ascending { "ASC" } else { "DESC" };
            query.push_str(&format!(" ORDER BY {} {}", col, direction));
        }

        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        self.execute_query(&query).await
    }

    pub async fn get_table_row_count(&self, schema: &str, table: &str) -> Result<i64> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let query = format!("SELECT COUNT(*) FROM {}.{}", escape_identifier(schema), escape_identifier(table));
        let row = client.query_one(&query, &[]).await?;
        Ok(row.get(0))
    }

    pub async fn update_cell(
        &self,
        schema: &str,
        table: &str,
        column: &str,
        new_value: &str,
        row_data: &[String],
        columns: &[String],
    ) -> Result<()> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        // Try to find primary key
        let pk_query = format!(
            "SELECT a.attname FROM pg_index i
             JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
             WHERE i.indrelid = '{}.{}'::regclass AND i.indisprimary",
            schema, table
        );

        let pk_column = match client.query(&pk_query, &[]).await {
            Ok(rows) if !rows.is_empty() => rows[0].get::<_, String>(0),
            _ => {
                // Fallback to first column (usually 'id')
                columns.first().context("No columns available")?.clone()
            }
        };

        // Find the index of primary key column
        let pk_index = columns.iter().position(|c| c == &pk_column)
            .unwrap_or(0);
        let pk_value = row_data.get(pk_index).context("No primary key value")?;

        // Handle NULL values
        let query = if new_value.to_uppercase() == "NULL" {
            format!(
                "UPDATE {}.{} SET {} = NULL WHERE {} = $2",
                escape_identifier(schema),
                escape_identifier(table),
                escape_identifier(column),
                escape_identifier(&pk_column)
            )
        } else {
            format!(
                "UPDATE {}.{} SET {} = $1 WHERE {} = $2",
                escape_identifier(schema),
                escape_identifier(table),
                escape_identifier(column),
                escape_identifier(&pk_column)
            )
        };

        if new_value.to_uppercase() == "NULL" {
            client.execute(&query, &[&pk_value]).await?;
        } else {
            client.execute(&query, &[&new_value, &pk_value]).await?;
        }
        Ok(())
    }
}


#[derive(Debug, Clone, Default)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub affected_rows: usize,
}



fn format_value(row: &Row, index: usize) -> String {
    let column = &row.columns()[index];
    let type_name = column.type_().name();

    match type_name {
        "int2" | "int4" | "int8" => {
            if let Ok(val) = row.try_get::<_, i16>(index) {
                return val.to_string();
            }
            if let Ok(val) = row.try_get::<_, i32>(index) {
                return val.to_string();
            }
            if let Ok(val) = row.try_get::<_, i64>(index) {
                return val.to_string();
            }
        }
        "float4" | "float8" | "numeric" => {
            if let Ok(val) = row.try_get::<_, f64>(index) {
                return val.to_string();
            }
        }
        "bool" | "boolean" => {
            if let Ok(val) = row.try_get::<_, bool>(index) {
                return val.to_string();
            }
        }
        "varchar" | "text" | "char" | "bpchar" => {
            if let Ok(val) = row.try_get::<_, String>(index) {
                return val;
            }
        }
        _ => {}
    }

    // Try as string
    if let Ok(val) = row.try_get::<_, String>(index) {
        return val;
    }

    // If all else fails
    "NULL".to_string()
}

fn escape_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace("\"", "\"\""))
}
