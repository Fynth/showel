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
    ExecuteQuery(String),
    LoadTableData(String, String, i64, i64),
    GetTableRowCount(String, String),
    CheckConnection,
}

enum DbResponse {
    Connected,
    Disconnected,
    ConnectionError(String),
    Databases(Vec<String>),
    Schemas(Vec<String>),
    Tables(Vec<String>),
    QueryResult(QueryResult),
    TableData(QueryResult, i64),
    Error(String),
    ConnectionStatus(bool, ConnectionConfig),
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

    // Timer for periodic checks
    last_connection_check: std::time::Instant,
}

impl ShowelApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (command_tx, command_rx) = channel::<DbCommand>();
        let (response_tx, response_rx) = channel::<DbResponse>();

        // Spawn database worker thread
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let db = DatabaseConnection::new();

            loop {
                match command_rx.recv() {
                    Ok(command) => {
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
                                DbCommand::ExecuteQuery(query) => {
                                    match db.execute_query(&query).await {
                                        Ok(result) => DbResponse::QueryResult(result),
                                        Err(e) => DbResponse::Error(e.to_string()),
                                    }
                                }
                                DbCommand::LoadTableData(schema, table, limit, offset) => {
                                    match db.get_table_data(&schema, &table, limit, offset).await {
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
                                DbCommand::GetTableRowCount(schema, table) => {
                                    match db.get_table_row_count(&schema, &table).await {
                                        Ok(count) => DbResponse::TableData(
                                            QueryResult {
                                                columns: vec![],
                                                rows: vec![],
                                                affected_rows: 0,
                                            },
                                            count,
                                        ),
                                        Err(e) => DbResponse::Error(e.to_string()),
                                    }
                                }
                                DbCommand::CheckConnection => {
                                    let is_connected = db.is_connected().await;
                                    let config = db.get_config().await;
                                    DbResponse::ConnectionStatus(is_connected, config)
                                }
                            }
                        });

                        if response_tx.send(response).is_err() {
                            break;
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

            command_tx,
            response_rx,

            status_message: "Ready".to_string(),
            error_message: None,

            current_schema: None,
            current_table: None,
            table_page: 0,
            table_page_size: 100,
            table_total_rows: 0,
            last_connection_check: std::time::Instant::now(),
        }
    }

    fn connect(&mut self, config: ConnectionConfig) {
        self.status_message = "Connecting...".to_string();
        self.error_message = None;
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

    fn execute_query(&mut self, query: String) {
        self.status_message = "Executing query...".to_string();
        self.error_message = None;
        let _ = self.command_tx.send(DbCommand::ExecuteQuery(query));
    }

    fn load_table_data(&mut self, schema: String, table: String) {
        let page_size = self.table_page_size;
        let offset = self.table_page * page_size;

        self.current_schema = Some(schema.clone());
        self.current_table = Some(table.clone());

        let _ = self
            .command_tx
            .send(DbCommand::LoadTableData(schema, table, page_size, offset));
    }

    fn check_connection(&mut self) {
        let _ = self.command_tx.send(DbCommand::CheckConnection);
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
                DbResponse::QueryResult(result) => {
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
                    self.results_table.columns = result.columns;
                    self.results_table.rows = result.rows;
                    if let (Some(schema), Some(table)) = (&self.current_schema, &self.current_table)
                    {
                        self.status_message =
                            format!("Loaded {}.{} ({} total rows)", schema, table, count);
                    }
                }
                DbResponse::Error(err) => {
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
                    ui.colored_label(egui::Color32::RED, format!("❌ {}", error));
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
                // Query editor - fixed at top
                if self.query_editor.show(ui) {
                    let query = self.query_editor.sql.clone();
                    self.execute_query(query);
                }

                ui.separator();

                // Table pagination if viewing a table - fixed
                if self.current_table.is_some() {
                    ui.horizontal(|ui| {
                        let total_pages = (self.table_total_rows as f64
                            / self.table_page_size as f64)
                            .ceil() as i64;

                        if ui.button("◀ Previous").clicked() && self.table_page > 0 {
                            self.table_page -= 1;
                            if let (Some(schema), Some(table)) =
                                (&self.current_schema, &self.current_table)
                            {
                                self.load_table_data(schema.clone(), table.clone());
                            }
                        }

                        ui.label(format!(
                            "Page {} of {}",
                            self.table_page + 1,
                            total_pages.max(1)
                        ));

                        if ui.button("Next ▶").clicked() && self.table_page < total_pages - 1 {
                            self.table_page += 1;
                            if let (Some(schema), Some(table)) =
                                (&self.current_schema, &self.current_table)
                            {
                                self.load_table_data(schema.clone(), table.clone());
                            }
                        }

                        ui.separator();
                        ui.label(format!("Showing {} rows per page", self.table_page_size));
                    });
                    ui.separator();
                }

                // Results table header - fixed
                ui.horizontal(|ui| {
                    ui.heading("Results");
                    if !self.results_table.columns.is_empty() {
                        ui.label(format!("({} rows)", self.results_table.rows.len()));
                    }
                });

                ui.separator();

                // Results table - scrollable area with remaining space
                self.results_table.show(ui);
            });
        });
    }
}
