use eframe::egui;
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
    ExecuteQuery(String),
    CancelQuery,
    ResetCancel,
    LoadTableData(String, String, i64, i64, Option<String>, bool), // schema, table, limit, offset, sort_column, sort_ascending
    CheckConnection,
    UpdateCell(String, String, String, String, Vec<String>, Vec<String>), // schema, table, column, value, row_data, columns
}

enum DbResponse {
    Connected,
    Disconnected,
    ConnectionError(String),
    Databases(Vec<String>),
    Schemas(Vec<String>),
    Tables(Vec<String>),
    ColumnTypes(Vec<(String, String)>), // (column_name, data_type)
    QueryResult(QueryResult),
    TableData(QueryResult, i64),
    Error(String),
    ConnectionStatus(bool, ConnectionConfig),
    CellUpdated,
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
    table_total_rows: i64,

    // Timer for periodic checks
    last_connection_check: std::time::Instant,

    // UI update flags
    need_repaint: bool,

    // Query execution
    is_query_running: bool,

    // Query history
    query_history: Vec<String>,
    show_query_history: bool,
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
                                        DbCommand::LoadTableData(schema, table, limit, offset, sort_column, sort_ascending) => {
                                            let sort_col_ref = sort_column.as_deref();
                                            match db.get_table_data(&schema, &table, limit, offset, sort_col_ref, sort_ascending).await {
                                                Ok(result) => {
                                                    // Also get row count
                                                    match db.get_table_row_count(&schema, &table).await {
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
                                        DbCommand::ResetCancel => {
                                            db.reset_cancel().await;
                                            DbResponse::Error("".to_string()) // Dummy response, not used
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
            table_total_rows: 0,
            last_connection_check: std::time::Instant::now(),
            need_repaint: false,
            is_query_running: false,
            query_history: Vec::new(),
            show_query_history: false,
        }
    }

    fn connect(&mut self, config: ConnectionConfig) {
        self.status_message = "Connecting...".to_string();
        self.error_message = None;
        self.need_repaint = true;
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
           self.query_history.last().is_none_or(|last| last != trimmed_query) {
            self.query_history.push(trimmed_query.to_string());
            // Keep only the last 50 queries
            if self.query_history.len() > 50 {
                self.query_history.remove(0);
            }
        }



        self.status_message = "Executing query...".to_string();
        self.error_message = None;
        self.is_query_running = true;
        let _ = self.command_tx.send(DbCommand::ExecuteQuery(query));
        // Reset cancellation flag for new query
        let _ = self.command_tx.send(DbCommand::ResetCancel);
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

            let _ = self
                .command_tx
                .send(DbCommand::LoadTableData(schema.clone(), table.clone(), page_size, offset, sort_column, sort_ascending));
        }
    }

    fn check_connection(&mut self) {
        let _ = self.command_tx.send(DbCommand::CheckConnection);
    }

    fn update_cell(&mut self, schema: String, table: String, column: String, value: String, row_data: Vec<String>, columns: Vec<String>) {
        let _ = self.command_tx.send(DbCommand::UpdateCell(schema, table, column, value, row_data, columns));
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
                });

                ui.separator();

                ui.label(&self.connection_status);

                if self.connected {
                    ui.label("🟢");
                } else {
                    ui.label("🔴");
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
                if cancel {
                    self.cancel_query();
                }

                ui.separator();





                // Results table header - fixed
                ui.horizontal(|ui| {
                    ui.heading("Results");
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
    }
}
