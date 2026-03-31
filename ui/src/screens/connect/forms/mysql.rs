use crate::app_state::add_connection_session;
use dioxus::prelude::*;
use models::{ConnectionRequest, MySqlFormData, SshTunnelConfig};

use super::{SshTunnelFields, connection_status_class};

#[component]
pub fn MySqlForm() -> Element {
    let mut host = use_signal(|| "localhost".to_string());
    let mut port = use_signal(|| "3306".to_string());
    let mut username = use_signal(|| "root".to_string());
    let mut password = use_signal(|| "".to_string());
    let mut database = use_signal(String::new);
    let ssh_enabled = use_signal(|| false);
    let ssh_host = use_signal(String::new);
    let ssh_port = use_signal(|| "22".to_string());
    let ssh_username = use_signal(String::new);
    let ssh_private_key_path = use_signal(String::new);
    let mut status = use_signal(|| "Idle".to_string());
    let status_value = status();
    let status_class = connection_status_class(&status_value);

    rsx! {
        form {
            class: "connect-form",
            onsubmit: move |event| {
                event.prevent_default();

                status.set("Connecting...".to_string());
                let request = ConnectionRequest::MySql(MySqlFormData {
                    host: host(),
                    port: port().parse().unwrap_or(3306),
                    username: username(),
                    password: password(),
                    database: database(),
                    ssh_tunnel: if ssh_enabled() {
                        Some(SshTunnelConfig {
                            host: ssh_host(),
                            port: ssh_port().parse().unwrap_or(22),
                            username: ssh_username(),
                            private_key_path: ssh_private_key_path(),
                        })
                    } else {
                        None
                    },
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
                        Err(err) => status.set(format!("Error: {err:?}")),
                    }
                });
            },
            div {
                class: "connect-form__grid",
                div {
                    class: "field",
                    label { class: "field__label", r#for: "mysql-host", "Host" }
                    input {
                        class: "input",
                        id: "mysql-host",
                        value: "{host}",
                        placeholder: "localhost or mysql://user:pass@host:3306/db",
                        oninput: move |event| host.set(event.value()),
                    }
                }

                div {
                    class: "field",
                    label { class: "field__label", r#for: "mysql-port", "Port" }
                    input {
                        class: "input",
                        id: "mysql-port",
                        value: "{port}",
                        placeholder: "3306",
                        oninput: move |event| port.set(event.value()),
                    }
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "mysql-username", "Username" }
                input {
                    class: "input",
                    id: "mysql-username",
                    value: "{username}",
                    placeholder: "root",
                    oninput: move |event| username.set(event.value()),
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "mysql-password", "Password" }
                input {
                    class: "input",
                    id: "mysql-password",
                    r#type: "password",
                    value: "{password}",
                    placeholder: "••••••••",
                    oninput: move |event| password.set(event.value()),
                }
            }

            div {
                class: "field",
                label { class: "field__label", r#for: "mysql-database", "Database" }
                input {
                    class: "input",
                    id: "mysql-database",
                    value: "{database}",
                    placeholder: "Optional default database",
                    oninput: move |event| database.set(event.value()),
                }
            }

            SshTunnelFields {
                enabled: ssh_enabled,
                host: ssh_host,
                port: ssh_port,
                username: ssh_username,
                private_key_path: ssh_private_key_path,
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
