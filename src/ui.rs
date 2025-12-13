use egui::{Context, ScrollArea, TextEdit, Ui, Color32};

fn highlight_sql_line(line: &str, dark_theme: bool) -> Vec<(String, Color32)> {
    let mut result = Vec::new();

    // Work with char indices instead of byte indices to handle Unicode properly
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Check for SQL keywords
        let mut found_keyword = false;
        let keywords = [
            "SELECT", "FROM", "WHERE", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "OUTER",
            "ON", "GROUP", "BY", "HAVING", "ORDER", "LIMIT", "OFFSET", "DISTINCT",
            "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "DROP",
            "ALTER", "TABLE", "INDEX", "VIEW", "DATABASE", "SCHEMA", "COLUMN",
            "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "CONSTRAINT", "UNIQUE",
            "NOT", "NULL", "DEFAULT", "AUTO_INCREMENT", "AND", "OR", "IN", "EXISTS",
            "BETWEEN", "LIKE", "ILIKE", "AS", "COUNT", "SUM", "AVG", "MIN", "MAX",
            "CASE", "WHEN", "THEN", "ELSE", "END", "UNION", "ALL", "WITH", "RECURSIVE",
            "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION"
        ];

        for &keyword in &keywords {
            let keyword_chars: Vec<char> = keyword.chars().collect();
            if i + keyword_chars.len() <= chars.len() &&
               chars[i..i + keyword_chars.len()] == keyword_chars &&
               (i + keyword_chars.len() == chars.len() ||
                !chars.get(i + keyword_chars.len()).unwrap().is_alphanumeric() &&
                *chars.get(i + keyword_chars.len()).unwrap() != '_') {
                let color = if dark_theme {
                    Color32::from_rgb(86, 156, 214) // Blue for keywords (dark theme)
                } else {
                    Color32::from_rgb(0, 0, 255) // Dark blue for keywords (light theme)
                };
                result.push((keyword.to_string(), color));
                i += keyword_chars.len();
                found_keyword = true;
                break;
            }
        }

        if found_keyword {
            continue;
        }

        // Check for strings
        if chars[i] == '\'' {
            let mut end = i + 1;
            while end < chars.len() && chars[end] != '\'' {
                end += 1;
            }
            if end < chars.len() {
                end += 1; // include closing quote
            }
            let color = if dark_theme {
                Color32::from_rgb(206, 145, 120) // Orange for strings (dark theme)
            } else {
                Color32::from_rgb(128, 0, 0) // Dark red for strings (light theme)
            };
            result.push((chars[i..end].iter().collect::<String>(), color));
            i = end;
            continue;
        }

        if chars[i] == '"' {
            let mut end = i + 1;
            while end < chars.len() && chars[end] != '"' {
                end += 1;
            }
            if end < chars.len() {
                end += 1; // include closing quote
            }
            let color = if dark_theme {
                Color32::from_rgb(206, 145, 120) // Orange for strings (dark theme)
            } else {
                Color32::from_rgb(128, 0, 0) // Dark red for strings (light theme)
            };
            result.push((chars[i..end].iter().collect::<String>(), color));
            i = end;
            continue;
        }

        // Check for comments
        if chars[i] == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            let mut end = i + 2;
            while end < chars.len() && chars[end] != '\n' {
                end += 1;
            }
            let color = if dark_theme {
                Color32::from_rgb(106, 153, 85) // Green for comments (dark theme)
            } else {
                Color32::from_rgb(0, 128, 0) // Dark green for comments (light theme)
            };
            result.push((chars[i..end].iter().collect::<String>(), color));
            i = end;
            continue;
        }

        if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            let mut end = i + 2;
            while end + 1 < chars.len() && !(chars[end] == '*' && chars[end + 1] == '/') {
                end += 1;
            }
            if end + 1 < chars.len() {
                end += 2; // include */
            }
            let color = if dark_theme {
                Color32::from_rgb(106, 153, 85) // Green for comments (dark theme)
            } else {
                Color32::from_rgb(0, 128, 0) // Dark green for comments (light theme)
            };
            result.push((chars[i..end].iter().collect::<String>(), color));
            i = end;
            continue;
        }

        // Check for numbers
        if chars[i].is_ascii_digit() {
            let mut end = i;
            while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            let color = if dark_theme {
                Color32::from_rgb(181, 206, 168) // Light green for numbers (dark theme)
            } else {
                Color32::from_rgb(0, 100, 0) // Dark green for numbers (light theme)
            };
            result.push((chars[i..end].iter().collect::<String>(), color));
            i = end;
            continue;
        }

        // Check for operators and punctuation
        let ch = chars[i];
        if "!@#$%^&*()-+=[]{}|;:,.<>?/".contains(ch) {
            let color = if dark_theme {
                Color32::from_rgb(180, 180, 180) // Gray for operators (dark theme)
            } else {
                Color32::from_rgb(100, 100, 100) // Dark gray for operators (light theme)
            };
            result.push((ch.to_string(), color));
            i += 1;
            continue;
        }

        // Regular character
        let color = if dark_theme {
            Color32::from_rgb(220, 220, 220) // Light gray for regular text (dark theme)
        } else {
            Color32::from_rgb(50, 50, 50) // Dark gray for regular text (light theme)
        };
        result.push((ch.to_string(), color));
        i += 1;
    }

    result
}

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

