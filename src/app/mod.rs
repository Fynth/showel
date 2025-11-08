// src/app/mod.rs

use crate::app::state::{ActiveTab, App, ConnectionConfig};
use egui::{CentralPanel, ComboBox, ScrollArea, SidePanel, TextEdit, Ui, Vec2};

// Make state module public so main.rs can import from it
pub mod state;

// Main application implementation
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        self.apply_theme(ctx);

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // Main layout
        SidePanel::left("left_panel")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_connection_panel(ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            self.render_main_content(ui);
        });
    }
}

impl App {
    /// Apply theme based on current setting
    fn apply_theme(&mut self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        match self.theme.as_str() {
            "Dark" => {
                style.visuals = egui::Visuals::dark();
            }
            "Light" => {
                style.visuals = egui::Visuals::light();
            }
            _ => {}
        }

        ctx.set_style(style);
    }

    /// Render main menu bar
    fn render_menu_bar(&mut self, ui: &mut Ui) {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu_button(ui, "File", |ui| {
                if ui.button("New Query").clicked() {
                    self.current_query.clear();
                    self.clear_messages();
                }
                if ui.button("Clear Results").clicked() {
                    self.query_results.clear();
                    self.clear_messages();
                }
            });

            egui::menu::menu_button(ui, "Database", |ui| {
                if ui.button("Add SQLite Connection").clicked() {
                    let config = ConnectionConfig {
                        name: "SQLite Demo Database".to_string(),
                        database_path: "demo.db".to_string(),
                        created_at: chrono::Utc::now(),
                    };
                    self.add_connection(config);
                    self.show_info("SQLite connection added".to_string());
                }
            });

            egui::menu::menu_button(ui, "Query", |ui| {
                if ui.button("Execute").clicked() {
                    self.execute_query();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                ui.label(format!("Theme:"));
                ComboBox::from_id_source("theme_selector")
                    .selected_text(&self.theme)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.theme, "Light".to_string(), "Light");
                        ui.selectable_value(&mut self.theme, "Dark".to_string(), "Dark");
                    });
            });
        });
    }

    /// Render connection panel
    fn render_connection_panel(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("🗄️ Database Connections");
            ui.add_space(5.0);

            if self.connections.is_empty() {
                ui.label("No connections configured");
                ui.add_space(5.0);
                if ui.button("➕ Add SQLite Connection").clicked() {
                    let config = ConnectionConfig {
                        name: "SQLite Demo Database".to_string(),
                        database_path: "demo.db".to_string(),
                        created_at: chrono::Utc::now(),
                    };
                    self.add_connection(config);
                }
            } else {
                // Collect connection data first to avoid borrow checker issues
                let connection_data: Vec<_> = self
                    .connections
                    .iter()
                    .enumerate()
                    .map(|(i, conn)| {
                        (
                            i,
                            conn.config.name.clone(),
                            conn.is_connected,
                            conn.config.database_path.clone(),
                            conn.config.created_at,
                        )
                    })
                    .collect();

                // Now render UI using the collected data
                for (index, name, is_connected, database_path, created_at) in connection_data {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let is_selected = self.current_connection_id == Some(index);
                            let button_text =
                                format!("{} {}", if is_selected { "▶️" } else { "  " }, name);

                            if ui.button(button_text).clicked() {
                                self.set_current_connection(index);
                            }

                            let status_text = if is_connected {
                                "🟢 Connected"
                            } else {
                                "🔴 Disconnected"
                            };
                            ui.label(status_text);
                        });

                        ui.indent(format!("conn_{}", index), |ui| {
                            ui.label(format!("Database: {}", database_path));
                            ui.label(format!("Created: {}", created_at.format("%Y-%m-%d %H:%M")));

                            if ui.button("Toggle Connection").clicked() {
                                if let Some(conn) = self.connections.get_mut(index) {
                                    conn.is_connected = !conn.is_connected;
                                    if conn.is_connected {
                                        self.show_info(format!("Connected to {}", name));
                                    } else {
                                        self.show_info(format!("Disconnected from {}", name));
                                    }
                                }
                            }
                        });
                    });
                }
            }
        });
    }

    /// Render main content area with tabs
    fn render_main_content(&mut self, ui: &mut Ui) {
        // Tab selection
        ui.horizontal(|ui| {
            if ui
                .selectable_value(
                    &mut self.active_tab,
                    ActiveTab::QueryEditor,
                    "📝 Query Editor",
                )
                .clicked()
            {}

            if ui
                .selectable_value(&mut self.active_tab, ActiveTab::Schema, "🏗️ Schema")
                .clicked()
            {}

            if ui
                .selectable_value(&mut self.active_tab, ActiveTab::Tables, "📊 Tables")
                .clicked()
            {}

            if ui
                .selectable_value(&mut self.active_tab, ActiveTab::History, "📚 History")
                .clicked()
            {}

            if ui
                .selectable_value(&mut self.active_tab, ActiveTab::Settings, "⚙️ Settings")
                .clicked()
            {}
        });

        ui.add_space(10.0);

        // Tab content
        match self.active_tab {
            ActiveTab::QueryEditor => {
                self.render_query_editor(ui);
                ui.add_space(10.0);
                self.render_results_panel(ui);
            }
            ActiveTab::Schema => {
                self.render_schema_browser(ui);
            }
            ActiveTab::Tables => {
                self.render_tables_view(ui);
            }
            ActiveTab::History => {
                self.render_history_view(ui);
            }
            ActiveTab::Settings => {
                self.render_settings_panel(ui);
            }
        }

        // Show messages
        if let Some(ref error) = self.error_message {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("❌ Error: {}", error)).color(egui::Color32::RED));
        }
        if let Some(ref info) = self.info_message {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(format!("ℹ️ {}", info)).color(egui::Color32::BLUE));
        }
    }

    /// Render query editor
    fn render_query_editor(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("📝 SQL Query Editor");
            ui.add_space(5.0);

            // Query input area
            let query_text_edit = TextEdit::multiline(&mut self.current_query)
                .hint_text("Enter your SQL query here...")
                .desired_rows(10)
                .font(egui::FontId::new(14.0, egui::FontFamily::Monospace));
            ui.add_sized(Vec2::new(ui.available_width(), 180.0), query_text_edit);

            ui.add_space(5.0);

            // Control buttons
            ui.horizontal(|ui| {
                if ui.button("▶️ Execute Query").clicked() {
                    self.execute_query();
                }

                if ui.button("🗑️ Clear").clicked() {
                    self.current_query.clear();
                    self.clear_messages();
                }

                // Sample query buttons
                ui.separator();
                ui.label("Sample queries:");
                if ui.button("SELECT *").clicked() {
                    self.current_query = "SELECT * FROM users;".to_string();
                }
                if ui.button("CREATE TABLE").clicked() {
                    self.current_query =
                        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT);"
                            .to_string();
                }
            });

            // Handle keyboard shortcuts
            ui.ctx().input(|input| {
                if input.key_pressed(egui::Key::Enter) && input.modifiers.ctrl {
                    self.execute_query();
                }
            });
        });
    }

    /// Render results panel
    fn render_results_panel(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("📊 Query Results");
            ui.add_space(5.0);

            if let Some(result) = self.query_results.first() {
                // Result header
                ui.horizontal(|ui| {
                    if result.success {
                        ui.label(
                            egui::RichText::new("✅ Query executed successfully")
                                .color(egui::Color32::GREEN),
                        );
                    } else {
                        ui.label(egui::RichText::new("❌ Query failed").color(egui::Color32::RED));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                        ui.label(format!("⏱️ {}ms", result.execution_time_ms));
                        ui.label(format!("🕐 {}", result.executed_at.format("%H:%M:%S")));
                    });
                });

                ui.add_space(5.0);

                // Query that was executed
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Executed Query:").strong());
                    ui.label(
                        egui::RichText::new(&result.query)
                            .font(egui::FontId::new(12.0, egui::FontFamily::Monospace)),
                    );
                });

                ui.add_space(5.0);

                // Results data
                if result.success {
                    if let Some(ref data) = result.data {
                        if data.is_empty() {
                            ui.label("Query returned no data");
                        } else {
                            // Show data in scrollable area
                            ScrollArea::vertical().show(ui, |ui| {
                                // Table headers
                                if !data.is_empty() {
                                    let headers: Vec<String> = data[0].keys().cloned().collect();
                                    ui.horizontal(|ui| {
                                        for header in &headers {
                                            ui.label(egui::RichText::new(header).strong());
                                            ui.separator();
                                        }
                                    });
                                    ui.add(egui::Separator::default());
                                }

                                // Data rows
                                for (_row_idx, row) in data.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        for (_key, value) in row {
                                            // Color code different data types
                                            let text = if value == "NULL" {
                                                egui::RichText::new(value)
                                                    .color(egui::Color32::GRAY)
                                            } else if value.parse::<i64>().is_ok() {
                                                egui::RichText::new(value)
                                                    .color(egui::Color32::BLUE)
                                            } else {
                                                egui::RichText::new(value)
                                            };
                                            ui.label(text);
                                            ui.separator();
                                        }
                                    });
                                }
                            });

                            ui.add_space(5.0);
                            ui.label(
                                egui::RichText::new(format!("Total rows: {}", data.len())).small(),
                            );
                        }
                    } else {
                        ui.label("Query executed successfully (no data returned)");
                    }
                } else {
                    // Error details
                    if let Some(ref error) = result.error_message {
                        ui.label(
                            egui::RichText::new(format!("Error: {}", error))
                                .color(egui::Color32::RED),
                        );
                    }
                }
            } else {
                ui.label("No query results yet. Execute a query to see results here.");
            }
        });
    }

    /// Render schema browser
    fn render_schema_browser(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("🏗️ Database Schema");
            ui.add_space(5.0);

            if let Some(connection) = self.current_connection() {
                if connection.is_connected {
                    ui.label("✅ Connected to database");
                    ui.add_space(10.0);

                    // Sample schema data
                    ui.collapsing("📋 Tables", |ui| {
                        ui.label("👤 users (User accounts)");
                        ui.label("📦 products (Product catalog)");
                        ui.label("🛒 orders (Order records)");
                        ui.label("📊 categories (Product categories)");
                    });

                    ui.collapsing("👁️ Views", |ui| {
                        ui.label("📈 user_stats (User statistics)");
                        ui.label("💰 sales_summary (Sales summary)");
                    });
                } else {
                    ui.label("🔴 Not connected to database");
                    ui.add_space(10.0);
                    if ui.button("🔗 Connect").clicked() {
                        if let Some(conn_index) = self.current_connection_id {
                            if let Some(conn) = self.connections.get_mut(conn_index) {
                                conn.is_connected = true;
                                self.show_info("Connected to database".to_string());
                            }
                        }
                    }
                }
            } else {
                ui.label("⚫ No active connection");
                ui.label("Select a connection from the left panel");
            }
        });
    }

    /// Render tables view
    fn render_tables_view(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("📊 Tables View");
            ui.add_space(5.0);
            ui.label("Table browser - select a table to view its data and structure");

            // Sample table listing
            ui.add_space(10.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("Available Tables:").strong());
                ui.add_space(5.0);

                let tables = ["users", "products", "orders", "categories"];
                for table in &tables {
                    ui.horizontal(|ui| {
                        ui.label(format!("📋 {}", table));
                        if ui.button("Browse").clicked() {
                            self.current_query = format!("SELECT * FROM {} LIMIT 100;", table);
                            self.active_tab = ActiveTab::QueryEditor;
                        }
                        if ui.button("Structure").clicked() {
                            self.current_query = format!("PRAGMA table_info({});", table);
                            self.active_tab = ActiveTab::QueryEditor;
                        }
                    });
                }
            });
        });
    }

    /// Render history view
    fn render_history_view(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("📚 Query History");
            ui.add_space(5.0);

            if self.query_results.is_empty() {
                ui.label("No queries executed yet");
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    for (idx, result) in self.query_results.iter().enumerate() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                if result.success {
                                    ui.label("✅");
                                } else {
                                    ui.label("❌");
                                }

                                ui.label(format!("#{} - {}ms", idx + 1, result.execution_time_ms));

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::RIGHT),
                                    |ui| {
                                        if ui.button("Load").clicked() {
                                            self.current_query = result.query.clone();
                                            self.active_tab = ActiveTab::QueryEditor;
                                        }
                                    },
                                );
                            });

                            ui.label(
                                egui::RichText::new(&result.query)
                                    .font(egui::FontId::new(12.0, egui::FontFamily::Monospace)),
                            );
                            ui.label(
                                egui::RichText::new(
                                    result.executed_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                )
                                .color(egui::Color32::GRAY)
                                .small(),
                            );
                        });

                        if idx < self.query_results.len() - 1 {
                            ui.add_space(5.0);
                        }
                    }
                });
            }
        });
    }

    /// Render settings panel
    fn render_settings_panel(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("⚙️ Settings");
            ui.add_space(5.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Appearance").strong());
                ui.add_space(5.0);

                ui.label("Theme:");
                ComboBox::from_id_source("theme_selector")
                    .selected_text(&self.theme)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.theme, "Light".to_string(), "☀️ Light");
                        ui.selectable_value(&mut self.theme, "Dark".to_string(), "🌙 Dark");
                    });
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Query Editor").strong());
                ui.add_space(5.0);

                ui.checkbox(&mut self.auto_commit, "Auto-execute queries on connection");
                ui.checkbox(&mut self.show_execution_time, "Show execution time");
            });
        });
    }
}
