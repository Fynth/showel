use egui::{Context, ScrollArea, TextEdit, Ui};

pub struct ConnectionDialog {
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
}

impl Default for ConnectionDialog {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: "5432".to_string(),
            database: "postgres".to_string(),
            user: "postgres".to_string(),
            password: String::new(),
        }
    }
}

impl ConnectionDialog {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) -> Option<crate::db::ConnectionConfig> {
        let mut connect = false;
        let mut cancel = false;
        let mut result = None;

        let mut is_open = *open;
        egui::Window::new("Connect to Database")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("connection_grid")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Host:");
                        ui.text_edit_singleline(&mut self.host);
                        ui.end_row();

                        ui.label("Port:");
                        ui.text_edit_singleline(&mut self.port);
                        ui.end_row();

                        ui.label("Database:");
                        ui.text_edit_singleline(&mut self.database);
                        ui.end_row();

                        ui.label("User:");
                        ui.text_edit_singleline(&mut self.user);
                        ui.end_row();

                        ui.label("Password:");
                        ui.add(TextEdit::singleline(&mut self.password).password(true));
                        ui.end_row();
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Connect").clicked() {
                        connect = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        *open = is_open;

        if connect {
            if let Ok(port) = self.port.parse::<u16>() {
                result = Some(crate::db::ConnectionConfig {
                    host: self.host.clone(),
                    port,
                    database: self.database.clone(),
                    user: self.user.clone(),
                    password: self.password.clone(),
                });
                *open = false;
            }
        }

        if cancel {
            *open = false;
        }

        result
    }
}

pub struct QueryEditor {
    pub sql: String,
    pub expanded: bool,
}

impl Default for QueryEditor {
    fn default() -> Self {
        Self {
            sql: "SELECT * FROM information_schema.tables LIMIT 10;".to_string(),
            expanded: true,
        }
    }
}

impl QueryEditor {
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut execute = false;

        ui.horizontal(|ui| {
            let icon = if self.expanded { "▼" } else { "▶" };
            if ui.button(icon).clicked() {
                self.expanded = !self.expanded;
            }
            ui.heading("SQL Query");

            if ui.button("▶ Execute").clicked() {
                execute = true;
            }
            if ui.button("Clear").clicked() {
                self.sql.clear();
            }
        });
        ui.separator();

        if self.expanded {
            ui.separator();

            let editor_height = if self.sql.lines().count() > 5 {
                150.0
            } else {
                100.0
            };

            ScrollArea::vertical()
                .max_height(editor_height)
                .id_source("sql_editor_scroll")
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.sql)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(5),
                    );
                });
        }

        execute
    }
}

pub struct ResultsTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Default for ResultsTable {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }
}

impl ResultsTable {
    pub fn show(&mut self, ui: &mut Ui) {
        if self.columns.is_empty() {
            ui.label("No results to display. Execute a query to see results.");
            return;
        }

        ui.label(format!("Results: {} rows", self.rows.len()));
        ui.separator();

        use egui_extras::{Column, TableBuilder};

        // Use ScrollArea for both horizontal and vertical scrolling
        ScrollArea::both()
            .auto_shrink([false, false])
            .id_source("results_table_scroll")
            .show(ui, |ui| {
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .min_scrolled_height(0.0)
                    .columns(
                        Column::auto().resizable(true).clip(true),
                        self.columns.len(),
                    )
                    .header(20.0, |mut header| {
                        for column in &self.columns {
                            header.col(|ui| {
                                ui.strong(column);
                            });
                        }
                    })
                    .body(|mut body| {
                        for row in &self.rows {
                            body.row(18.0, |mut row_ui| {
                                for cell in row {
                                    row_ui.col(|ui| {
                                        ui.label(cell);
                                    });
                                }
                            });
                        }
                    });
            });
    }
}

pub struct DatabaseTree {
    pub databases: Vec<String>,
    pub schemas: Vec<String>,
    pub tables: Vec<String>,
    pub selected_database: Option<String>,
    pub selected_schema: Option<String>,
    pub selected_table: Option<String>,
    pub expanded_databases: std::collections::HashSet<String>,
    pub expanded_schemas: std::collections::HashSet<String>,
}

impl Default for DatabaseTree {
    fn default() -> Self {
        Self {
            databases: Vec::new(),
            schemas: Vec::new(),
            tables: Vec::new(),
            selected_database: None,
            selected_schema: None,
            selected_table: None,
            expanded_databases: std::collections::HashSet::new(),
            expanded_schemas: std::collections::HashSet::new(),
        }
    }
}

impl DatabaseTree {
    pub fn show(&mut self, ui: &mut Ui) -> Option<TreeAction> {
        let mut action = None;

        ui.heading("Database Explorer");
        ui.separator();

        ScrollArea::vertical().show(ui, |ui| {
            if self.databases.is_empty() {
                ui.label("Connect to see databases");
                return;
            }

            for database in &self.databases.clone() {
                let is_expanded = self.expanded_databases.contains(database);
                let _header_response = ui.horizontal(|ui| {
                    let icon = if is_expanded { "▼" } else { "▶" };
                    if ui.button(icon).clicked() {
                        if is_expanded {
                            self.expanded_databases.remove(database);
                        } else {
                            self.expanded_databases.insert(database.clone());
                            action = Some(TreeAction::LoadSchemas(database.clone()));
                        }
                    }
                    ui.label(format!("📊 {}", database));
                });

                if is_expanded {
                    ui.indent(database, |ui| {
                        for schema in &self.schemas.clone() {
                            let is_schema_expanded = self.expanded_schemas.contains(schema);
                            ui.horizontal(|ui| {
                                let icon = if is_schema_expanded { "▼" } else { "▶" };
                                if ui.button(icon).clicked() {
                                    if is_schema_expanded {
                                        self.expanded_schemas.remove(schema);
                                    } else {
                                        self.expanded_schemas.insert(schema.clone());
                                        action = Some(TreeAction::LoadTables(schema.clone()));
                                    }
                                }
                                ui.label(format!("📁 {}", schema));
                            });

                            if is_schema_expanded {
                                ui.indent(schema, |ui| {
                                    for table in &self.tables.clone() {
                                        if ui
                                            .selectable_label(
                                                self.selected_table.as_ref() == Some(table),
                                                format!("📋 {}", table),
                                            )
                                            .clicked()
                                        {
                                            self.selected_table = Some(table.clone());
                                            self.selected_schema = Some(schema.clone());
                                            action = Some(TreeAction::SelectTable(
                                                schema.clone(),
                                                table.clone(),
                                            ));
                                        }
                                    }
                                });
                            }
                        }
                    });
                }
            }
        });

        action
    }
}

#[derive(Debug, Clone)]
pub enum TreeAction {
    LoadSchemas(String),
    LoadTables(String),
    SelectTable(String, String),
}
