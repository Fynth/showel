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

pub struct EditDialog {
    pub open: bool,
    pub value: String,
    pub original_value: String,
    pub column_name: String,
    pub column_type: String,
    pub row_index: usize,
    pub col_index: usize,
}

impl Default for EditDialog {
    fn default() -> Self {
        Self {
            open: false,
            value: String::new(),
            original_value: String::new(),
            column_name: String::new(),
            column_type: String::new(),
            row_index: 0,
            col_index: 0,
        }
    }
}

impl EditDialog {
    pub fn open(&mut self, value: String, column_name: String, row_index: usize, col_index: usize, column_type: String) {
        self.value = value.clone();
        self.original_value = value;
        self.column_name = column_name;
        self.column_type = column_type;
        self.row_index = row_index;
        self.col_index = col_index;
        self.open = true;
    }

    pub fn show(&mut self, ctx: &Context) -> Option<(String, usize, usize)> {
        let mut result = None;
        let mut save = false;
        let mut cancel = false;

        if !self.open {
            return None;
        }

        egui::Window::new(format!("Edit: {}", self.column_name))
            .open(&mut self.open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(format!("Column: {}", self.column_name));
                    ui.label(format!("Type: {}", self.column_type));

                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("Original:");
                        ui.monospace(&self.original_value);
                    });

                    ui.separator();

                    // Different UI based on data type
                    let type_lower = self.column_type.to_lowercase();
                    let is_bool = type_lower == "boolean" || type_lower == "bool";
                    let is_numeric = matches!(
                        type_lower.as_str(),
                        "int2" | "int4" | "int8" | "integer" | "smallint" | "bigint"
                        | "numeric" | "decimal" | "real" | "double precision"
                        | "float4" | "float8"
                    );

                    if is_bool {
                        ui.label("New value:");
                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.value == "true", "✓ true").clicked() {
                                self.value = "true".to_string();
                            }
                            if ui.selectable_label(self.value == "false", "✗ false").clicked() {
                                self.value = "false".to_string();
                            }
                            if ui.selectable_label(self.value.to_lowercase() == "null", "NULL").clicked() {
                                self.value = "NULL".to_string();
                            }
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label("New value:");
                            if ui.small_button("Set NULL").clicked() {
                                self.value = "NULL".to_string();
                            }
                        });

                        let hint = if is_numeric {
                            "Enter number..."
                        } else {
                            "Enter new value..."
                        };

                        let text_edit = ui.add(
                            TextEdit::singleline(&mut self.value)
                                .desired_width(300.0)
                                .hint_text(hint)
                        );

                        if text_edit.changed() {
                            // Visual feedback that value changed
                        }

                        // Validation hint for numeric types
                        if is_numeric && !self.value.is_empty() && self.value.to_uppercase() != "NULL" {
                            if self.value.parse::<f64>().is_err() {
                                ui.colored_label(egui::Color32::RED, "⚠ Invalid number");
                            }
                        }
                    }

                    ui.add_space(5.0);

                    if self.value != self.original_value {
                        ui.colored_label(egui::Color32::YELLOW, "⚠ Value will be updated");
                    }

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            save = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            });

        if save {
            result = Some((self.value.clone(), self.row_index, self.col_index));
            self.open = false;
        }

        if cancel {
            self.open = false;
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
    pub fn show(&mut self, ui: &mut Ui, is_query_running: bool) -> (bool, bool) {
        let mut execute = false;
        let mut cancel = false;

        ui.horizontal(|ui| {
            let icon = if self.expanded { "▼" } else { "▶" };
            if ui.button(icon).clicked() {
                self.expanded = !self.expanded;
            }
            ui.heading("SQL Query");

            if is_query_running {
                if ui.button("⏹ Cancel").clicked() {
                    cancel = true;
                }
            } else {
                if ui.button("▶ Execute").clicked() {
                    execute = true;
                }
            }
            if ui.button("Clear").clicked() {
                self.sql.clear();
            }
        });
        ui.separator();

        if self.expanded {
            ui.separator();

            let lines: Vec<&str> = self.sql.lines().collect();
            let num_lines = lines.len().max(1);
            let editor_height = if num_lines > 5 { 150.0 } else { 100.0 };

            ScrollArea::vertical()
                .max_height(editor_height)
                .id_source("sql_editor_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Line numbers column
                        ui.vertical(|ui| {
                            for i in 1..=num_lines {
                                ui.monospace(format!("{:3} ", i));
                            }
                        });
                        ui.separator();

                        // Text editor
                        ui.add(
                            TextEdit::multiline(&mut self.sql)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(5),
                        );
                    });
                });
        }

        (execute, cancel)
    }
}

