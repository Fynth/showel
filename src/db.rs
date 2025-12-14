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
    // Stores the backend PID for the currently running query so we can request server-side cancel.
    current_backend_pid: Arc<Mutex<Option<i32>>>,
}

impl DatabaseConnection {
    pub fn new() -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(ConnectionConfig::default())),
            cancelled: Arc::new(Mutex::new(false)),
            current_backend_pid: Arc::new(Mutex::new(None)),
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

        // Try a plain (non-TLS) connection. If it fails, provide a clearer hint.
        let (client, connection) = pg_config
            .connect(NoTls)
            .await
            .map_err(|e| {
                // Improve message: hint that the server may require SSL/TLS.
                anyhow::anyhow!(
                    "Failed to connect to database at {}:{}/{} as {}: {}. \
If the server requires SSL/TLS, enable TLS support or connect with an SSL-capable client.",
                    config.host, config.port, config.database, config.user, e
                )
            })?;

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
        // Mark client-side cancelled flag
        *self.cancelled.lock().await = true;

        // Try server-side cancellation using stored backend PID.
        let pid_opt = { *self.current_backend_pid.lock().await };
        if let Some(pid) = pid_opt {
            // Clone config to use for creating a temporary control connection
            let cfg = self.config.lock().await.clone();

            let mut pg_cfg = Config::new();
            pg_cfg
                .host(&cfg.host)
                .port(cfg.port)
                .dbname(&cfg.database)
                .user(&cfg.user)
                .password(&cfg.password);

            // Create a short-lived connection to call pg_cancel_backend(pid)
            match pg_cfg.connect(NoTls).await {
                Ok((client, connection)) => {
                    // Spawn the connection handler so the client can be used immediately
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            eprintln!("Cancel connection error: {}", e);
                        }
                    });

                    // Execute cancel command; ignore errors but log them
                    match client.query_one("SELECT pg_cancel_backend($1)", &[&pid]).await {
                        Ok(row) => {
                            // pg_cancel_backend returns bool - true when cancellation was requested
                            if let Ok(cancelled) = row.try_get::<_, bool>(0) {
                                if !cancelled {
                                    eprintln!("pg_cancel_backend returned false for pid {}", pid);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to call pg_cancel_backend: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create cancel connection: {}", e);
                }
            }
        }
    }

    pub async fn reset_cancel(&self) {
        *self.cancelled.lock().await = false;
        // Clear any stored backend PID
        *self.current_backend_pid.lock().await = None;
    }

    pub async fn begin_transaction(&self) -> Result<()> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        client.execute("BEGIN", &[]).await?;
        Ok(())
    }

    pub async fn commit_transaction(&self) -> Result<()> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        client.execute("COMMIT", &[]).await?;
        Ok(())
    }

    pub async fn rollback_transaction(&self) -> Result<()> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        client.execute("ROLLBACK", &[]).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn is_in_transaction(&self) -> Result<bool> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let row = client.query_one(
            "SELECT COUNT(*) > 0 AS in_transaction FROM pg_stat_activity WHERE pid = pg_backend_pid() AND state = 'active' AND query LIKE '%BEGIN%'",
            &[],
        ).await?;

        Ok(row.get(0))
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

    pub async fn get_table_info(&self, schema: &str, table: &str) -> Result<TableInfo> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        // Get basic table info
        let table_info_row = client
            .query_one(
                "SELECT table_name, table_type, table_schema
                 FROM information_schema.tables
                 WHERE table_schema = $1 AND table_name = $2",
                &[&schema, &table],
            )
            .await?;

        let table_name = table_info_row.get::<_, String>(0);
        let table_type = table_info_row.get::<_, String>(1);
        let table_schema = table_info_row.get::<_, String>(2);

        // Get column details
        let columns = client
            .query(
                "SELECT column_name, data_type, is_nullable, column_default
                 FROM information_schema.columns
                 WHERE table_schema = $1 AND table_name = $2
                 ORDER BY ordinal_position",
                &[&schema, &table],
            )
            .await?;

        let column_details = columns
            .iter()
            .map(|row| {
                let column_name = row.get::<_, String>(0);
                let data_type = row.get::<_, String>(1);
                let is_nullable = row.get::<_, String>(2) == "YES";
                let column_default = row.get::<_, Option<String>>(3);

                ColumnInfo {
                    name: column_name,
                    data_type,
                    is_nullable,
                    default_value: column_default,
                }
            })
            .collect();

        // Get primary key
        let primary_key = client
            .query(
                "SELECT a.attname
                 FROM pg_index i
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
                 WHERE i.indrelid = $1::regclass AND i.indisprimary",
                &[&format!("{}.{}", schema, table)],
            )
            .await
            .unwrap_or_default();

        let primary_key_columns = primary_key
            .iter()
            .map(|row| row.get::<_, String>(0))
            .collect();

        // Get foreign keys
        let foreign_keys = client
            .query(
                "SELECT tc.constraint_name, kcu.column_name,
                        ccu.table_schema AS foreign_table_schema,
                        ccu.table_name AS foreign_table_name,
                        ccu.column_name AS foreign_column_name
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage kcu
                   ON tc.constraint_name = kcu.constraint_name
                 JOIN information_schema.constraint_column_usage ccu
                   ON ccu.constraint_name = tc.constraint_name
                 WHERE tc.constraint_type = 'FOREIGN KEY'
                   AND tc.table_schema = $1 AND tc.table_name = $2",
                &[&schema, &table],
            )
            .await
            .unwrap_or_default();

        let foreign_key_details = foreign_keys
            .iter()
            .map(|row| {
                ForeignKeyInfo {
                    name: row.get::<_, String>(0),
                    column: row.get::<_, String>(1),
                    foreign_schema: row.get::<_, String>(2),
                    foreign_table: row.get::<_, String>(3),
                    foreign_column: row.get::<_, String>(4),
                }
            })
            .collect();

        // Get indexes
        let indexes = client
            .query(
                "SELECT indexname, indexdef
                 FROM pg_indexes
                 WHERE schemaname = $1 AND tablename = $2
                 ORDER BY indexname",
                &[&schema, &table],
            )
            .await
            .unwrap_or_default();

        let index_details = indexes
            .iter()
            .map(|row| {
                IndexInfo {
                    name: row.get::<_, String>(0),
                    definition: row.get::<_, String>(1),
                }
            })
            .collect();

        Ok(TableInfo {
            name: table_name,
            schema: table_schema,
            table_type,
            columns: column_details,
            primary_key: primary_key_columns,
            foreign_keys: foreign_key_details,
            indexes: index_details,
        })
    }

    pub async fn search_objects(&self, search_term: &str) -> Result<Vec<SearchResult>> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let search_pattern = format!("%{}%", search_term);

        // Search tables
        let tables = client
            .query(
                "SELECT table_schema, table_name, 'table' as object_type
                 FROM information_schema.tables
                 WHERE (table_schema ILIKE $1 OR table_name ILIKE $1)
                 AND table_type = 'BASE TABLE'
                 ORDER BY table_schema, table_name",
                &[&search_pattern],
            )
            .await?;

        // Search columns
        let columns = client
            .query(
                "SELECT table_schema, table_name, column_name, 'column' as object_type
                 FROM information_schema.columns
                 WHERE (table_schema ILIKE $1 OR table_name ILIKE $1 OR column_name ILIKE $1)
                 ORDER BY table_schema, table_name, column_name",
                &[&search_pattern],
            )
            .await?;

        // Search views
        let views = client
            .query(
                "SELECT table_schema, table_name, 'view' as object_type
                 FROM information_schema.views
                 WHERE (table_schema ILIKE $1 OR table_name ILIKE $1)
                 ORDER BY table_schema, table_name",
                &[&search_pattern],
            )
            .await?;

        let mut results = Vec::new();

        for row in tables {
            results.push(SearchResult {
                schema: row.get(0),
                name: row.get(1),
                object_type: row.get(2),
                column_name: None,
            });
        }

        for row in columns {
            results.push(SearchResult {
                schema: row.get(0),
                name: row.get(1),
                object_type: row.get(3),
                column_name: Some(row.get(2)),
            });
        }

        for row in views {
            results.push(SearchResult {
                schema: row.get(0),
                name: row.get(1),
                object_type: row.get(2),
                column_name: None,
            });
        }

        Ok(results)
    }

    pub async fn execute_query(&self, query: &str) -> Result<QueryResult> {
        // Check if cancelled before starting
        if *self.cancelled.lock().await {
            return Err(anyhow::anyhow!("Query cancelled"));
        }

        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let query = query.trim();

        // Attempt to capture the backend PID for server-side cancellation.
        if let Ok(row) = client.query_one("SELECT pg_backend_pid()", &[]).await {
            if let Ok(pid) = row.try_get::<_, i32>(0) {
                *self.current_backend_pid.lock().await = Some(pid);
            }
        }

        // Check if it's a SELECT query or other
        if query.to_uppercase().starts_with("SELECT")
            || query.to_uppercase().starts_with("WITH")
            || query.to_uppercase().starts_with("SHOW")
        {
            // For SELECT queries, check cancellation before and after
            if *self.cancelled.lock().await {
                *self.current_backend_pid.lock().await = None;
                return Err(anyhow::anyhow!("Query cancelled"));
            }

            let rows = client.query(query, &[]).await?;

            if *self.cancelled.lock().await {
                *self.current_backend_pid.lock().await = None;
                return Err(anyhow::anyhow!("Query cancelled"));
            }

            if rows.is_empty() {
                *self.current_backend_pid.lock().await = None;
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

            // Clear stored backend pid before returning
            *self.current_backend_pid.lock().await = None;
            Ok(QueryResult {
                columns,
                rows: data,
                affected_rows: 0,
            })
        } else {
            // For non-SELECT queries, check cancellation before execution
            if *self.cancelled.lock().await {
                *self.current_backend_pid.lock().await = None;
                return Err(anyhow::anyhow!("Query cancelled"));
            }

            let affected = client.execute(query, &[]).await?;
            // Clear stored backend pid before returning
            *self.current_backend_pid.lock().await = None;
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: affected as usize,
            })
        }
    }



    #[allow(clippy::too_many_arguments)]
    pub async fn get_table_data(
        &self,
        schema: &str,
        table: &str,
        limit: i64,
        offset: i64,
        sort_column: Option<&str>,
        sort_ascending: bool,
        where_clause: Option<&str>,
    ) -> Result<QueryResult> {
        let mut query = format!("SELECT * FROM {}.{}", escape_identifier(schema), escape_identifier(table));

        if let Some(where_clause) = where_clause {
            if !where_clause.trim().is_empty() {
                query.push_str(&format!(" WHERE {}", where_clause));
            }
        }

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

    pub async fn get_table_row_count_with_filter(&self, schema: &str, table: &str, where_clause: &str) -> Result<i64> {
        let client = self.client.lock().await;
        let client = client.as_ref().context("Not connected to database")?;

        let query = format!("SELECT COUNT(*) FROM {}.{} WHERE {}", escape_identifier(schema), escape_identifier(table), where_clause);
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

        // Try to find primary key (use parameterized query to avoid injection and quoting issues)
        let pk_rows = client
            .query(
                "SELECT a.attname FROM pg_index i
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
                 WHERE i.indrelid = $1::regclass AND i.indisprimary",
                &[&format!("{}.{}", schema, table)],
            )
            .await
            .unwrap_or_default();

        let pk_column = if !pk_rows.is_empty() {
            pk_rows[0].get::<_, String>(0)
        } else {
            // Fallback to first column (usually 'id')
            columns.first().context("No columns available")?.clone()
        };

        // Find the index of primary key column
        let pk_index = columns.iter().position(|c| c == &pk_column)
            .unwrap_or(0);
        let pk_value = row_data.get(pk_index).context("No primary key value")?;

        // Handle NULL values
        // Use $1 for primary key in NULL-case and ($1 = new_value, $2 = pk) for non-NULL to maintain parameter order
        let query = if new_value.to_uppercase() == "NULL" {
            format!(
                "UPDATE {}.{} SET {} = NULL WHERE {} = $1",
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
            // For NULL case the only parameter is the primary key value
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

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyInfo {
    pub name: String,
    pub column: String,
    pub foreign_schema: String,
    pub foreign_table: String,
    pub foreign_column: String,
}

#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub definition: String,
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub table_type: String,
    pub columns: Vec<ColumnInfo>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub schema: String,
    pub name: String,
    pub object_type: String,
    pub column_name: Option<String>,
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