#[derive(Default)]
pub struct EditDialog {
    pub open: bool,
    pub value: String,
    pub original_value: String,
    pub column_name: String,
    pub column_type: String,
    pub row_index: usize,
    pub col_index: usize,
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
                        if is_numeric && !self.value.is_empty() && self.value.to_uppercase() != "NULL" && self.value.parse::<f64>().is_err() {
                            ui.colored_label(egui::Color32::RED, "⚠ Invalid number");
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
    pub show_completions: bool,
    pub completion_items: Vec<String>,
    pub selected_completion: Option<usize>,
    pub dark_theme: bool,
}

impl Default for QueryEditor {
    fn default() -> Self {
        Self {
            sql: "SELECT * FROM information_schema.tables LIMIT 10;".to_string(),
            expanded: true,
            show_completions: false,
            completion_items: Vec::new(),
            selected_completion: None,
            dark_theme: true, // Default to dark theme
        }
    }
}

impl QueryEditor {
    pub fn format_sql(&mut self) {
        // Simple SQL formatting - capitalize keywords and add proper spacing
        let mut formatted = String::new();
        let mut in_string = false;
        let mut in_comment = false;
        let mut chars = self.sql.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '\'' | '"' if !in_comment => {
                    in_string = !in_string;
                    formatted.push(ch);
                }
                '-' if !in_string && !in_comment => {
                    chars.next(); // consume next char
                    if chars.peek() == Some(&'-') {
                        in_comment = true;
                        formatted.push_str("--");
                    } else {
                        formatted.push('-');
                    }
                }
                '/' if !in_string && !in_comment => {
                    chars.next(); // consume next char
                    if chars.peek() == Some(&'*') {
                        in_comment = true;
                        formatted.push_str("/*");
                    } else {
                        formatted.push('/');
                    }
                }
                '*' if in_comment => {
                    chars.next(); // consume next char
                    if chars.peek() == Some(&'/') {
                        in_comment = false;
                        formatted.push_str("*/");
                    } else {
                        formatted.push('*');
                    }
                }
                '\n' => {
                    if in_comment && chars.peek() != Some(&'-') && chars.peek() != Some(&'/') {
                        in_comment = false;
                    }
                    formatted.push('\n');
                }
                ch if ch.is_whitespace() => {
                    // Skip extra whitespace
                    if !formatted.ends_with(char::is_whitespace) {
                        formatted.push(' ');
                    }
                }
                ch => {
                    if !in_string && !in_comment {
                        // Capitalize keywords
                        let mut word = String::new();
                        word.push(ch);
                        while let Some(&next_ch) = chars.peek() {
                            if next_ch.is_alphanumeric() || next_ch == '_' {
                                word.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }

                        let upper_word = word.to_uppercase();
                        let keywords = [
                            "SELECT", "FROM", "WHERE", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "OUTER",
                            "ON", "GROUP", "BY", "HAVING", "ORDER", "LIMIT", "OFFSET", "DISTINCT",
                            "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "DROP",
                            "ALTER", "TABLE", "INDEX", "VIEW", "DATABASE", "SCHEMA", "COLUMN",
                            "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "CONSTRAINT", "UNIQUE",
                            "NOT", "NULL", "DEFAULT", "AND", "OR", "IN", "EXISTS", "AS"
                        ];

                        if keywords.contains(&upper_word.as_str()) {
                            formatted.push_str(&upper_word);
                        } else {
                            formatted.push_str(&word);
                        }
                    } else {
                        formatted.push(ch);
                    }
                }
            }
        }

