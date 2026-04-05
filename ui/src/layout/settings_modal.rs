use crate::{
    app_state::{
        APP_SHOW_HISTORY, APP_SHOW_SETTINGS_MODAL, APP_SQL_FORMAT_SETTINGS, APP_THEME,
        APP_UI_SETTINGS, close_settings_modal,
    },
    screens::SqlFormatSettingsFields,
};
use dioxus::prelude::*;
use models::AppThemePreference;

#[component]
#[allow(clippy::redundant_closure)]
pub fn SettingsModal() -> Element {
    if !APP_SHOW_SETTINGS_MODAL() {
        return VNode::empty();
    }

    let mut sql_format_settings = use_signal(|| APP_SQL_FORMAT_SETTINGS());
    let settings = APP_UI_SETTINGS();

    use_effect(move || {
        let settings = sql_format_settings();
        if APP_SQL_FORMAT_SETTINGS() != settings {
            *APP_SQL_FORMAT_SETTINGS.write() = settings;
        }
    });

    rsx! {
        div {
            class: "settings-modal__backdrop",
            onclick: move |_| close_settings_modal(),
            div {
                class: "settings-modal",
                onclick: move |event| event.stop_propagation(),
                div {
                    class: "settings-modal__header",
                    div {
                        class: "settings-modal__header-copy",
                        h2 { class: "settings-modal__title", "Settings" }
                        p {
                            class: "settings-modal__hint",
                            "Theme, workspace defaults and SQL formatting are saved automatically."
                        }
                    }
                    button {
                        class: "button button--ghost button--small",
                        onclick: move |_| close_settings_modal(),
                        "Close"
                    }
                }

                div {
                    class: "settings-modal__body",
                    section {
                        class: "settings-modal__section",
                        div {
                            class: "settings-modal__section-header",
                            h3 { class: "settings-modal__section-title", "Appearance" }
                        }
                        div {
                            class: "settings-modal__segmented",
                            button {
                                class: if settings.theme == AppThemePreference::Dark {
                                    "button button--ghost button--small button--active"
                                } else {
                                    "button button--ghost button--small"
                                },
                                onclick: move |_| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.theme = AppThemePreference::Dark;
                                    });
                                    *APP_THEME.write() = AppThemePreference::Dark.css_class().to_string();
                                },
                                "Dark"
                            }
                            button {
                                class: if settings.theme == AppThemePreference::Light {
                                    "button button--ghost button--small button--active"
                                } else {
                                    "button button--ghost button--small"
                                },
                                onclick: move |_| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.theme = AppThemePreference::Light;
                                    });
                                    *APP_THEME.write() = AppThemePreference::Light.css_class().to_string();
                                },
                                "Light"
                            }
                        }
                    }

                    section {
                        class: "settings-modal__section",
                        div {
                            class: "settings-modal__section-header",
                            h3 { class: "settings-modal__section-title", "Workspace" }
                            button {
                                class: "button button--ghost button--small",
                                onclick: move |_| {
                                    let defaults = models::AppUiSettings::default();
                                    *APP_SHOW_HISTORY.write() = defaults.show_history;
                                    *APP_THEME.write() = defaults.theme.css_class().to_string();
                                    *APP_UI_SETTINGS.write() = defaults;
                                },
                                "Reset UI"
                            }
                        }
                        div {
                            class: "settings-modal__grid",
                            div {
                                class: "field",
                                span { class: "field__label", "Default page size" }
                                input {
                                    class: "input",
                                    r#type: "number",
                                    min: "10",
                                    max: "1000",
                                    value: "{settings.default_page_size}",
                                    oninput: move |event| {
                                        APP_UI_SETTINGS.with_mut(|current| {
                                            current.default_page_size = parse_u32_in_range(
                                                &event.value(),
                                                current.default_page_size,
                                                10,
                                                1000,
                                            );
                                        });
                                    },
                                }
                            }
                        }
                        p {
                            class: "settings-modal__section-hint",
                            "Tool panels can be dragged between the left sidebar and the right inspector."
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.ai_features_enabled,
                                oninput: move |event| {
                                    let enabled = event.checked();
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.ai_features_enabled = enabled;
                                        if !enabled {
                                            current.show_agent_panel = false;
                                        }
                                    });
                                },
                            }
                            span { "Enable AI features (ACP panel, prompts, and SQL actions)" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.restore_session_on_launch,
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.restore_session_on_launch = event.checked();
                                    });
                                },
                            }
                            span { "Restore previous session on launch" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.show_saved_queries,
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.show_saved_queries = event.checked();
                                    });
                                },
                            }
                            span { "Show saved queries panel by default" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.show_connections,
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.show_connections = event.checked();
                                    });
                                },
                            }
                            span { "Show connections panel by default" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.show_explorer,
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.show_explorer = event.checked();
                                    });
                                },
                            }
                            span { "Show explorer by default" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.show_history,
                                oninput: move |event| {
                                    let checked = event.checked();
                                    *APP_SHOW_HISTORY.write() = checked;
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.show_history = checked;
                                    });
                                },
                            }
                            span { "Show history by default" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.show_sql_editor,
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.show_sql_editor = event.checked();
                                    });
                                },
                            }
                            span { "Show SQL editor by default" }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.show_agent_panel,
                                disabled: !settings.ai_features_enabled,
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.show_agent_panel = event.checked();
                                    });
                                },
                            }
                            span { "Show ACP agent panel by default" }
                        }
                    }

                    section {
                        class: "settings-modal__section",
                        div {
                            class: "settings-modal__section-header",
                            div {
                                h3 { class: "settings-modal__section-title", "SQL Formatting" }
                                p {
                                    class: "settings-modal__section-hint",
                                    "Controls keyword case, wrapping, joins and inline arguments."
                                }
                            }
                            button {
                                class: "button button--ghost button--small",
                                onclick: move |_| sql_format_settings.set(models::SqlFormatSettings::default()),
                                "Reset SQL"
                            }
                        }
                        SqlFormatSettingsFields {
                            settings: sql_format_settings,
                        }
                    }

                    section {
                        class: "settings-modal__section",
                        div {
                            class: "settings-modal__section-header",
                            h3 { class: "settings-modal__section-title", "CodeStral Completion" }
                            p {
                                class: "settings-modal__section-hint",
                                "AI-powered SQL code completion via CodeStral API."
                            }
                        }
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: settings.codestral.enabled,
                                disabled: settings.codestral.api_key.is_empty(),
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.codestral.enabled = event.checked();
                                    });
                                },
                            }
                            span { "Enable CodeStral inline completion" }
                        }
                        div {
                            class: "field",
                            span { class: "field__label", "API Key" }
                            input {
                                class: "input",
                                r#type: "password",
                                placeholder: "sk-...",
                                value: "{settings.codestral.api_key}",
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.codestral.api_key = event.value();
                                        if current.codestral.api_key.is_empty() {
                                            current.codestral.enabled = false;
                                        }
                                    });
                                },
                            }
                        }
                        div {
                            class: "field",
                            span { class: "field__label", "Model" }
                            input {
                                class: "input",
                                placeholder: "codestral-latest",
                                value: "{settings.codestral.model}",
                                oninput: move |event| {
                                    APP_UI_SETTINGS.with_mut(|current| {
                                        current.codestral.model = event.value();
                                    });
                                },
                            }
                        }
                        if settings.codestral.api_key.is_empty() {
                            p {
                                class: "settings-modal__section-hint",
                                "Enter an API key to enable CodeStral completion. Get your key from "
                                a {
                                    href: "https://codestral.mistral.ai/",
                                    target: "_blank",
                                    "codestral.mistral.ai"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn parse_u32_in_range(value: &str, fallback: u32, min: u32, max: u32) -> u32 {
    value
        .parse::<u32>()
        .map(|parsed| parsed.clamp(min, max))
        .unwrap_or(fallback)
}
