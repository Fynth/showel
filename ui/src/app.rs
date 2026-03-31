use crate::{
    app_state::{
        APP_SHOW_HISTORY, APP_SHOW_SETTINGS_MODAL, APP_SQL_FORMAT_SETTINGS, APP_STATE, APP_THEME,
        APP_TOOLTIP, APP_UI_SETTINGS, restore_connection_sessions,
    },
    layout::{SettingsModal, StatusBar, ToastContainer, Toolbar},
    screens::{DbConnect, Workspace},
};
use dioxus::prelude::*;
use futures_util::future::join_all;
use models::{AppUiSettings, SqlFormatSettings};

#[component]
pub fn App() -> Element {
    let mut restored_once = use_signal(|| false);
    let mut ui_settings_loaded = use_signal(|| false);
    let mut sql_settings_loaded = use_signal(|| false);
    let mut last_saved_ui_settings = use_signal(|| None::<AppUiSettings>);
    let mut last_saved_sql_settings = use_signal(|| None::<SqlFormatSettings>);
    let persisted_ui_settings =
        use_resource(
            move || async move { storage::load_app_ui_settings().await.unwrap_or_default() },
        );
    let persisted_sql_settings = use_resource(move || async move {
        storage::load_sql_format_settings()
            .await
            .unwrap_or_default()
    });

    use_effect(move || {
        let Some(settings) = persisted_ui_settings() else {
            return;
        };
        if ui_settings_loaded() {
            return;
        }

        *APP_UI_SETTINGS.write() = settings.clone();
        *APP_THEME.write() = settings.theme.css_class().to_string();
        *APP_SHOW_HISTORY.write() = settings.show_history;
        last_saved_ui_settings.set(Some(settings.clone()));
        ui_settings_loaded.set(true);

        if restored_once() || !settings.restore_session_on_launch {
            restored_once.set(true);
            return;
        }

        restored_once.set(true);
        spawn(async move {
            let Ok((open_requests, active_connection_name)) = storage::load_session_state().await
            else {
                return;
            };
            if open_requests.is_empty() {
                return;
            }

            let restored = join_all(open_requests.into_iter().map(|request| async move {
                connection::connect_to_db(request.clone())
                    .await
                    .ok()
                    .map(|connection| (request, connection))
            }))
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

            if !restored.is_empty() {
                restore_connection_sessions(restored, active_connection_name);
            }
        });
    });

    use_effect(move || {
        let Some(settings) = persisted_sql_settings() else {
            return;
        };
        if sql_settings_loaded() {
            return;
        }

        *APP_SQL_FORMAT_SETTINGS.write() = settings.clone();
        last_saved_sql_settings.set(Some(settings));
        sql_settings_loaded.set(true);
    });

    use_effect(move || {
        if !ui_settings_loaded() {
            return;
        }

        let settings = APP_UI_SETTINGS();
        *APP_THEME.write() = settings.theme.css_class().to_string();
        *APP_SHOW_HISTORY.write() = settings.show_history;

        if last_saved_ui_settings().as_ref() == Some(&settings) {
            return;
        }

        last_saved_ui_settings.set(Some(settings.clone()));
        spawn(async move {
            let _ = storage::save_app_ui_settings(settings).await;
        });
    });

    use_effect(move || {
        if !sql_settings_loaded() {
            return;
        }

        let settings = APP_SQL_FORMAT_SETTINGS();
        if last_saved_sql_settings().as_ref() == Some(&settings) {
            return;
        }

        last_saved_sql_settings.set(Some(settings.clone()));
        spawn(async move {
            let _ = storage::save_sql_format_settings(settings).await;
        });
    });

    let theme_name = APP_THEME();
    let (has_sessions, should_show_connect) = {
        let app_state = APP_STATE.read();
        (
            app_state.has_sessions(),
            app_state.show_connection_screen || !app_state.has_sessions(),
        )
    };

    rsx! {
        div {
            class: "app {theme_name}",
            Toolbar {}
            main {
                class: if has_sessions {
                    "app__body"
                } else {
                    "app__body app__body--welcome"
                },
                if has_sessions {
                    ErrorBoundary {
                        handle_error: |_| {
                            rsx! {
                                div {
                                    class: "workspace-error",
                                    p { "Something went wrong. Please restart the application." }
                                }
                            }
                        },
                        Workspace {}
                    }
                    if should_show_connect {
                        div {
                            class: "app__overlay",
                            DbConnect {}
                        }
                    }
                } else {
                    DbConnect {}
                }
                if APP_SHOW_SETTINGS_MODAL() {
                    SettingsModal {}
                }
                if let Some(tooltip) = APP_TOOLTIP() {
                    div {
                        class: "app__tooltip-layer",
                        div {
                            class: "app__tooltip",
                            style: format!("left: {:.0}px; top: {:.0}px;", tooltip.x, tooltip.y),
                            "{tooltip.label}"
                        }
                    }
                }
                ToastContainer {}
            }
            StatusBar {}
        }
    }
}