        self.sql = formatted;
    }

    pub fn update_completions(&mut self, tables: &[String], columns: &[String]) {
        // Get the current word being typed
        let cursor_pos = self.sql.len(); // For simplicity, assume cursor is at end
        let text_before_cursor = &self.sql[..cursor_pos];

        // Find the current word
        let word_start = text_before_cursor.rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        let current_word = &text_before_cursor[word_start..];

        if current_word.is_empty() {
            self.show_completions = false;
            return;
        }

        // Generate completion items
        let mut items = Vec::new();

        // Add SQL keywords
        let keywords = [
            "SELECT", "FROM", "WHERE", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "OUTER",
            "ON", "GROUP", "BY", "HAVING", "ORDER", "LIMIT", "OFFSET", "DISTINCT",
            "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "DROP",
            "ALTER", "TABLE", "INDEX", "VIEW", "DATABASE", "SCHEMA", "COLUMN",
            "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "CONSTRAINT", "UNIQUE",
            "NOT", "NULL", "DEFAULT", "AND", "OR", "IN", "EXISTS", "BETWEEN",
            "LIKE", "AS", "COUNT", "SUM", "AVG", "MIN", "MAX", "CASE", "WHEN",
            "THEN", "ELSE", "END", "UNION", "ALL"
        ];

        for keyword in &keywords {
            if keyword.to_lowercase().starts_with(&current_word.to_lowercase()) {
                items.push(keyword.to_string());
            }
        }

        // Add table names
        for table in tables {
            if table.to_lowercase().starts_with(&current_word.to_lowercase()) {
                items.push(table.clone());
            }
        }

        // Add column names
        for column in columns {
            if column.to_lowercase().starts_with(&current_word.to_lowercase()) {
                items.push(column.clone());
            }
        }

        // Remove duplicates and sort
        items.sort();
        items.dedup();

        self.completion_items = items;
        self.show_completions = !self.completion_items.is_empty();
        self.selected_completion = None;
    }

    pub fn show(&mut self, ui: &mut Ui, is_query_running: bool, _query_history: &mut Vec<String>, show_history: &mut bool) -> (bool, bool) {
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
            } else if ui.button("▶ Execute").clicked() {
                execute = true;
            }
            if ui.button("Clear").clicked() {
                self.sql.clear();
            }
            ui.separator();
            if ui.button("📚 History").clicked() {
                *show_history = !*show_history;
            }
            ui.separator();
            let theme_icon = if self.dark_theme { "🌙" } else { "☀" };
            if ui.button(theme_icon).on_hover_text("Toggle syntax theme").clicked() {
                self.dark_theme = !self.dark_theme;
            }
            ui.separator();
            ui.label(format!("Lines: {}", self.sql.lines().count()));
            ui.separator();
            if ui.button("🔧 Format").on_hover_text("Format SQL query").clicked() {
                self.format_sql();
            }
        });
        ui.separator();

        if self.expanded {
            ui.separator();

            // Inline syntax highlighting in TextEdit
            let text_edit_response = ui.add(
                TextEdit::multiline(&mut self.sql)
                    .font(egui::TextStyle::Monospace)
                    .text_color(Color32::TRANSPARENT) // Make text invisible
                    .desired_width(f32::INFINITY)
                    .desired_rows(5)
            );

            // Draw highlighted text over the TextEdit
            let rect = text_edit_response.rect;
            let painter = ui.painter();
            let font_id = ui.style().text_styles.get(&egui::TextStyle::Monospace).cloned().unwrap_or(egui::FontId::monospace(12.0));

            // Calculate the actual text area within TextEdit (accounting for padding)
            let line_height = ui.fonts(|fonts| fonts.row_height(&font_id));

            // Calculate the actual text area within TextEdit (accounting for padding)
            let text_padding = ui.spacing().button_padding; // TextEdit uses button padding
            let text_start_x = rect.min.x + text_padding.x;
            let text_start_y = rect.min.y + text_padding.y;

            for (line_idx, line) in self.sql.lines().enumerate() {
                let highlighted = highlight_sql_line(line, self.dark_theme);
                let mut job = egui::text::LayoutJob::default();
                for (text, color) in highlighted {
                    if !text.is_empty() {
                        job.append(&text, 0.0, egui::TextFormat {
                            font_id: font_id.clone(),
                            color,
                            ..Default::default()
                        });
                    }
                }
                let galley = painter.layout_job(job);
                painter.galley(egui::pos2(text_start_x, text_start_y + line_idx as f32 * line_height), galley, Color32::TRANSPARENT);
            }
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
    // Virtual scrolling support
    pub total_rows: i64, // Total rows in the table (-1 if unknown)
    pub loaded_rows: usize, // How many rows are currently loaded
    pub page_size: usize, // How many rows to load at once
    pub load_more: bool, // Flag to trigger loading more rows
}

