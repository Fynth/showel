use crate::{
    app_state::{APP_STATE, open_connection_screen},
    screens::workspace::{
        actions::{
            load_tab_page, new_query_tab, open_structure_tab, refresh_tab_result,
            replace_active_tab_sql, run_query_for_tab, set_active_tab_status,
            tab_connection_or_error, update_active_tab_sql,
        },
        components::{ResultTable, SqlEditor},
    },
};
use dioxus::prelude::*;
use models::{QueryHistoryItem, QueryOutput, QueryTabState};
use rfd::AsyncFileDialog;

const EDITOR_MIN_HEIGHT: f64 = 160.0;
const EDITOR_MAX_HEIGHT: f64 = 720.0;

#[derive(Clone, Copy, PartialEq)]
struct EditorResizeState {
    start_y: f64,
    start_height: f64,
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Csv,
    Json,
    Xlsx,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Xlsx => "xlsx",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Xlsx => "XLSX",
        }
    }
}

#[component]
pub fn TabsManager(
    mut tabs: Signal<Vec<QueryTabState>>,
    mut active_tab_id: Signal<u64>,
    mut next_tab_id: Signal<u64>,
    history: Signal<Vec<QueryHistoryItem>>,
    next_history_id: Signal<u64>,
    show_sql_editor: Signal<bool>,
) -> Element {
    let mut editor_height = use_signal(|| 260.0);
    let mut editor_resize = use_signal(|| None::<EditorResizeState>);
    let active_tab = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id())
        .cloned();

    let session_labels = {
        let app_state = APP_STATE.read();
        app_state
            .sessions
            .iter()
            .map(|session| (session.id, session.name.clone()))
            .collect::<std::collections::HashMap<_, _>>()
    };

    rsx! {
        div {
            class: {
                let mut class_name = if show_sql_editor() {
                    "editor-shell".to_string()
                } else {
                    "editor-shell editor-shell--editor-hidden".to_string()
                };

                if editor_resize().is_some() {
                    class_name.push_str(" editor-shell--resizing");
                }

                class_name
            },
            style: if show_sql_editor() {
                format!("--editor-pane-height: {:.0}px;", editor_height())
            } else {
                String::new()
            },
            onmousemove: move |event| {
                let Some(resize) = editor_resize() else {
                    return;
                };

                if event.held_buttons().is_empty() {
                    editor_resize.set(None);
                    return;
                }

                let delta_y = event.client_coordinates().y - resize.start_y;
                let next_height =
                    (resize.start_height + delta_y).clamp(EDITOR_MIN_HEIGHT, EDITOR_MAX_HEIGHT);
                editor_height.set(next_height);
            },
            onmouseup: move |_| editor_resize.set(None),
            onmouseleave: move |_| editor_resize.set(None),
            if show_sql_editor() {
                div {
                    class: "tabbar",
                    for tab in tabs() {
                        div {
                            class: if tab.id == active_tab_id() {
                                "tabbar__tab tabbar__tab--active"
                            } else {
                                "tabbar__tab"
                            },
                            onclick: {
                                let tab_id = tab.id;
                                let session_id = tab.session_id;
                                move |_| {
                                    active_tab_id.set(tab_id);
                                    crate::app_state::activate_session(session_id);
                                }
                            },
                            div {
                                class: "tabbar__copy",
                                span { class: "tabbar__label", "{tab.title}" }
                                if let Some(session_name) = session_labels.get(&tab.session_id) {
                                    span { class: "tabbar__context", "{session_name}" }
                                }
                            }
                            button {
                                class: "tabbar__close",
                                onclick: {
                                    let tab_id = tab.id;
                                    move |event| {
                                        event.stop_propagation();
                                        if tabs.read().len() == 1 {
                                            return;
                                        }

                                        tabs.with_mut(|all_tabs| all_tabs.retain(|tab| tab.id != tab_id));
                                        if active_tab_id() == tab_id
                                            && let Some(first_tab) = tabs.read().first()
                                        {
                                            active_tab_id.set(first_tab.id);
                                            crate::app_state::activate_session(first_tab.session_id);
                                        }
                                    }
                                },
                                "x"
                            }
                        }
                    }
                    button {
                        class: "tabbar__add",
                        onclick: move |_| {
                            let Some(session_id) = APP_STATE.read().active_session_id else {
                                open_connection_screen();
                                return;
                            };

                            let new_id = next_tab_id();
                            next_tab_id += 1;
                            tabs.with_mut(|all_tabs| {
                                all_tabs.push(new_query_tab(
                                    new_id,
                                    session_id,
                                    format!("Query {new_id}"),
                                    String::new(),
                                ));
                            });
                            active_tab_id.set(new_id);
                        },
                        "+ Tab"
                    }
                }
            }

            if let Some(active_tab) = active_tab {
                if show_sql_editor() {
                    div {
                        class: "editor",
                        SqlEditor {
                            sql: active_tab.sql.clone(),
                            tabs,
                            active_tab_id,
                        }
                    }
                    div {
                        class: if editor_resize().is_some() {
                            "editor-shell__resize-handle editor-shell__resize-handle--active"
                        } else {
                            "editor-shell__resize-handle"
                        },
                        onmousedown: move |event| {
                            event.prevent_default();
                            editor_resize.set(Some(EditorResizeState {
                                start_y: event.client_coordinates().y,
                                start_height: editor_height(),
                            }));
                        }
                    }
                }
                div {
                    class: "editor__actions",
                    button {
                        class: "button button--primary",
                        onclick: move |_| {
                            let current_id = active_tab_id();
                            let current_tab = tabs
                                .read()
                                .iter()
                                .find(|tab| tab.id == current_id)
                                .cloned();

                            let Some(current_tab) = current_tab else {
                                return;
                            };

                            let sql = current_tab.sql.trim().to_string();
                            let tab_title = current_tab.title.clone();
                            let page_size = current_tab.page_size;
                            let connection_name = session_labels
                                .get(&current_tab.session_id)
                                .cloned()
                                .unwrap_or_else(|| "Detached session".to_string());

                            if sql.is_empty() {
                                tabs.with_mut(|all_tabs| {
                                    if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                                        tab.status = "Query is empty".to_string();
                                    }
                                });
                                return;
                            }

                            let Some(connection) =
                                tab_connection_or_error(tabs, current_id, current_tab.session_id)
                            else {
                                return;
                            };

                            run_query_for_tab(
                                tabs,
                                current_id,
                                connection,
                                sql,
                                0,
                                page_size,
                                Some((history, next_history_id, tab_title, connection_name)),
                            );
                        },
                        "Run SQL"
                    }
                    button {
                        class: "button button--ghost",
                        onclick: {
                            let current_id = active_tab_id();
                            move |_| {
                                update_active_tab_sql(
                                    tabs,
                                    current_id,
                                    String::new(),
                                    "Ready".to_string(),
                                );
                            }
                        },
                        "Clear"
                    }
                    button {
                        class: "button button--ghost",
                        onclick: {
                            let current_tab = active_tab.clone();
                            move |_| format_active_sql(tabs, current_tab.clone())
                        },
                        "Format SQL"
                    }
                    button {
                        class: "button button--ghost",
                        disabled: active_tab.preview_source.is_none(),
                        onclick: {
                            let current_tab = active_tab.clone();
                            move |_| open_structure_for_active_preview(
                                tabs,
                                active_tab_id,
                                next_tab_id,
                                current_tab.clone(),
                            )
                        },
                        "Open Structure"
                    }
                    button {
                        class: "button button--ghost",
                        disabled: !has_tabular_result(&active_tab),
                        onclick: {
                            let current_tab = active_tab.clone();
                            move |_| export_active_page(tabs, current_tab.clone(), ExportFormat::Csv)
                        },
                        "Export CSV"
                    }
                    button {
                        class: "button button--ghost",
                        disabled: !has_tabular_result(&active_tab),
                        onclick: {
                            let current_tab = active_tab.clone();
                            move |_| export_active_page(tabs, current_tab.clone(), ExportFormat::Json)
                        },
                        "Export JSON"
                    }
                    button {
                        class: "button button--ghost",
                        disabled: !has_tabular_result(&active_tab),
                        onclick: {
                            let current_tab = active_tab.clone();
                            move |_| export_active_page(tabs, current_tab.clone(), ExportFormat::Xlsx)
                        },
                        "Export XLSX"
                    }
                    button {
                        class: "button button--ghost",
                        disabled: active_tab.preview_source.is_none(),
                        onclick: {
                            let current_tab = active_tab.clone();
                            move |_| import_csv_into_active_table(tabs, current_tab.clone())
                        },
                        "Import CSV"
                    }
                }
                if let Some(QueryOutput::Table(page)) = active_tab.result.clone() {
                    div {
                        class: "editor__pagination",
                        p { class: "editor__pagination-meta",
                            "Rows {page.offset + 1}-{page.offset + page.rows.len() as u64} · page size {page.page_size}"
                        }
                        button {
                            class: "button button--ghost",
                            disabled: !page.has_previous || active_tab.last_run_sql.is_none(),
                            onclick: {
                                let current_tab = active_tab.clone();
                                move |_| {
                                    if current_tab.last_run_sql.is_none()
                                        && current_tab.preview_source.is_none()
                                    {
                                        return;
                                    };
                                    load_tab_page(
                                        tabs,
                                        current_tab.clone(),
                                        page.offset.saturating_sub(current_tab.page_size as u64),
                                    );
                                }
                            },
                            "Previous"
                        }
                        button {
                            class: "button button--ghost",
                            disabled: !page.has_next || active_tab.last_run_sql.is_none(),
                            onclick: {
                                let current_tab = active_tab.clone();
                                move |_| {
                                    if current_tab.last_run_sql.is_none()
                                        && current_tab.preview_source.is_none()
                                    {
                                        return;
                                    };
                                    load_tab_page(
                                        tabs,
                                        current_tab.clone(),
                                        page.offset + current_tab.page_size as u64,
                                    );
                                }
                            },
                            "Next"
                        }
                    }
                }
                div {
                    class: "workspace__results",
                    p { class: "workspace__status", "Status: {active_tab.status}" }
                    ResultTable {
                        result: active_tab.result.clone(),
                        tabs,
                        active_tab_id,
                    }
                }
            } else {
                div {
                    class: "workspace__empty",
                    p { class: "empty-state", "No active tab for the selected connection." }
                }
            }
        }
    }
}

