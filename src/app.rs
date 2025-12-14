#![allow(dead_code, clippy::too_many_arguments)]
use eframe::egui;
use egui::TextEdit;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::db::{ConnectionConfig, DatabaseConnection, QueryResult};
use crate::ui::{ConnectionDialog, DatabaseTree, QueryEditor, ResultsTable, TreeAction};

enum DbCommand {
    Connect(ConnectionConfig),
    Disconnect,
    GetDatabases,
    GetSchemas,
    GetTables(String),
    GetColumnTypes(String, String), // schema, table
    GetTableInfo(String, String), // schema, table
    SearchObjects(String), // search term
    ExecuteQuery(String),
    CancelQuery,
    ResetCancel,
    LoadTableData(String, String, i64, i64, Option<String>, bool, Option<String>), // schema, table, limit, offset, sort_column, sort_ascending, where_clause
    CheckConnection,
    UpdateCell(String, String, String, String, Vec<String>, Vec<String>), // schema, table, column, value, row_data, columns
    BeginTransaction,
    CommitTransaction,
    RollbackTransaction,

}

enum DbResponse {
    Connected,
    Disconnected,
    ConnectionError(String),
    Databases(Vec<String>),
    Schemas(Vec<String>),
    Tables(Vec<String>),
    ColumnTypes(Vec<(String, String)>), // (column_name, data_type)
    TableInfo(crate::db::TableInfo),
    SearchResults(Vec<crate::db::SearchResult>),
    QueryResult(QueryResult),
    TableData(QueryResult, i64),
    Error(String),
    ConnectionStatus(bool, ConnectionConfig),
    CellUpdated,
    TransactionStarted,
    TransactionCommitted,
    TransactionRolledBack,
}

pub struct ShowelApp {
    // Connection
    connection_dialog: ConnectionDialog,
    show_connection_dialog: bool,
    connected: bool,
    connection_status: String,
    current_config: Option<ConnectionConfig>,

    // UI State
    database_tree: DatabaseTree,
    query_editor: QueryEditor,
    results_table: ResultsTable,
    edit_dialog: crate::ui::EditDialog,
    column_types: Vec<(String, String)>,

    // Channels
    command_tx: Sender<DbCommand>,
    response_rx: Receiver<DbResponse>,

    // Status
    status_message: String,
    error_message: Option<String>,

    // Table view
    current_schema: Option<String>,
    current_table: Option<String>,
    table_page: i64,
    table_page_size: i64,
    table_total_rows: i64,
    use_pagination: bool, // Toggle between pagination and virtual scrolling

    // Table metadata
    table_info: Option<crate::db::TableInfo>,
    show_table_info: bool,

    // Search functionality
    search_results: Vec<crate::db::SearchResult>,
    show_search_results: bool,
    search_query: String,

    // Data filtering
    table_filter: String,
    show_filter_dialog: bool,

    // Timer for periodic checks
    last_connection_check: std::time::Instant,

    // UI update flags
    need_repaint: bool,

    // Query execution
    is_query_running: bool,

    // Query history
    query_history: Vec<String>,
    show_query_history: bool,

    // Query favorites
    query_favorites: Vec<(String, String)>, // (name, query)
    show_query_favorites: bool,

    // Transaction management
    is_in_transaction: bool,

    // Connection management
    connections: Vec<ConnectionConfig>,
    active_connection_index: usize,
    show_connection_manager: bool,
    new_connection_dialog: ConnectionDialog,
}

