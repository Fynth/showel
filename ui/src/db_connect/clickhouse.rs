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
        div {
            input {
                value: "{host}",
                placeholder: "Host",
                oninput: move |event| host.set(event.value()),
            }

            input {
                value: "{port}",
                placeholder: "Port",
                oninput: move |event| port.set(event.value()),
            }

            input {
                value: "{username}",
                placeholder: "Username",
                oninput: move |event| username.set(event.value()),
            }

            input {
                r#type: "password",
                value: "{password}",
                placeholder: "Password",
                oninput: move |event| password.set(event.value()),
            }

            input {
                value: "{database}",
                placeholder: "Database",
                oninput: move |event| database.set(event.value()),
            }

            button {
                onclick: move |_| {
                    let request = ConnectionRequest::ClickHouse(ClickHouseFormData {
                        host: host(),
                        port: port().parse().unwrap_or(5432),
                        username: username(),
                        password: password(),
                        database: database(),
                    });

                    spawn(async move {
                        match services::connect_to_db(request).await {
                            Ok(_) => status.set("Connected".to_string()),
                            Err(err) => status.set(format!("Error: {err:?}")),
                        }
                    });
                },
                "Connect"
            }

            p { "Status: {status}" }
        }
    }
}
