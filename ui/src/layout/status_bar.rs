use crate::app_state::{APP_STATE, APP_THEME};
use dioxus::prelude::*;

#[component]
pub fn StatusBar() -> Element {
    let (connection_label, session_count) = {
        let app_state = APP_STATE.read();
        let label = match app_state.active_session() {
            Some(session) => format!("Active: {}", session.name),
            None => "Disconnected".to_string(),
        };
        (label, app_state.sessions.len())
    };
    let theme_name = APP_THEME();

    rsx! {
        footer {
            class: "statusbar",
            span { class: "statusbar__item", "{connection_label}" }
            span { class: "statusbar__item", "Sessions: {session_count}" }
            span { class: "statusbar__item", "Theme: {theme_name}" }
            span { class: "statusbar__item", "Rust + Dioxus 0.7" }
        }
    }
}