impl Default for ResultsTable {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            selected_cell: None,
            sort_column: None,
            sort_ascending: true,
            total_rows: 0,
            loaded_rows: 0,
            page_size: 100,
            load_more: false,
        }
    }
}

impl ResultsTable {
    pub fn export_csv(&self) {
        if self.columns.is_empty() {
            return;
        }

        let mut csv_content = String::new();

        // Add header row
        csv_content.push_str(&self.columns.join(","));
        csv_content.push('\n');

        // Add data rows
        for row in &self.rows {
            let csv_row: Vec<String> = row.iter().map(|cell| {
                // Escape quotes and wrap in quotes if necessary
                if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                    format!("\"{}\"", cell.replace('"', "\"\""))
                } else {
                    cell.clone()
                }
            }).collect();
            csv_content.push_str(&csv_row.join(","));
            csv_content.push('\n');
        }

        // For now, just print to console. In a real implementation, you'd save to a file
        // or show a file save dialog
        println!("CSV Export:\n{}", csv_content);
    }

    pub fn export_json(&self) {
        if self.columns.is_empty() {
            return;
        }

        use serde_json::json;

        let mut json_array = Vec::new();

        for row in &self.rows {
            let mut json_row = serde_json::Map::new();
            for (i, cell) in row.iter().enumerate() {
                if i < self.columns.len() {
                    json_row.insert(self.columns[i].clone(), json!(cell));
                }
            }
            json_array.push(json_row);
        }

        let json_content = serde_json::to_string_pretty(&json_array).unwrap_or_else(|_| "[]".to_string());

        // For now, just print to console. In a real implementation, you'd save to a file
        println!("JSON Export:\n{}", json_content);
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<(String, String, usize, usize)> {
        let mut clicked_cell = None;
        let mut sort_by_column: Option<usize> = None;
        if self.columns.is_empty() {
            ui.label("No results to display. Execute a query to see results.");
            return None;
        }



        ui.horizontal(|ui| {
            let total_display = if self.total_rows > 0 {
                self.total_rows.to_string()
            } else {
                "?".to_string()
            };
            ui.label(format!("Results: {} / {} rows", self.loaded_rows, total_display));
            ui.separator();
            ui.label("Double-click a cell to edit");
            ui.separator();
            ui.label("Click column header to sort");
            ui.separator();
            ui.label("Export:");
            if ui.button("CSV").clicked() {
                self.export_csv();
            }
            if ui.button("JSON").clicked() {
                self.export_json();
            }
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
                        for (row_idx, row) in self.rows.iter().enumerate() {
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

                        // Add loading rows if more data available
                        let remaining = if self.total_rows > 0 {
                            self.total_rows as usize - self.rows.len()
                        } else {
                            0
                        };
                        if remaining > 0 {
                            let loading_count = remaining.min(10); // Show up to 10 loading rows
                            for _ in 0..loading_count {
                                body.row(18.0, |mut row_ui| {
                                    for _ in 0..self.columns.len() {
                                        row_ui.col(|ui| {
                                            ui.label("Loading...");
                                        });
                                    }
                                });
                            }
                            if !self.load_more {
                                self.load_more = true;
                            }
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

#[derive(Default)]
pub struct DatabaseTree {
    pub databases: Vec<String>,
    pub schemas: Vec<String>,
    pub tables: Vec<String>,

    pub selected_schema: Option<String>,
    pub selected_table: Option<String>,
    pub expanded_databases: std::collections::HashSet<String>,
    pub expanded_schemas: std::collections::HashSet<String>,
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
