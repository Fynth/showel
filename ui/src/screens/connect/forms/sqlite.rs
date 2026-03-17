use crate::app_state::add_connection_session;
use dioxus::prelude::*;
use models::{ConnectionRequest, SqliteFormData};

#[component]
pub fn SqliteForm() -> Element {
    let mut path = use_signal(|| "".to_string());
    let mut status = use_signal(|| "Idle".to_string());

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
                    match services::connect_to_db(request.clone()).await {
                        Ok(connection) => {
                            add_connection_session(request.clone(), connection);
                            match services::save_connection_request(request).await {
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
                input {
                    class: "input",
                    id: "sqlite-path",
                    value: "{path}",
                    placeholder: "/path/to/app.db",
                    oninput: move |event| {
                        path.set(event.value());
                    }
                }
            }

            button {
                class: "button button--primary",
                r#type: "submit",
                "Connect"
            }

            p { class: "connect-screen__status", "Status: {status}" }
        }
    }
}
