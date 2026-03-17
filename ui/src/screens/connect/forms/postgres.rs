use crate::app_state::add_connection_session;
use dioxus::prelude::*;
use models::{ConnectionRequest, PostgresFormData};

#[component]
pub fn PostgresForm() -> Element {
    let mut host = use_signal(|| "localhost".to_string());
    let mut port = use_signal(|| "5432".to_string());
    let mut username = use_signal(|| "postgres".to_string());
    let mut password = use_signal(|| "".to_string());
    let mut database = use_signal(|| "postgres".to_string());
    let mut status = use_signal(|| "Idle".to_string());

    rsx! {
        form {
            class: "connect-form",
            onsubmit: move |event| {
                event.prevent_default();

                status.set("Connecting...".to_string());
                let request = ConnectionRequest::Postgres(PostgresFormData {
                    host: host(),
                    port: port().parse().unwrap_or(5432),
                    username: username(),
                    password: password(),
                    database: database(),
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
                        Err(err) => status.set(format!("Error: {err:?}")),
                    }
                });
            },
            div {
                class: "connect-form__grid",
                div {
                    class: "field",
                    label { class: "field__label", r#for: "pg-host", "Host" }
                    input {
                        class: "input",
                        id: "pg-host",
                        value: "{host}",
                        placeholder: "localhost or postgres://user:pass@host:5432/db",
                        oninput: move |event| host.set(event.value()),
                    }
                }

                div {
                    class: "field",
                    label { class: "field__label", r#for: "pg-port", "Port" }
                    input {
                        class: "input",
                        id: "pg-port",
                        value: "{port}",
                        placeholder: "5432",
                        oninput: move |event| port.set(event.value()),
                    }
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "pg-username", "Username" }
                input {
                    class: "input",
                    id: "pg-username",
                    value: "{username}",
                    placeholder: "postgres",
                    oninput: move |event| username.set(event.value()),
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "pg-password", "Password" }
                input {
                    class: "input",
                    id: "pg-password",
                    r#type: "password",
                    value: "{password}",
                    placeholder: "••••••••",
                    oninput: move |event| password.set(event.value()),
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "pg-database", "Database" }
                input {
                    class: "input",
                    id: "pg-database",
                    value: "{database}",
                    placeholder: "app",
                    oninput: move |event| database.set(event.value()),
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
