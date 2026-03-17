use crate::app_state::add_connection_session;
use dioxus::prelude::*;
use models::SavedConnection;

#[component]
pub fn RecentConnections(saved_connections: Option<Vec<SavedConnection>>) -> Element {
    let mut status = use_signal(String::new);

    rsx! {
        section {
            class: "connect-screen__recent",
            h2 { class: "connect-screen__section-title", "Recent Connections" }
            match saved_connections {
                Some(connections) if connections.is_empty() => rsx! {
                    p { class: "empty-state", "No saved connections yet." }
                },
                Some(connections) => rsx! {
                    for saved_connection in connections {
                        div {
                            class: "recent-connection",
                            div {
                                class: "recent-connection__meta",
                                p { class: "recent-connection__name", "{saved_connection.name}" }
                            }
                            button {
                                class: "button button--ghost",
                                onclick: {
                                    let request = saved_connection.request.clone();
                                    move |_| {
                                        let request_to_connect = request.clone();
                                        let request_to_save = request.clone();
                                        let request_to_register = request.clone();
                                        spawn(async move {
                                            match connection::connect_to_db(request_to_connect).await {
                                                Ok(connection) => {
                                                    let save_result =
                                                        storage::save_connection_request(request_to_save).await;
                                                    add_connection_session(request_to_register, connection);
                                                    match save_result {
                                                        Ok(()) => status.set("Connected".to_string()),
                                                        Err(err) => status.set(format!(
                                                            "Connected, but failed to update saved connections: {err}"
                                                        )),
                                                    }
                                                }
                                                Err(err) => {
                                                    status.set(format!("Error: {err:?}"));
                                                }
                                            }
                                        });
                                    }
                                },
                                "Connect"
                            }
                        }
                    }
                },
                None => rsx! {
                    p { class: "empty-state", "Loading saved connections..." }
                },
            }
            if !status().is_empty() {
                p { class: "connect-screen__status", "{status}" }
            }
        }
    }
}