fn export_active_page(
    tabs: Signal<Vec<QueryTabState>>,
    current_tab: QueryTabState,
    format: ExportFormat,
) {
    let Some(QueryOutput::Table(page)) = current_tab.result.clone() else {
        set_active_tab_status(
            tabs,
            current_tab.id,
            "Nothing to export in the current tab".to_string(),
        );
        return;
    };

    let file_name = default_export_file_name(&current_tab, format);
    set_active_tab_status(
        tabs,
        current_tab.id,
        format!("Select a destination for the {} export", format.label()),
    );

    spawn(async move {
        let Some(file) = AsyncFileDialog::new()
            .set_file_name(&file_name)
            .add_filter(format.label(), &[format.extension()])
            .save_file()
            .await
        else {
            set_active_tab_status(tabs, current_tab.id, "Export cancelled".to_string());
            return;
        };

        let path = file.path().to_path_buf();
        set_active_tab_status(
            tabs,
            current_tab.id,
            format!(
                "Exporting {} rows to {}...",
                page.rows.len(),
                format.label()
            ),
        );

        let export_result = match format {
            ExportFormat::Csv => services::export_query_page_csv(page, path.clone()).await,
            ExportFormat::Json => services::export_query_page_json(page, path.clone()).await,
            ExportFormat::Xlsx => services::export_query_page_xlsx(page, path.clone()).await,
        };

        match export_result {
            Ok(rows) => {
                let destination = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                set_active_tab_status(
                    tabs,
                    current_tab.id,
                    format!("Exported {rows} row(s) to {destination}"),
                );
            }
            Err(err) => set_active_tab_status(
                tabs,
                current_tab.id,
                format!("{} export error: {err}", format.label()),
            ),
        }
    });
}

