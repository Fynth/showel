use crate::app_state::{APP_STATE, open_connection_screen, open_settings_modal, show_workspace};
use dioxus::{desktop::use_window, html::input_data::MouseButton, prelude::*};

const APP_ICON: &str = include_str!("../../../app/assets/icon.svg");

#[component]
pub fn Toolbar() -> Element {
    let desktop = use_window();
    let desktop_drag = desktop.clone();
    let desktop_toggle = desktop.clone();
    let desktop_minimize = desktop.clone();
    let desktop_maximize = desktop.clone();
    let desktop_close = desktop.clone();
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
                class: "toolbar__drag",
                onmousedown: move |event| {
                    if event.trigger_button() == Some(MouseButton::Primary) {
                        desktop_drag.drag();
                    }
                },
                ondoubleclick: move |_| desktop_toggle.toggle_maximized(),
                div {
                    class: "toolbar__brand",
                    div {
                        class: "toolbar__logo",
                        dangerous_inner_html: APP_ICON,
                    }
                    div {
                        class: "toolbar__brand-copy",
                        span { class: "toolbar__eyebrow", "Database Client" }
                        strong { class: "toolbar__title", "Shovel" }
                    }
                }
                div {
                    class: "toolbar__connection",
                    span { class: "toolbar__connection-dot" }
                    "{connection_label}"
                }
                div { class: "toolbar__spacer" }
            }
            div {
                class: "toolbar__actions",
                onmousedown: move |event| event.stop_propagation(),
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
            div {
                class: "toolbar__window-controls",
                onmousedown: move |event| event.stop_propagation(),
                button {
                    class: "toolbar__window-button",
                    title: "Minimize",
                    onclick: move |_| desktop_minimize.set_minimized(true),
                    span { class: "toolbar__window-symbol toolbar__window-symbol--minimize" }
                }
                button {
                    class: "toolbar__window-button",
                    title: "Maximize",
                    onclick: move |_| desktop_maximize.toggle_maximized(),
                    span { class: "toolbar__window-symbol toolbar__window-symbol--maximize" }
                }
                button {
                    class: "toolbar__window-button toolbar__window-button--close",
                    title: "Close",
                    onclick: move |_| desktop_close.close(),
                    span { class: "toolbar__window-symbol toolbar__window-symbol--close" }
                }
            }
        }
    }
}
