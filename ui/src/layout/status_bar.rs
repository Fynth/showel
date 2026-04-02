use crate::app_state::APP_STATE;
use dioxus::prelude::*;

pub fn status_bar_session_label(session_name: Option<&str>) -> String {
    match session_name {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => "No connection".to_string(),
    }
}

pub fn status_bar_session_count(count: usize) -> String {
    format!("Sessions {count}")
}

pub fn is_allowed_status_bar_item(text: &str) -> bool {
    let text = text.trim();

    if text.contains("Rust + Dioxus") || text.starts_with("Theme:") {
        return false;
    }

    if text.starts_with("Active:") {
        return false;
    }

    true
}

#[component]
pub fn StatusBar() -> Element {
    let (connection_label, session_count) = {
        let app_state = APP_STATE.read();
        let label = match app_state.active_session() {
            Some(session) => session.name.clone(),
            None => "No connection".to_string(),
        };
        (label, app_state.sessions.len())
    };

    rsx! {
        footer {
            class: "statusbar",
            span { class: "statusbar__item", "{connection_label}" }
            span { class: "statusbar__item", "Sessions {session_count}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_label_shows_name_without_prefix() {
        assert_eq!(status_bar_session_label(Some("My Database")), "My Database");
    }

    #[test]
    fn session_label_falls_back_to_no_connection() {
        assert_eq!(status_bar_session_label(None), "No connection");
        assert_eq!(status_bar_session_label(Some("")), "No connection");
    }

    #[test]
    fn session_count_formats_compactly() {
        assert_eq!(status_bar_session_count(0), "Sessions 0");
        assert_eq!(status_bar_session_count(3), "Sessions 3");
    }

    #[test]
    fn rejects_rust_dioxus_metadata() {
        assert!(!is_allowed_status_bar_item("Rust + Dioxus 0.7"));
        assert!(!is_allowed_status_bar_item("Rust + Dioxus 0.7.1"));
    }

    #[test]
    fn rejects_theme_metadata() {
        assert!(!is_allowed_status_bar_item("Theme: dark"));
        assert!(!is_allowed_status_bar_item("Theme: light"));
    }

    #[test]
    fn rejects_active_prefix() {
        assert!(!is_allowed_status_bar_item("Active: My Database"));
    }

    #[test]
    fn allows_session_and_connection_labels() {
        assert!(is_allowed_status_bar_item("My Database"));
        assert!(is_allowed_status_bar_item("No connection"));
        assert!(is_allowed_status_bar_item("Sessions 3"));
    }
}
