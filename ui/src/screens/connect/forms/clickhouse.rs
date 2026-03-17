use crate::app_state::add_connection_session;
use dioxus::prelude::*;
use models::{ClickHouseFormData, ConnectionRequest};

#[component]
pub fn ClickHouseForm() -> Element {
    let mut host = use_signal(|| "".to_string());
    let mut port = use_signal(|| "5001".to_string());
    let mut username = use_signal(|| "".to_string());
    let mut password = use_signal(|| "".to_string());
    let mut database = use_signal(|| "".to_string());
    let mut status = use_signal(|| "Idle".to_string());

    rsx! {
        form {
            class: "connect-form",
            onsubmit: move |event| event.prevent_default(),
            div {
                class: "connect-form__grid",
                div {
                    class: "field",
                    label { class: "field__label", r#for: "ch-host", "Host" }
                    input {
                        class: "input",
                        id: "ch-host",
                        value: "{host}",
                        placeholder: "localhost",
                        oninput: move |event| host.set(event.value()),
                    }
                }

                div {
                    class: "field",
                    label { class: "field__label", r#for: "ch-port", "Port" }
                    input {
                        class: "input",
                        id: "ch-port",
                        value: "{port}",
                        placeholder: "8123",
                        oninput: move |event| port.set(event.value()),
                    }
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "ch-username", "Username" }
                input {
                    class: "input",
                    id: "ch-username",
                    value: "{username}",
                    placeholder: "default",
                    oninput: move |event| username.set(event.value()),
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "ch-password", "Password" }
                input {
                    class: "input",
                    id: "ch-password",
                    r#type: "password",
                    value: "{password}",
                    placeholder: "••••••••",
                    oninput: move |event| password.set(event.value()),
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "ch-database", "Database" }
                input {
                    class: "input",
                    id: "ch-database",
                    value: "{database}",
                    placeholder: "default",
                    oninput: move |event| database.set(event.value()),
                }
            }

            button {
                class: "button button--primary",
                onclick: move |_| {
                    let request = ConnectionRequest::ClickHouse(ClickHouseFormData {
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
                "Connect"
            }

            p { class: "connect-screen__status", "Status: {status}" }
        }
    }
}