fn import_csv_into_active_table(tabs: Signal<Vec<QueryTabState>>, current_tab: QueryTabState) {
    let Some(source) = current_tab.preview_source.clone() else {
        set_active_tab_status(
            tabs,
            current_tab.id,
            "Open a table preview before importing CSV".to_string(),
        );
        return;
    };

    let Some(connection) = tab_connection_or_error(tabs, current_tab.id, current_tab.session_id)
    else {
        return;
    };

    set_active_tab_status(
        tabs,
        current_tab.id,
        format!("Select a CSV file to import into {}", source.table_name),
    );

    spawn(async move {
        let Some(file) = AsyncFileDialog::new()
            .add_filter("CSV", &["csv"])
            .pick_file()
            .await
        else {
            set_active_tab_status(tabs, current_tab.id, "CSV import cancelled".to_string());
            return;
        };

        let path = file.path().to_path_buf();
        set_active_tab_status(
            tabs,
            current_tab.id,
            format!("Importing {}...", path.to_string_lossy()),
        );

        match services::import_csv_into_table(connection, source.clone(), path).await {
            Ok(rows) => {
                set_active_tab_status(
                    tabs,
                    current_tab.id,
                    format!("Imported {rows} row(s) into {}", source.table_name),
                );
                if let Some(updated_tab) = tabs
                    .read()
                    .iter()
                    .find(|tab| tab.id == current_tab.id)
                    .cloned()
                {
                    refresh_tab_result(tabs, updated_tab, Some(source));
                }
            }
            Err(err) => {
                set_active_tab_status(tabs, current_tab.id, format!("CSV import error: {err}"))
            }
        }
    });
}

