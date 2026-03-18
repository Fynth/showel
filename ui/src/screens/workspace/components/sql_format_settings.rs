use dioxus::prelude::*;
use models::{SqlFormatSettings, SqlKeywordCase};

#[component]
pub fn SqlFormatSettingsPanel(
    mut settings: Signal<SqlFormatSettings>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "editor__format-settings",
            div {
                class: "editor__format-settings-header",
                div {
                    class: "editor__format-settings-copy",
                    h3 { class: "editor__format-settings-title", "SQL Formatting" }
                    p {
                        class: "editor__format-settings-hint",
                        "Settings are saved automatically. Long SQL is moved to a new line after the configured character limit. Leave the inline-arguments field empty for no limit."
                    }
                }
                button {
                    class: "button button--ghost button--small",
                    onclick: move |_| settings.set(SqlFormatSettings::default()),
                    "Reset"
                }
                button {
                    class: "button button--ghost button--small",
                    onclick: move |_| on_close.call(()),
                    "Close"
                }
            }

            div {
                class: "editor__format-settings-grid",
                div {
                    class: "field",
                    span { class: "field__label", "Formatting mode" }
                    select {
                        class: "input",
                        value: if settings().inline { "inline" } else { "multiline" },
                        oninput: move |event| {
                            settings.with_mut(|current| {
                                current.inline = event.value() == "inline";
                            });
                        },
                        option { value: "multiline", "Multiline" }
                        option { value: "inline", "Single line" }
                    }
                }

                div {
                    class: "field",
                    span { class: "field__label", "Keyword case" }
                    select {
                        class: "input",
                        value: keyword_case_value(settings().keyword_case),
                        oninput: move |event| {
                            settings.with_mut(|current| {
                                current.keyword_case = parse_keyword_case(event.value());
                            });
                        },
                        option { value: "uppercase", "Uppercase" }
                        option { value: "lowercase", "Lowercase" }
                        option { value: "preserve", "Preserve" }
                    }
                }

                div {
                    class: "field",
                    span { class: "field__label", "Indent width" }
                    input {
                        class: "input",
                        r#type: "number",
                        min: "1",
                        max: "8",
                        value: "{settings().indent_width}",
                        oninput: move |event| {
                            settings.with_mut(|current| {
                                current.indent_width =
                                    parse_u8_in_range(&event.value(), current.indent_width, 1, 8);
                            });
                        },
                    }
                }

                div {
                    class: "field",
                    span { class: "field__label", "Blank lines between queries" }
                    input {
                        class: "input",
                        r#type: "number",
                        min: "0",
                        max: "4",
                        value: "{settings().lines_between_queries}",
                        oninput: move |event| {
                            settings.with_mut(|current| {
                                current.lines_between_queries = parse_u8_in_range(
                                    &event.value(),
                                    current.lines_between_queries,
                                    0,
                                    4,
                                );
                            });
                        },
                    }
                }

                div {
                    class: "field",
                    span { class: "field__label", "Wrap to new line after" }
                    input {
                        class: "input",
                        r#type: "number",
                        min: "20",
                        max: "255",
                        disabled: settings().inline,
                        value: "{settings().max_inline_block}",
                        oninput: move |event| {
                            settings.with_mut(|current| {
                                let wrap_width = parse_u8_in_range(
                                    &event.value(),
                                    current.max_inline_block,
                                    20,
                                    255,
                                );
                                current.max_inline_block = wrap_width;
                                current.max_inline_top_level = Some(wrap_width);
                            });
                        },
                    }
                }

                div {
                    class: "field",
                    span { class: "field__label", "Keep arguments inline up to" }
                    input {
                        class: "input",
                        r#type: "number",
                        min: "1",
                        max: "255",
                        disabled: settings().inline,
                        placeholder: "No limit",
                        value: "{inline_arguments_input_value(settings().max_inline_arguments)}",
                        oninput: move |event| {
                            settings.with_mut(|current| {
                                current.max_inline_arguments =
                                    parse_optional_u8_in_range(
                                        &event.value(),
                                        current.max_inline_arguments,
                                        1,
                                        255,
                                    );
                            });
                        },
                    }
                }

            }

            label {
                class: "editor__format-settings-toggle",
                input {
                    r#type: "checkbox",
                    checked: settings().joins_as_top_level,
                    disabled: settings().inline,
                    oninput: move |event| {
                        settings.with_mut(|current| {
                            current.joins_as_top_level = event.checked();
                        });
                    },
                }
                span { "Move JOIN clauses to top level" }
            }
        }
    }
}

fn keyword_case_value(case: SqlKeywordCase) -> &'static str {
    match case {
        SqlKeywordCase::Uppercase => "uppercase",
        SqlKeywordCase::Lowercase => "lowercase",
        SqlKeywordCase::Preserve => "preserve",
    }
}

fn parse_keyword_case(value: String) -> SqlKeywordCase {
    match value.as_str() {
        "lowercase" => SqlKeywordCase::Lowercase,
        "preserve" => SqlKeywordCase::Preserve,
        _ => SqlKeywordCase::Uppercase,
    }
}

fn inline_arguments_input_value(value: Option<u8>) -> String {
    match value {
        Some(limit) => limit.to_string(),
        None => String::new(),
    }
}

fn parse_u8_in_range(value: &str, fallback: u8, min: u8, max: u8) -> u8 {
    value
        .parse::<u8>()
        .map(|parsed| parsed.clamp(min, max))
        .unwrap_or(fallback)
}

fn parse_optional_u8_in_range(value: &str, fallback: Option<u8>, min: u8, max: u8) -> Option<u8> {
    if value.trim().is_empty() {
        None
    } else {
        value
            .parse::<u8>()
            .map(|parsed| parsed.clamp(min, max))
            .ok()
            .or(fallback)
    }
}
