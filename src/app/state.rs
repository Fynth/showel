// src/app/state.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simple database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub database_path: String,
    pub created_at: DateTime<Utc>,
}

impl ConnectionConfig {
    pub fn new_sqlite(path: String) -> Self {
        Self {
            name: format!(
                "SQLite: {}",
                std::path::Path::new(&path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            ),
            database_path: path,
            created_at: Utc::now(),
        }
    }
}

/// Database connection state
#[derive(Debug, Clone)]
pub struct DatabaseConnection {
    pub config: ConnectionConfig,
    pub is_connected: bool,
    pub last_error: Option<String>,
}

/// Query execution result
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub success: bool,
    pub data: Option<Vec<HashMap<String, String>>>,
    pub execution_time_ms: u128,
    pub error_message: Option<String>,
    pub query: String,
    pub executed_at: DateTime<Utc>,
}

/// Application tabs with Default trait
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ActiveTab {
    #[default]
    QueryEditor,
    Schema,
    Tables,
    History,
    Settings,
}

/// Main application state - minimal working version
#[derive(Default)]
pub struct App {
    pub connections: Vec<DatabaseConnection>,
    pub current_connection_id: Option<usize>,
    pub current_query: String,
    pub query_results: Vec<QueryResult>,
    pub active_tab: ActiveTab,
    pub theme: String,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub auto_commit: bool,
    pub show_execution_time: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: ActiveTab::QueryEditor,
            theme: "Light".to_string(),
            auto_commit: false,
            show_execution_time: true,
            ..Default::default()
        }
    }

    /// Get current active connection
    pub fn current_connection(&self) -> Option<&DatabaseConnection> {
        self.current_connection_id.and_then(|index| {
            if index < self.connections.len() {
                self.connections.get(index)
            } else {
                None
            }
        })
    }

    /// Get mutable reference to current connection
    pub fn current_connection_mut(&mut self) -> Option<&mut DatabaseConnection> {
        if let Some(index) = self.current_connection_id {
            self.connections.get_mut(index)
        } else {
            None
        }
    }

    /// Add new connection
    pub fn add_connection(&mut self, config: ConnectionConfig) {
        self.connections.push(DatabaseConnection {
            config,
            is_connected: false,
            last_error: None,
        });
    }

    /// Set active connection by index
    pub fn set_current_connection(&mut self, index: usize) {
        if index < self.connections.len() {
            self.current_connection_id = Some(index);
        }
    }

    /// Show error message
    pub fn show_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.info_message = None; // Clear info when showing error
    }

    /// Show info message
    pub fn show_info(&mut self, message: String) {
        self.info_message = Some(message);
        self.error_message = None; // Clear error when showing info
    }

    /// Clear messages
    pub fn clear_messages(&mut self) {
        self.error_message = None;
        self.info_message = None;
    }

    /// Set active tab
    pub fn set_active_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
    }

    /// Execute current query (mock implementation with sample data)
    pub fn execute_query(&mut self) {
        // Clear previous messages
        self.clear_messages();

        // Check connection
        if self.current_connection().is_none() {
            self.show_error("No active database connection".to_string());
            return;
        }

        // Check query
        if self.current_query.trim().is_empty() {
            self.show_error("Please enter a SQL query".to_string());
            return;
        }

        // Mock query execution with sample data
        let result = QueryResult {
            success: true,
            data: Some(vec![
                {
                    let mut row = HashMap::new();
                    row.insert("id".to_string(), "1".to_string());
                    row.insert("name".to_string(), "Alice Johnson".to_string());
                    row.insert("email".to_string(), "alice@example.com".to_string());
                    row.insert("created_at".to_string(), "2024-01-15 10:30:00".to_string());
                    row
                },
                {
                    let mut row = HashMap::new();
                    row.insert("id".to_string(), "2".to_string());
                    row.insert("name".to_string(), "Bob Smith".to_string());
                    row.insert("email".to_string(), "bob@example.com".to_string());
                    row.insert("created_at".to_string(), "2024-01-16 14:22:00".to_string());
                    row
                },
                {
                    let mut row = HashMap::new();
                    row.insert("id".to_string(), "3".to_string());
                    row.insert("name".to_string(), "Carol White".to_string());
                    row.insert("email".to_string(), "carol@example.com".to_string());
                    row.insert("created_at".to_string(), "2024-01-17 09:15:00".to_string());
                    row
                },
            ]),
            execution_time_ms: 12,
            error_message: None,
            query: self.current_query.clone(),
            executed_at: Utc::now(),
        };

        self.query_results.insert(0, result);
        self.show_info("Query executed successfully".to_string());
    }
}
