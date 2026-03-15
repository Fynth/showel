use dioxus::prelude::*;

#[component]
pub fn App() -> Element {
    let mut info = use_signal(|| "".to_string());
    let mut status = use_signal(|| "Idle".to_string());
    let mut query = use_signal(|| "".to_string());
    let mut db_type = use_signal(|| "sqlite".to_string());
    let mut sqlite_path = use_signal(|| "".to_string());

    let mut host = use_signal(|| "".to_string());
    let mut port = use_signal(|| "5432".to_string());
    let mut username = use_signal(|| "".to_string());
    let mut password = use_signal(|| "".to_string());
    let mut database = use_signal(|| "".to_string());
    let config = match db_type().as_str() {
        "sqlite" => models::DatabaseConfig::Sqlite(sqlite_path()),
        "postgres" => models::DatabaseConfig::Postgres {
            host: host(),
            port: port().parse().unwrap_or(5432),
            username: username(),
            password: password(),
            database: database(),
        },
        _ => {
            status.set("Unknown database type".to_string());
            return rsx! { div {} };
        }
    };
    rsx! {
            div {display: "flex",
                p { "Status: {status}" }
            input {value: "{info}",
                oninput: move |event| {
                    info.set(event.value());
                }
            }
            button {
                onclick: move |_| {
                spawn(async move {
                    match services::connect_to_db(config).await {
                        Ok(_pool) => {
                            status.set("Connected".to_string());
                        }
                        Err(err) => {
                            status.set(format!("Error: {err:?}"));
                        }
                    }
                });
            },
            p {},
            "connect"
        }}
            div {
                input {
                value: "{query}",
                oninput: move |event| {
                    query.set(event.value());
                }}
            }
    }
}