pub struct ResultsTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub selected_cell: Option<(usize, usize)>, // (row, col)
    pub sort_column: Option<usize>,
    pub sort_ascending: bool,
    pub max_display_rows: usize,
}

impl Default for ResultsTable {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            selected_cell: None,
            sort_column: None,
            sort_ascending: true,
            max_display_rows: 1000,
        }
    }
}

impl ResultsTable {
    pub fn show(&mut self, ui: &mut Ui) -> Option<(String, String, usize, usize)> {
        let mut clicked_cell = None;
        let mut sort_by_column: Option<usize> = None;
        if self.columns.is_empty() {
            ui.label("No results to display. Execute a query to see results.");
            return None;
        }

        let display_rows = self.rows.len().min(self.max_display_rows);
        let truncated = self.rows.len() > self.max_display_rows;

        ui.horizontal(|ui| {
            if truncated {
                ui.label(format!("Results: {} rows (showing first {})", self.rows.len(), display_rows));
            } else {
                ui.label(format!("Results: {} rows", self.rows.len()));
            }
            ui.separator();
            ui.label("Double-click a cell to edit");
            ui.separator();
            ui.label("Click column header to sort");
        });
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
                        for (col_idx, column) in self.columns.iter().enumerate() {
                            header.col(|ui| {
                                let (text, is_sorted) = if self.sort_column == Some(col_idx) {
                                    let arrow = if self.sort_ascending { " ▲" } else { " ▼" };
                                    (format!("{}{}", column, arrow), true)
                                } else {
                                    (column.clone(), false)
                                };

                                let button = if is_sorted {
                                    ui.button(egui::RichText::new(text).strong())
                                } else {
                                    ui.button(text)
                                };

                                if button.clicked() {
                                    sort_by_column = Some(col_idx);
                                }
                            });
                        }
                    })
                    .body(|mut body| {
                        for (row_idx, row) in self.rows.iter().enumerate().take(display_rows) {
                            body.row(18.0, |mut row_ui| {
                                for (col_idx, cell) in row.iter().enumerate() {
                                    row_ui.col(|ui| {
                                        let is_selected = self.selected_cell == Some((row_idx, col_idx));

                                        let response = if is_selected {
                                            ui.add(egui::SelectableLabel::new(true, cell))
                                        } else {
                                            ui.add(egui::Label::new(cell).sense(egui::Sense::click()))
                                        };

                                        if response.double_clicked() {
                                            self.selected_cell = Some((row_idx, col_idx));
                                            clicked_cell = Some((
                                                cell.clone(),
                                                self.columns[col_idx].clone(),
                                                row_idx,
                                                col_idx,
                                            ));
                                        } else if response.clicked() {
                                            self.selected_cell = Some((row_idx, col_idx));
                                        }
                                    });
                                }
                            });
                        }
                    });
            });

        // Handle column sort
        if let Some(col_idx) = sort_by_column {
            if self.sort_column == Some(col_idx) {
                self.sort_ascending = !self.sort_ascending;
            } else {
                self.sort_column = Some(col_idx);
                self.sort_ascending = true;
            }
        }

        clicked_cell
    }

    pub fn update_cell(&mut self, row_idx: usize, col_idx: usize, new_value: String) {
        if row_idx < self.rows.len() && col_idx < self.rows[row_idx].len() {
            self.rows[row_idx][col_idx] = new_value;
        }
    }

    pub fn get_sort_info(&self) -> Option<(String, bool)> {
        self.sort_column.map(|idx| {
            (self.columns[idx].clone(), self.sort_ascending)
        })
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