fn default_export_file_name(tab: &QueryTabState, format: ExportFormat) -> String {
    let base = tab
        .preview_source
        .as_ref()
        .map(|source| source.table_name.clone())
        .unwrap_or_else(|| tab.title.clone());
    let sanitized = sanitize_file_name(&base);
    format!("{sanitized}.{}", format.extension())
}

fn sanitize_file_name(value: &str) -> String {
    let candidate = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if candidate.is_empty() {
        "query_result".to_string()
    } else {
        candidate
    }
}

fn has_tabular_result(tab: &QueryTabState) -> bool {
    matches!(tab.result.as_ref(), Some(QueryOutput::Table(_)))
}

fn format_active_sql(tabs: Signal<Vec<QueryTabState>>, current_tab: QueryTabState) {
    let sql = current_tab.sql.trim();
    if sql.is_empty() {
        set_active_tab_status(
            tabs,
            current_tab.id,
            "Nothing to format in the current tab".to_string(),
        );
        return;
    }

    let session_kind = APP_STATE
        .read()
        .session(current_tab.session_id)
        .map(|session| session.kind);
    let formatted = services::format_sql(session_kind, sql);
    replace_active_tab_sql(tabs, current_tab.id, formatted, "SQL formatted".to_string());
}

fn open_structure_for_active_preview(
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    current_tab: QueryTabState,
) {
    let Some(source) = current_tab.preview_source.clone() else {
        set_active_tab_status(
            tabs,
            current_tab.id,
            "Structure view is available for previewed tables and views".to_string(),
        );
        return;
    };

    let Some(connection) = tab_connection_or_error(tabs, current_tab.id, current_tab.session_id)
    else {
        return;
    };

    open_structure_tab(
        tabs,
        active_tab_id,
        next_tab_id,
        current_tab.session_id,
        connection,
        source,
    );
}