impl ShowelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load custom fonts to support Cyrillic characters
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "DejaVuSans".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf")),
        );
        fonts.font_data.insert(
            "DejaVuSansMono".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/DejaVuSansMono.ttf")),
        );

        // Put my font first (highest priority) for proportional text:
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "DejaVuSans".to_owned());

        // Put my font first (highest priority) for monospace text:
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "DejaVuSansMono".to_owned());

        cc.egui_ctx.set_fonts(fonts);

        let (command_tx, command_rx) = channel::<DbCommand>();
        let (response_tx, response_rx) = channel::<DbResponse>();

        // Spawn database worker thread
        #[allow(clippy::while_let_loop)]
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let db = DatabaseConnection::new();

            loop {
                match command_rx.recv() {
                    Ok(command) => {
                        match command {
                            DbCommand::ExecuteQuery(query) => {
                                let response_tx_clone = response_tx.clone();
                                let db_clone = db.clone();
                                rt.spawn(async move {
                                    match db_clone.execute_query(&query).await {
                                        Ok(result) => {
                                            let _ = response_tx_clone.send(DbResponse::QueryResult(result));
                                        }
                                        Err(e) => {
                                            let _ = response_tx_clone.send(DbResponse::Error(e.to_string()));
                                        }
                                    }
                                });
                                // No response for ExecuteQuery, it's async
                                continue;
                            }
                            DbCommand::CancelQuery => {
                                rt.block_on(async {
                                    db.cancel().await;
                                });
                                let _ = response_tx.send(DbResponse::Error("Query cancelled".to_string()));
                            }
                            DbCommand::ResetCancel => {
                                rt.block_on(async {
                                    db.reset_cancel().await;
                                });
                            }
                            _ => {
                                let response = rt.block_on(async {
                                    match command {
                                        DbCommand::Connect(config) => match db.connect(config).await {
                                            Ok(_) => DbResponse::Connected,
                                            Err(e) => DbResponse::ConnectionError(e.to_string()),
                                        },
                                        DbCommand::Disconnect => {
                                            db.disconnect().await;
                                            DbResponse::Disconnected
                                        }
                                        DbCommand::GetDatabases => match db.get_databases().await {
                                            Ok(databases) => DbResponse::Databases(databases),
                                            Err(e) => DbResponse::Error(e.to_string()),
                                        },
                                        DbCommand::GetSchemas => match db.get_schemas().await {
                                            Ok(schemas) => DbResponse::Schemas(schemas),
                                            Err(e) => DbResponse::Error(e.to_string()),
                                        },
                                        DbCommand::GetTables(schema) => {
                                            match db.get_tables(&schema).await {
                                                Ok(tables) => DbResponse::Tables(tables),
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::GetColumnTypes(schema, table) => {
                                            match db.get_column_types(&schema, &table).await {
                                                Ok(types) => DbResponse::ColumnTypes(types),
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::GetTableInfo(schema, table) => {
                                            match db.get_table_info(&schema, &table).await {
                                                Ok(info) => DbResponse::TableInfo(info),
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::SearchObjects(search_term) => {
                                            match db.search_objects(&search_term).await {
                                                Ok(results) => DbResponse::SearchResults(results),
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::LoadTableData(schema, table, limit, offset, sort_column, sort_ascending, where_clause) => {
                                            let sort_col_ref = sort_column.as_deref();
                                            let where_clause_ref = where_clause.as_deref();
                                            match db.get_table_data(&schema, &table, limit, offset, sort_col_ref, sort_ascending, where_clause_ref).await {
                                                Ok(result) => {
                                                    // Also get row count (with filter if present)
                                                    let count = if let Some(where_clause) = where_clause_ref {
                                                        db.get_table_row_count_with_filter(&schema, &table, where_clause).await
                                                    } else {
                                                        db.get_table_row_count(&schema, &table).await
                                                    };

                                                    match count {
                                                        Ok(count) => DbResponse::TableData(result, count),
                                                        Err(e) => DbResponse::Error(e.to_string()),
                                                    }
                                                }
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }

                                        DbCommand::CheckConnection => {
                                            let is_connected = db.is_connected().await;
                                            let config = db.get_config().await;
                                            DbResponse::ConnectionStatus(is_connected, config)
                                        }
                                        DbCommand::UpdateCell(schema, table, column, value, row_data, columns) => {
                                            match db.update_cell(&schema, &table, &column, &value, &row_data, &columns).await {
                                                Ok(_) => DbResponse::CellUpdated,
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::BeginTransaction => {
                                            match db.begin_transaction().await {
                                                Ok(_) => DbResponse::TransactionStarted,
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::CommitTransaction => {
                                            match db.commit_transaction().await {
                                                Ok(_) => DbResponse::TransactionCommitted,
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }
                                        DbCommand::RollbackTransaction => {
                                            match db.rollback_transaction().await {
                                                Ok(_) => DbResponse::TransactionRolledBack,
                                                Err(e) => DbResponse::Error(e.to_string()),
                                            }
                                        }

                                        DbCommand::ResetCancel => {
                                            // ResetCancel may be sent from the UI thread; handle it here as well to keep match exhaustive.
                                            // Perform the reset and return a harmless dummy response.
                                            db.reset_cancel().await;
                                            DbResponse::Error("".to_string())
                                        }

                                        DbCommand::ExecuteQuery(_) | DbCommand::CancelQuery => unreachable!(),
                                    }
                                });

                                if response_tx.send(response).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            connection_dialog: ConnectionDialog::default(),
            show_connection_dialog: false,
            connected: false,
            connection_status: "Not connected".to_string(),
            current_config: None,

            database_tree: DatabaseTree::default(),
            query_editor: QueryEditor::default(),
            results_table: ResultsTable::default(),
            edit_dialog: crate::ui::EditDialog::default(),
            column_types: Vec::new(),

            command_tx,
            response_rx,

            status_message: "Ready".to_string(),
            error_message: None,

            current_schema: None,
            current_table: None,
            table_page: 0,
            table_page_size: 100,
            table_total_rows: 0,
            use_pagination: true, // Default to pagination mode
            last_connection_check: std::time::Instant::now(),
            need_repaint: false,
            is_query_running: false,
            query_history: Vec::new(),
            show_query_history: false,

            // Query favorites
            query_favorites: Vec::new(),
            show_query_favorites: false,

            // Transaction management
            is_in_transaction: false,

            // Table metadata
            table_info: None,
            show_table_info: false,

            // Data filtering
            table_filter: String::new(),
            show_filter_dialog: false,

            // Search functionality
            search_results: Vec::new(),
            show_search_results: false,
            search_query: String::new(),

            // Connection management
            connections: Vec::new(),
            active_connection_index: 0,
            show_connection_manager: false,
            new_connection_dialog: ConnectionDialog::default(),
        }
    }

    fn connect(&mut self, config: ConnectionConfig) {
        self.status_message = "Connecting...".to_string();
        self.error_message = None;
        self.need_repaint = true;

        // Store the config in case we need to retry
        self.current_config = Some(config.clone());

        let _ = self.command_tx.send(DbCommand::Connect(config));
    }

    fn disconnect(&mut self) {
        let _ = self.command_tx.send(DbCommand::Disconnect);
        self.connected = false;
        self.connection_status = "Not connected".to_string();
        self.database_tree = DatabaseTree::default();
        self.results_table = ResultsTable::default();
        self.current_schema = None;
        self.current_table = None;
        self.current_config = None;
        self.need_repaint = true;
    }

    fn load_databases(&mut self) {
        let _ = self.command_tx.send(DbCommand::GetDatabases);
    }

    fn load_schemas(&mut self) {
        let _ = self.command_tx.send(DbCommand::GetSchemas);
    }

    fn load_tables(&mut self, schema: String) {
        let _ = self.command_tx.send(DbCommand::GetTables(schema));
    }

    fn cancel_query(&mut self) {
        self.is_query_running = false;
        self.status_message = "Query cancelled".to_string();
        let _ = self.command_tx.send(DbCommand::CancelQuery);
    }

    fn execute_query(&mut self, query: String) {
        // Add to history if not already the most recent query
        let trimmed_query = query.trim();
        if !trimmed_query.is_empty() &&
           self.query_history.last().map(|last| last != trimmed_query).unwrap_or(true) {
            self.query_history.push(trimmed_query.to_string());
            // Keep only the last 50 queries
            if self.query_history.len() > 50 {
                self.query_history.remove(0);
            }
        }



        self.status_message = "Executing query...".to_string();
        self.error_message = None;
        self.is_query_running = true;
        // Reset cancellation flag for new query (send before starting the query)
        let _ = self.command_tx.send(DbCommand::ResetCancel);
        let _ = self.command_tx.send(DbCommand::ExecuteQuery(query));
    }



    fn load_table_data(&mut self, schema: String, table: String) {
        self.current_schema = Some(schema.clone());
        self.current_table = Some(table.clone());

        // Reset table state for new table
        self.results_table.columns.clear();
        self.results_table.rows.clear();
        self.results_table.loaded_rows = 0;
        self.results_table.total_rows = 0;

        // Load column types first
        let _ = self
            .command_tx
            .send(DbCommand::GetColumnTypes(schema.clone(), table.clone()));

        // Load first page of data
        self.load_more_table_data();
    }



    fn load_more_table_data(&mut self) {
        // Only stop loading if we know the total count and have loaded all rows
        if self.results_table.total_rows > 0 && self.results_table.loaded_rows >= self.results_table.total_rows as usize {
            return;
        }
        if let (Some(ref schema), Some(ref table)) = (&self.current_schema, &self.current_table) {
            let page_size = self.results_table.page_size as i64;
            let offset = self.results_table.loaded_rows as i64;

            // Get sort info
            let (sort_column, sort_ascending) = self.results_table.get_sort_info()
                .map(|(col, asc)| (Some(col), asc))
                .unwrap_or((None, true));

            // Apply filter if present
            let where_clause = if !self.table_filter.is_empty() {
                Some(self.table_filter.clone())
            } else {
                None
            };

            let _ = self
                .command_tx
                .send(DbCommand::LoadTableData(schema.clone(), table.clone(), page_size, offset, sort_column, sort_ascending, where_clause));
        }
    }

    fn check_connection(&mut self) {
        let _ = self.command_tx.send(DbCommand::CheckConnection);
    }

    fn update_cell(&mut self, schema: String, table: String, column: String, value: String, row_data: Vec<String>, columns: Vec<String>) {
        let _ = self.command_tx.send(DbCommand::UpdateCell(schema, table, column, value, row_data, columns));
    }

    fn load_table_info(&mut self, schema: String, table: String) {
        let _ = self.command_tx.send(DbCommand::GetTableInfo(schema, table));
    }

    fn search_objects(&mut self, search_term: String) {
        self.search_query = search_term.clone();
        let _ = self.command_tx.send(DbCommand::SearchObjects(search_term));
    }

    fn add_query_favorite(&mut self, name: String, query: String) {
        // Check if this query already exists in favorites
        if !self.query_favorites.iter().any(|(_, q)| q == &query) {
            self.query_favorites.push((name, query));
        }
    }

    fn remove_query_favorite(&mut self, index: usize) {
        if index < self.query_favorites.len() {
            self.query_favorites.remove(index);
        }
    }

    fn begin_transaction(&mut self) {
        let _ = self.command_tx.send(DbCommand::BeginTransaction);
    }

    fn commit_transaction(&mut self) {
        let _ = self.command_tx.send(DbCommand::CommitTransaction);
    }

    fn rollback_transaction(&mut self) {
        let _ = self.command_tx.send(DbCommand::RollbackTransaction);
    }

    #[allow(dead_code)]
    fn check_transaction_status(&mut self) {
        // Transaction status polling is not currently used.
    }

    fn add_connection(&mut self, config: ConnectionConfig) {
        self.connections.push(config);
    }

    fn switch_connection(&mut self, index: usize) {
        if index < self.connections.len() {
            self.active_connection_index = index;
            if let Some(config) = self.connections.get(index) {
                self.connect(config.clone());
            }
        }
    }

    fn remove_connection(&mut self, index: usize) {
        if index < self.connections.len() {
            self.connections.remove(index);
            if self.active_connection_index >= self.connections.len() && !self.connections.is_empty() {
                self.active_connection_index = self.connections.len() - 1;
            }
        }
    }

    fn show_table_info_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_table_info;

        egui::Window::new(format!("Table Info: {}",
            self.table_info.as_ref().map_or("Unknown".to_string(), |info| format!("{}.{}", info.schema, info.name))
        ))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            if let Some(info) = &self.table_info {
                ui.heading(format!("{}.{} ({})", info.schema, info.name, info.table_type));
                ui.separator();

                // Basic info
                ui.label(format!("Schema: {}", info.schema));
                ui.label(format!("Name: {}", info.name));
                ui.label(format!("Type: {}", info.table_type));

                if !info.primary_key.is_empty() {
                    ui.label(format!("Primary Key: {}", info.primary_key.join(", ")));
                }
                ui.separator();

                // Columns
                ui.heading("Columns");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("columns_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Type");
                            ui.label("Nullable");
                            ui.label("Default");
                            ui.end_row();

                            for col in &info.columns {
                                ui.label(&col.name);
                                ui.label(&col.data_type);
                                ui.label(if col.is_nullable { "YES" } else { "NO" });
                                ui.label(col.default_value.as_deref().unwrap_or("NULL"));
                                ui.end_row();
                            }
                        });
                });

                ui.separator();

                // Foreign Keys
                if !info.foreign_keys.is_empty() {
                    ui.heading("Foreign Keys");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("fk_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Name");
                                ui.label("Column");
                                ui.label("References");
                                ui.end_row();

                                for fk in &info.foreign_keys {
                                    ui.label(&fk.name);
                                    ui.label(&fk.column);
                                    ui.label(format!("{}.{}.{}", fk.foreign_schema, fk.foreign_table, fk.foreign_column));
                                    ui.end_row();
                                }
                            });
                    });
                    ui.separator();
                }

                // Indexes
                if !info.indexes.is_empty() {
                    ui.heading("Indexes");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("indexes_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Name");
                                ui.label("Definition");
                                ui.end_row();

                                for idx in &info.indexes {
                                    ui.label(&idx.name);
                                    ui.label(&idx.definition);
                                    ui.end_row();
                                }
                            });
                    });
                }
            }
        });

        self.show_table_info = open;
    }

    fn show_query_favorites_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_query_favorites;
        let mut new_favorite_name = String::new();
        let mut add_new_favorite = false;
        let mut remove_index: Option<usize> = None;
        let mut load_index: Option<usize> = None;

        egui::Window::new("Query Favorites")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([500.0, 400.0])
        .show(ctx, |ui| {
            ui.heading("Query Favorites");
            ui.separator();

            // Add new favorite section
            if !self.query_editor.sql.trim().is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut new_favorite_name);
                    if ui.button("Add Current Query").clicked() && !new_favorite_name.trim().is_empty() {
                        add_new_favorite = true;
                    }
                });
                ui.separator();
            }

            // List of favorites
            if self.query_favorites.is_empty() {
                ui.label("No favorites yet. Add your frequently used queries!");
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("favorites_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Query");
                            ui.label("Actions");
                            ui.end_row();

                            for (i, (name, query)) in self.query_favorites.iter().enumerate() {
                                ui.label(name);
                                ui.label(query);
                                ui.horizontal(|ui| {
                                    if ui.button("Load").clicked() {
                                        load_index = Some(i);
                                    }
                                    if ui.button("Remove").clicked() {
                                        remove_index = Some(i);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
            }
        });

        // Handle actions
        if add_new_favorite {
            self.add_query_favorite(new_favorite_name.clone(), self.query_editor.sql.clone());
            new_favorite_name.clear();
        }

        if let Some(index) = remove_index {
            self.remove_query_favorite(index);
        }

        if let Some(index) = load_index {
            if let Some((_, query)) = self.query_favorites.get(index) {
                self.query_editor.sql = query.clone();
            }
        }

        self.show_query_favorites = open;
    }

    fn show_filter_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_filter_dialog;
        let mut filter_query = self.table_filter.clone();
        let mut apply_filter = false;
        let mut clear_filter = false;
        let mut should_close = false;

        egui::Window::new("Data Filter")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([500.0, 300.0])
        .show(ctx, |ui| {
            ui.heading("Filter Table Data");
            ui.separator();

            ui.label("Enter SQL WHERE clause (without the WHERE keyword):");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Filter:");
                if ui.text_edit_singleline(&mut filter_query)
                    .lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    apply_filter = true;
                }
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Apply Filter").clicked() {
                    apply_filter = true;
                }
                if ui.button("Clear Filter").clicked() {
                    clear_filter = true;
                }
                if ui.button("Close").clicked() {
                    should_close = true;
                }
            });

            ui.add_space(10.0);
            ui.label("Examples:");
            ui.label("- id > 100");
            ui.label("- name LIKE '%test%'");
            ui.label("- created_at > '2023-01-01'");
            ui.label("- status = 'active' AND priority > 5");
        });

        if apply_filter {
            self.table_filter = filter_query;
            // Reload table data with filter
            if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                self.load_table_data(schema.clone(), table.clone());
            }
        }

        if clear_filter {
            self.table_filter.clear();
            // Reload table data without filter
            if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                self.load_table_data(schema.clone(), table.clone());
            }
        }

        if should_close {
            open = false;
        }

        self.show_filter_dialog = open;
    }

    fn show_connection_manager_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_connection_manager;
        let mut remove_index: Option<usize> = None;
        let mut switch_index: Option<usize> = None;

        egui::Window::new("Connection Manager")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            ui.heading("Connection Manager");
            ui.separator();

            // Current connection info
            if self.connected {
                ui.label(format!("Active: {}", self.connection_status));
            } else {
                ui.label("Not connected");
            }
            ui.separator();

            // Add new connection section
            ui.heading("Add New Connection");
            egui::Grid::new("new_connection_grid")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Host:");
                    ui.text_edit_singleline(&mut self.new_connection_dialog.host);
                    ui.end_row();

                    ui.label("Port:");
                    ui.text_edit_singleline(&mut self.new_connection_dialog.port);
                    ui.end_row();

                    ui.label("Database:");
                    ui.text_edit_singleline(&mut self.new_connection_dialog.database);
                    ui.end_row();

                    ui.label("User:");
                    ui.text_edit_singleline(&mut self.new_connection_dialog.user);
                    ui.end_row();

                    ui.label("Password:");
                    ui.add(TextEdit::singleline(&mut self.new_connection_dialog.password).password(true));
                    ui.end_row();
                });

            if ui.button("Add Connection").clicked() {
                if let Ok(port) = self.new_connection_dialog.port.parse::<u16>() {
                    let config = ConnectionConfig {
                        host: self.new_connection_dialog.host.clone(),
                        port,
                        database: self.new_connection_dialog.database.clone(),
                        user: self.new_connection_dialog.user.clone(),
                        password: self.new_connection_dialog.password.clone(),
                    };
                    self.add_connection(config);
                    // Reset only if successfully added
                    self.new_connection_dialog = ConnectionDialog::default();
                }
            }

            ui.separator();

            // List of connections
            ui.heading("Saved Connections");
            if self.connections.is_empty() {
                ui.label("No saved connections. Add one above!");
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("connections_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Name");
                            ui.label("Host:Port");
                            ui.label("Database");
                            ui.label("Actions");
                            ui.end_row();

                            for (i, conn) in self.connections.iter().enumerate() {
                                let name = format!("{}@{}:{}", conn.user, conn.host, conn.port);
                                ui.label(&name);
                                ui.label(format!("{}:{}", conn.host, conn.port));
                                ui.label(&conn.database);
                                ui.horizontal(|ui| {
                                    if self.active_connection_index == i {
                                        ui.label("🟢 Active");
                                    }
                                    if ui.button("Switch").clicked() {
                                        switch_index = Some(i);
                                    }
                                    if ui.button("Remove").clicked() {
                                        remove_index = Some(i);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
            }
        });

        // Handle actions
        if let Some(index) = remove_index {
            self.remove_connection(index);
        }

        if let Some(index) = switch_index {
            self.switch_connection(index);
        }

        self.show_connection_manager = open;
    }

    fn show_search_results_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_search_results;

        egui::Window::new("Search Database Objects")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([500.0, 400.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                if ui.text_edit_singleline(&mut self.search_query).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.search_query.is_empty() {
                        self.search_objects(self.search_query.clone());
                    }

                if ui.button("🔍 Search").clicked() && !self.search_query.is_empty() {
                        self.search_objects(self.search_query.clone());
                    }
            });

            ui.separator();

            if !self.search_results.is_empty() {
                ui.label(format!("Found {} results:", self.search_results.len()));
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for result in &self.search_results {
                        let icon = match result.object_type.as_str() {
                            "table" => "📋",
                            "view" => "🖼️",
                            "column" => "📝",
                            _ => "📊",
                        };

                        if result.object_type == "column" {
                            if let Some(col_name) = &result.column_name {
                                ui.horizontal(|ui| {
                                    ui.label(icon);
                                    ui.label(format!("{}.{}.{}", result.schema, result.name, col_name));
                                    ui.label("(");
                                    ui.monospace(&result.object_type);
                                    ui.label(")");
                                });
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.label(icon);
                                ui.label(format!("{}.{}", result.schema, result.name));
                                ui.label("(");
                                ui.monospace(&result.object_type);
                                ui.label(")");
                            });
                        }
                    }
                });
            } else if !self.search_query.is_empty() {
                ui.label("No results found.");
            } else {
                ui.label("Enter a search term to find database objects.");
            }
        });

        self.show_search_results = open;
    }

    fn process_responses(&mut self) {
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                DbResponse::Connected => {
                    self.connected = true;
                    self.status_message = "Connected successfully".to_string();
                    self.load_databases();
                }
                DbResponse::Disconnected => {
                    self.connected = false;
                    self.connection_status = "Not connected".to_string();
                }
                DbResponse::ConnectionError(err) => {
                    self.error_message = Some(format!("Connection failed: {}", err));
                    self.status_message = "Connection failed".to_string();

                    // Provide more helpful error messages for common issues
                    if err.contains("timeout") {
                        self.error_message = Some(format!("{} - Check if database server is running and accessible", err));
                    } else if err.contains("password authentication failed") {
                        self.error_message = Some(format!("{} - Verify username and password", err));
                    } else if err.contains("does not exist") {
                        self.error_message = Some(format!("{} - Check database name", err));
                    } else if err.contains("Connection refused") {
                        self.error_message = Some(format!("{} - Verify host and port, check firewall settings", err));
                    }
                }
                DbResponse::Databases(databases) => {
                    self.database_tree.databases = databases;
                    self.status_message = "Databases loaded".to_string();
                }
                DbResponse::Schemas(schemas) => {
                    self.database_tree.schemas = schemas;
                }
                DbResponse::Tables(tables) => {
                    self.database_tree.tables = tables;
                }
                DbResponse::ColumnTypes(types) => {
                    self.column_types = types;
                }
                DbResponse::TableInfo(info) => {
                    self.table_info = Some(info);
                    self.show_table_info = true;
                }
                DbResponse::SearchResults(results) => {
                    self.search_results = results;
                    self.show_search_results = true;
                }
                DbResponse::QueryResult(result) => {
                    self.is_query_running = false;
                    if result.affected_rows > 0 {
                        self.status_message =
                            format!("Query executed. {} rows affected", result.affected_rows);
                        self.results_table.columns.clear();
                        self.results_table.rows.clear();
                    } else {
                        self.status_message =
                            format!("Query executed. {} rows returned", result.rows.len());
                        self.results_table.columns = result.columns;
                        self.results_table.rows = result.rows;
                    }
                }
                DbResponse::TableData(result, count) => {
                    self.table_total_rows = count;
                    self.results_table.total_rows = count;
                    let rows_len = result.rows.len();
                    if self.results_table.loaded_rows == 0 {
                        self.results_table.columns = result.columns;
                        self.results_table.rows = result.rows;
                    } else {
                        self.results_table.rows.extend(result.rows);
                    }
                    self.results_table.loaded_rows += rows_len;
                    if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table)
                    {
                        self.status_message =
                            format!("Loaded {}.{} ({} / {} rows)", schema, table, self.results_table.loaded_rows, count);
                    }
                }
                DbResponse::Error(err) => {
                    self.is_query_running = false;
                    self.error_message = Some(err);
                    self.status_message = "Operation failed".to_string();
                }
                DbResponse::ConnectionStatus(is_connected, config) => {
                    if is_connected != self.connected {
                        self.connected = is_connected;
                        if is_connected {
                            self.current_config = Some(config.clone());
                            self.connection_status = format!(
                                "Connected to {}@{}:{}/{}",
                                config.user, config.host, config.port, config.database
                            );
                        } else {
                            self.connection_status = "Not connected".to_string();
                        }
                        self.need_repaint = true;
                    }
                }
                DbResponse::CellUpdated => {
                    self.status_message = "Cell updated successfully".to_string();
                    // Reload table data
                    if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                        self.load_table_data(schema.clone(), table.clone());
                    }
                }
                DbResponse::TransactionStarted => {
                    self.is_in_transaction = true;
                    self.status_message = "Transaction started".to_string();
                }
                DbResponse::TransactionCommitted => {
                    self.is_in_transaction = false;
                    self.status_message = "Transaction committed".to_string();
                }
                DbResponse::TransactionRolledBack => {
                    self.is_in_transaction = false;
                    self.status_message = "Transaction rolled back".to_string();
                }
            }
        }
    }
}

