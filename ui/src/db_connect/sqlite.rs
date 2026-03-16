use dioxus::prelude::*;
use models::{ConnectionRequest, SqliteFormData};

#[component]
pub fn SqliteForm() -> Element {
    let mut path = use_signal(|| "".to_string());
    let mut status = use_signal(|| "Idle".to_string());

    rsx! {
        div {
            input {
                value: "{path}",
                placeholder: "Path to sqlite file",
                oninput: move |event| {
                    path.set(event.value());
                }
            }

            button {
                onclick: move |_| {
                    let request = ConnectionRequest::Sqlite(SqliteFormData {
                        path: path(),
                    });

                    spawn(async move {
                        if path().is_empty() {
                            status.set("Empty".to_string());
                        }
                        else {
                            match services::connect_to_db(request).await {
                                Ok(_) => {
                                    status.set("Connected".to_string());
                                }
                                Err(err) => {
                                    status.set(format!("Error: {err:?}"));
                                }
                            }
                        }

                    });
                },
                "Connect"
            }

            p { "Status: {status}" }
        }
    }
}
