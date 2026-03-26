use crate::app_state::add_connection_session;
use dioxus::prelude::*;
use models::{ConnectionRequest, SqliteFormData};
use rfd::AsyncFileDialog;

use super::connection_status_class;

#[component]
pub fn SqliteForm() -> Element {
    let mut path = use_signal(|| "".to_string());
    let mut status = use_signal(|| "Idle".to_string());
    let status_value = status();
    let status_class = connection_status_class(&status_value);

    rsx! {
        form {
            class: "connect-form",
            onsubmit: move |event| {
                event.prevent_default();

                let current_path = path().trim().to_string();
                if current_path.is_empty() {
                    status.set("Config is empty".to_string());
                    return;
                }

                status.set("Connecting...".to_string());
                let request = ConnectionRequest::Sqlite(SqliteFormData {
                    path: current_path,
                });

                spawn(async move {
                    match connection::connect_to_db(request.clone()).await {
                        Ok(connection) => {
                            let save_result =
                                storage::save_connection_request(request.clone()).await;
                            add_connection_session(request, connection);
                            match save_result {
                                Ok(()) => status.set("Connected".to_string()),
                                Err(err) => status.set(format!(
                                    "Connected, but failed to save connection: {err}"
                                )),
                            }
                        }
                        Err(err) => {
                            status.set(format!("Error: {err:?}"));
                        }
                    }
                });
            },
            div {
                class: "field",
                label {
                    class: "field__label",
                    r#for: "sqlite-path",
                    "SQLite file path"
                }
                div {
                    class: "connect-form__path-row",
                    input {
                        class: "input connect-form__path-input",
                        id: "sqlite-path",
                        value: "{path}",
                        placeholder: "/path/to/app.db",
                        oninput: move |event| {
                            path.set(event.value());
                        }
                    }
                    button {
                        class: "button button--ghost",
                        r#type: "button",
                        onclick: move |_| {
                            spawn(async move {
                                let file = AsyncFileDialog::new()
                                    .add_filter("SQLite database", &["db", "sqlite", "sqlite3", "db3"])
                                    .add_filter("All files", &["*"])
                                    .pick_file()
                                    .await;

                                if let Some(file) = file {
                                    path.set(file.path().display().to_string());
                                }
                            });
                        },
                        "Browse"
                    }
                }
            }

            div {
                class: "connect-form__actions",
                button {
                    class: "button button--primary connect-form__submit",
                    r#type: "submit",
                    "Connect"
                }
                p { class: "{status_class}", "Status: {status_value}" }
            }
        }
    }
}