impl eframe::App for ShowelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending database responses
        self.process_responses();

        // Force repaint if needed for immediate UI updates
        if self.need_repaint {
            ctx.request_repaint();
            self.need_repaint = false;
        }

        // Periodically check connection status
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        // Check connection status every 2 seconds
        let now = std::time::Instant::now();
        if now.duration_since(self.last_connection_check).as_secs() >= 2 {
            self.check_connection();
            self.last_connection_check = now;
        }

        // Top menu bar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Connection", |ui| {
                    if ui.button("Connect...").clicked() {
                        self.show_connection_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Disconnect").clicked() {
                        self.disconnect();
                        ui.close_menu();
                    }
                    if ui.button("Manage Connections...").clicked() {
                        self.show_connection_manager = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Refresh").clicked() {
                        if self.connected {
                            self.load_databases();
                        }
                        ui.close_menu();
                    }
                    if ui.button("Table Info").clicked() {
                        if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                            self.load_table_info(schema.clone(), table.clone());
                        }
                        ui.close_menu();
                    }
                    if ui.button("Search Objects...").clicked() {
                        self.show_search_results = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Favorites", |ui| {
                    if ui.button("Add Current Query").clicked() {
                        if !self.query_editor.sql.trim().is_empty() {
                            self.show_query_favorites = true;
                        }
                        ui.close_menu();
                    }
                    if ui.button("Manage Favorites...").clicked() {
                        self.show_query_favorites = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Transaction", |ui| {
                    if self.is_in_transaction {
                        if ui.button("🟢 Commit").clicked() {
                            self.commit_transaction();
                            ui.close_menu();
                        }
                        if ui.button("🔴 Rollback").clicked() {
                            self.rollback_transaction();
                            ui.close_menu();
                        }
                    } else if ui.button("🟡 Begin").clicked() {
                            self.begin_transaction();
                            ui.close_menu();
                        }
                });

                ui.separator();

                ui.label(&self.connection_status);

                if self.connected {
                    ui.label("🟢");
                } else {
                    ui.label("🔴");
                }

                // Transaction status
                if self.is_in_transaction {
                    ui.label("🔒 TRANSACTION");
                }
            });
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                if let Some(ref error) = self.error_message {
                    ui.separator();
                    // Use ScrollArea for long error messages to prevent truncation
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.colored_label(egui::Color32::RED, format!("❌ {}", error));
                    });
                    if ui.button("Clear").clicked() {
                        self.error_message = None;
                    }
                }
            });
        });

        // Connection dialog
        if self.show_connection_dialog {
            if let Some(config) = self
                .connection_dialog
                .show(ctx, &mut self.show_connection_dialog)
            {
                self.connect(config);
            }
        }

        // Left sidebar - Database tree
        egui::SidePanel::left("left_panel")
            .default_width(250.0)
            .show(ctx, |ui| {
                if let Some(action) = self.database_tree.show(ui) {
                    match action {
                        TreeAction::LoadSchemas(_db) => {
                            self.load_schemas();
                        }
                        TreeAction::LoadTables(schema) => {
                            self.load_tables(schema);
                        }
                        TreeAction::SelectTable(schema, table) => {
                            self.table_page = 0;
                            self.load_table_data(schema, table);
                        }
                    }
                }
            });

        // Central panel - Query editor and results
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                // Check for search shortcut (Ctrl+F)
                if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
                    self.show_search_results = true;
                }
                // Update query editor completions
                let mut all_tables = Vec::new();
                let mut all_columns = Vec::new();

                // Add tables from database tree
                all_tables.extend(self.database_tree.tables.clone());

                // Add columns from current results
                all_columns.extend(self.results_table.columns.iter().cloned());

                // Add column names from column_types
                for (col_name, _) in &self.column_types {
                    if !all_columns.contains(col_name) {
                        all_columns.push(col_name.clone());
                    }
                }

                self.query_editor.update_completions(&all_tables, &all_columns);

                // Query editor - fixed at top
                let (execute, cancel) = self.query_editor.show(ui, self.is_query_running, &mut self.query_history, &mut self.show_query_history);
                if execute {
                    let query = self.query_editor.sql.clone();
                    self.execute_query(query);
                }
                if cancel {
                    self.cancel_query();
                }

                ui.separator();

                // Pagination controls (only show when viewing a table)
                if self.current_table.is_some() {
                    ui.horizontal(|ui| {
                        // Toggle between pagination and virtual scrolling
                        ui.checkbox(&mut self.use_pagination, "Use Pagination");
                        ui.separator();

                        if self.use_pagination {
                            let total_pages = if self.table_total_rows > 0 {
                                (self.table_total_rows as f64 / self.table_page_size as f64).ceil() as i64
                            } else {
                                1
                            };

                            ui.label(format!(
                                "Page {} of {}",
                                self.table_page + 1,
                                total_pages.max(1)
                            ));

                            if ui.button("◀ Previous").clicked() && self.table_page > 0 {
                                self.table_page -= 1;
                                if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                                    let page_size = self.table_page_size;
                                    let offset = self.table_page * page_size;
                                    let (sort_column, sort_ascending) = self.results_table.get_sort_info()
                                        .map(|(col, asc)| (Some(col), asc))
                                        .unwrap_or((None, true));
                                    let _ = self.command_tx.send(DbCommand::LoadTableData(
                                        schema.clone(), table.clone(), page_size, offset, sort_column, sort_ascending, None
                                    ));
                                }
                            }

                            if ui.button("Next ▶").clicked() && self.table_page < total_pages - 1 {
                                self.table_page += 1;
                                if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                                    let page_size = self.table_page_size;
                                    let offset = self.table_page * page_size;
                                    let (sort_column, sort_ascending) = self.results_table.get_sort_info()
                                        .map(|(col, asc)| (Some(col), asc))
                                        .unwrap_or((None, true));
                                    let _ = self.command_tx.send(DbCommand::LoadTableData(
                                        schema.clone(), table.clone(), page_size, offset, sort_column, sort_ascending, None
                                    ));
                                }
                            }

                            ui.separator();
                            ui.label(format!("Showing {} rows per page", self.table_page_size));

                            // Page size selector
                            ui.add_space(10.0);
                            ui.label("Rows per page:");
                            if ui.selectable_label(self.table_page_size == 50, "50").clicked() {
                                self.table_page_size = 50;
                                self.table_page = 0;
                                if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                                    let offset = self.table_page * self.table_page_size;
                                    let (sort_column, sort_ascending) = self.results_table.get_sort_info()
                                        .map(|(col, asc)| (Some(col), asc))
                                        .unwrap_or((None, true));
                                    let _ = self.command_tx.send(DbCommand::LoadTableData(
                                        schema.clone(), table.clone(), self.table_page_size, offset, sort_column, sort_ascending, None
                                    ));
                                }
                            }
                            if ui.selectable_label(self.table_page_size == 100, "100").clicked() {
                                self.table_page_size = 100;
                                self.table_page = 0;
                                if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                                    let offset = self.table_page * self.table_page_size;
                                    let (sort_column, sort_ascending) = self.results_table.get_sort_info()
                                        .map(|(col, asc)| (Some(col), asc))
                                        .unwrap_or((None, true));
                                    let _ = self.command_tx.send(DbCommand::LoadTableData(
                                        schema.clone(), table.clone(), self.table_page_size, offset, sort_column, sort_ascending, None
                                    ));
                                }
                            }
                            if ui.selectable_label(self.table_page_size == 200, "200").clicked() {
                                self.table_page_size = 200;
                                self.table_page = 0;
                                if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                                    let offset = self.table_page * self.table_page_size;
                                    let (sort_column, sort_ascending) = self.results_table.get_sort_info()
                                        .map(|(col, asc)| (Some(col), asc))
                                        .unwrap_or((None, true));
                                    let _ = self.command_tx.send(DbCommand::LoadTableData(
                                        schema.clone(), table.clone(), self.table_page_size, offset, sort_column, sort_ascending, None
                                    ));
                                }
                            }
                        }
                    });
                }

                // Results table header - fixed
                ui.horizontal(|ui| {
                    ui.heading("Results");

                    // Filter button
                    if ui.button("🔍 Filter").clicked() {
                        self.show_filter_dialog = true;
                    }

                    // Export menu
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("📤 Export", |ui| {
                            if ui.button("CSV").clicked() {
                                self.results_table.export_csv();
                            }
                            if ui.button("JSON").clicked() {
                                self.results_table.export_json();
                            }
                            if ui.button("JSON with Metadata").clicked() {
                                if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                                    self.results_table.export_json_with_metadata(table, schema);
                                }
                            }
                            if ui.button("SQL INSERTs").clicked() {
                                if let Some(table) = &self.current_table {
                                    self.results_table.export_sql_inserts(table);
                                }
                            }
                        });
                    });
                });

                ui.separator();

                // Results table - scrollable area with remaining space
                let prev_sort = self.results_table.get_sort_info();

                if let Some((value, column_name, row_idx, col_idx)) = self.results_table.show(ui) {
                    // Get column type
                    let column_type = self.column_types.iter()
                        .find(|(col, _)| col == &column_name)
                        .map(|(_, typ)| typ.clone())
                        .unwrap_or_else(|| "text".to_string());

                    self.edit_dialog.open(value, column_name, row_idx, col_idx, column_type);
                }

                if self.results_table.load_more {
                    self.load_more_table_data();
                    self.results_table.load_more = false;
                }

                // Check if sort changed
                let new_sort = self.results_table.get_sort_info();
                if prev_sort != new_sort {
                    // Reload table with new sort
                    if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                        self.load_table_data(schema.clone(), table.clone());
                    }
                }
            });
        });

        // Edit dialog
        if let Some((new_value, row_idx, col_idx)) = self.edit_dialog.show(ctx) {
            if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table) {
                let column = self.results_table.columns[col_idx].clone();
                let row_data = self.results_table.rows[row_idx].clone();
                let columns = self.results_table.columns.clone();

                self.update_cell(
                    schema.clone(),
                    table.clone(),
                    column,
                    new_value.clone(),
                    row_data,
                    columns,
                );

                // Update UI immediately for responsiveness
                self.results_table.update_cell(row_idx, col_idx, new_value);
            }
        }

        // Table info dialog
        if self.show_table_info {
            self.show_table_info_dialog(ctx);
        }

        // Search results dialog
        if self.show_search_results {
            self.show_search_results_dialog(ctx);
        }

        // Connection manager dialog
        if self.show_connection_manager {
            self.show_connection_manager_dialog(ctx);
        }

        // Filter dialog
        if self.show_filter_dialog {
            self.show_filter_dialog(ctx);
        }

        // Query favorites dialog
        if self.show_query_favorites {
            self.show_query_favorites_dialog(ctx);
        }
    }
}
