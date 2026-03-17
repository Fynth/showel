use dioxus::prelude::*;
use models::QueryTabState;
use std::cell::RefCell;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::screens::workspace::actions::update_active_tab_sql;

const SQL_HIGHLIGHT_NAMES: [&str; 21] = [
    "attribute",
    "boolean",
    "comment",
    "conditional",
    "field",
    "float",
    "function.call",
    "keyword",
    "keyword.operator",
    "number",
    "operator",
    "parameter",
    "punctuation.bracket",
    "punctuation.delimiter",
    "spell",
    "storageclass",
    "string",
    "type",
    "type.builtin",
    "type.qualifier",
    "variable",
];

#[derive(Clone, PartialEq)]
struct SqlHighlightSegment {
    class_name: &'static str,
    text: String,
}

thread_local! {
    static SQL_HIGHLIGHT_CONFIG: RefCell<Option<HighlightConfiguration>> =
        RefCell::new(build_highlight_config());
}

#[component]
pub fn SqlEditor(
    sql: String,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut scroll_left = use_signal(|| 0.0_f64);
    let editor_offset = format!(
        "transform: translate(-{}px, -{}px);",
        scroll_left(),
        scroll_top()
    );

    rsx! {
        div {
            class: "sql-editor",
            div {
                class: "sql-editor__viewport",
                pre {
                    class: "sql-editor__highlight",
                    style: "{editor_offset}",
                    aria_hidden: "true",
                    SqlHighlightContent { sql: sql.clone() }
                }
            }
            textarea {
                class: "sql-editor__input",
                value: "{sql}",
                rows: "16",
                cols: "80",
                spellcheck: "false",
                oninput: move |event| {
                    update_active_tab_sql(
                        tabs,
                        active_tab_id(),
                        event.value(),
                        "Ready".to_string(),
                    );
                },
                onscroll: move |event| {
                    scroll_top.set(event.data().scroll_top());
                    scroll_left.set(event.data().scroll_left());
                }
            }
        }
    }
}

#[component]
fn SqlHighlightContent(sql: String) -> Element {
    let highlighted_sql = highlight_sql(&sql);

    rsx! {
        if sql.is_empty() {
            span {
                class: "sql-editor__placeholder",
                "-- Write SQL here. Syntax highlighting is powered by tree-sitter."
            }
        } else {
            for segment in highlighted_sql {
                span {
                    class: format!("sql-editor__token {}", segment.class_name),
                    "{segment.text}"
                }
            }
        }
    }
}

fn build_highlight_config() -> Option<HighlightConfiguration> {
    let mut config = HighlightConfiguration::new(
        tree_sitter_sequel::LANGUAGE.into(),
        "sql",
        tree_sitter_sequel::HIGHLIGHTS_QUERY,
        "",
        "",
    )
    .ok()?;
    config.configure(&SQL_HIGHLIGHT_NAMES);
    Some(config)
}

fn highlight_sql(sql: &str) -> Vec<SqlHighlightSegment> {
    if sql.is_empty() {
        return Vec::new();
    }

    SQL_HIGHLIGHT_CONFIG.with(|config| {
        let config = config.borrow();
        let Some(config) = config.as_ref() else {
            return vec![plain_segment(sql)];
        };

        let mut highlighter = Highlighter::new();
        let events = match highlighter.highlight(config, sql.as_bytes(), None, |_| None) {
            Ok(events) => events,
            Err(_) => return vec![plain_segment(sql)],
        };

        let mut segments = Vec::new();
        let mut highlight_stack = Vec::<usize>::new();

        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(highlight)) => highlight_stack.push(highlight.0),
                Ok(HighlightEvent::HighlightEnd) => {
                    highlight_stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    let text = &sql[start..end];
                    push_segment(
                        &mut segments,
                        token_class(highlight_stack.last().copied()),
                        text,
                    );
                }
                Err(_) => return vec![plain_segment(sql)],
            }
        }

        segments
    })
}

fn push_segment(segments: &mut Vec<SqlHighlightSegment>, class_name: &'static str, text: &str) {
    if text.is_empty() {
        return;
    }

    if let Some(last) = segments.last_mut() {
        if last.class_name == class_name {
            last.text.push_str(text);
            return;
        }
    }

    segments.push(SqlHighlightSegment {
        class_name,
        text: text.to_string(),
    });
}

fn plain_segment(sql: &str) -> SqlHighlightSegment {
    SqlHighlightSegment {
        class_name: "sql-editor__token--plain",
        text: sql.to_string(),
    }
}

fn token_class(highlight_index: Option<usize>) -> &'static str {
    match highlight_index
        .and_then(|index| SQL_HIGHLIGHT_NAMES.get(index))
        .copied()
    {
        Some("keyword") | Some("conditional") | Some("storageclass") => {
            "sql-editor__token--keyword"
        }
        Some("keyword.operator") | Some("operator") => "sql-editor__token--operator",
        Some("string") => "sql-editor__token--string",
        Some("number") | Some("float") | Some("boolean") => "sql-editor__token--number",
        Some("comment") | Some("spell") => "sql-editor__token--comment",
        Some("function.call") => "sql-editor__token--function",
        Some("type") | Some("type.builtin") | Some("type.qualifier") => "sql-editor__token--type",
        Some("field") | Some("parameter") | Some("variable") => "sql-editor__token--identifier",
        Some("attribute") => "sql-editor__token--attribute",
        Some("punctuation.bracket") | Some("punctuation.delimiter") => {
            "sql-editor__token--punctuation"
        }
        _ => "sql-editor__token--plain",
    }
}
