use crate::app_state::{open_connection_screen, open_settings_modal, show_workspace, APP_STATE};
use dioxus::prelude::*;

const APP_ICON: &str = include_str!("../../../app/assets/icon.svg");

#[component]
pub fn Toolbar() -> Element {
    let (connection_label, has_sessions, show_connect_screen) = {
        let app_state = APP_STATE.read();
        let label = match app_state.active_session() {
            Some(session) => format!(
                "{} active · {} open",
                session.name,
                app_state.sessions.len()
            ),
            None => "No active connection".to_string(),
        };

        (
            label,
            app_state.has_sessions(),
            app_state.show_connection_screen,
        )
    };
    rsx! {
        header {
            class: "toolbar",
            div {
                class: "toolbar__brand",
                div {
                    class: "toolbar__logo",
                    dangerous_inner_html: APP_ICON,
                }
                div {
                    class: "toolbar__brand-copy",
                    span { class: "toolbar__eyebrow", "Database Client" }
                    strong { class: "toolbar__title", "Showel" }
                }
            }
            div {
                class: "toolbar__connection",
                span { class: "toolbar__connection-dot" }
                "{connection_label}"
            }
            div {
                class: "toolbar__actions",
                if has_sessions {
                    button {
                        class: if show_connect_screen {
                            "button button--ghost button--small"
                        } else {
                            "button button--primary button--small"
                        },
                        onclick: move |_| {
                            if show_connect_screen {
                                show_workspace();
                            } else {
                                open_connection_screen();
                            }
                        },
                        if show_connect_screen { "Back to Workspace" } else { "New Connection" }
                    }
                }
                button {
                    class: "button button--ghost button--small",
                    onclick: move |_| open_settings_modal(),
                    "Settings"
                }
            }
        }
    }
}
