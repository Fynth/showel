use crate::screens::workspace::actions::{
    refresh_tab_result, set_active_tab_status, tab_connection_or_error,
};
use dioxus::prelude::*;
use models::{QueryOutput, QueryTabState};

#[derive(Clone, PartialEq)]
struct EditingCell {
    row_index: usize,
    col_index: usize,
    value: String,
}

#[component]
pub fn ResultTable(
    result: Option<QueryOutput>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let mut editing_cell = use_signal(|| None::<EditingCell>);
    let current_editing = editing_cell();

    rsx! {
        match result {
            Some(QueryOutput::AffectedRows(rows)) => rsx! {
                div {
                    class: "results",
                    p { class: "results__summary", "Rows affected: {rows}" }
                }
            },
            Some(QueryOutput::Table(page)) => rsx! {
                if page.columns.is_empty() && page.rows.is_empty() {
                    p { class: "empty-state", "Query returned no rows." }
                } else {
                    div {
                        class: "results",
                        table {
                            class: "results__table",
                            thead {
                                tr {
                                    for column in page.columns {
                                        th { class: "results__head", "{column}" }
                                    }
                                }
                            }
                            tbody {
                                for (row_index, row) in page.rows.iter().enumerate() {
                                    tr { class: "results__row",
                                        for (col_index, cell) in row.iter().enumerate() {
                                            td {
                                                class: if page.editable.is_some() {
                                                    "results__cell results__cell--editable"
                                                } else {
                                                    "results__cell"
                                                },
                                                ondoubleclick: {
                                                    let cell_value = cell.clone();
                                                    let editable = page.editable.is_some();
                                                    move |_| {
                                                        if editable {
                                                            editing_cell.set(Some(EditingCell {
                                                                row_index,
                                                                col_index,
                                                                value: cell_value.clone(),
                                                            }));
                                                        }
                                                    }
                                                },
                                                if let Some(current_edit) = current_editing.clone() {
                                                    if current_edit.row_index == row_index && current_edit.col_index == col_index {
                                                        input {
                                                            class: "results__cell-input",
                                                            value: "{current_edit.value}",
                                                            oninput: move |event| {
                                                                let value = event.value();
                                                                editing_cell.with_mut(|editing| {
                                                                    if let Some(editing) = editing.as_mut() {
                                                                        editing.value = value;
                                                                    }
                                                                });
                                                            },
                                                            onkeydown: {
                                                                move |event| {
                                                                    if event.key() == Key::Enter {
                                                                        if let Some(editing) = editing_cell() {
                                                                            commit_cell_edit(
                                                                                editing_cell,
                                                                                tabs,
                                                                                active_tab_id,
                                                                                editing,
                                                                            );
                                                                        }
                                                                    } else if event.key() == Key::Escape {
                                                                        editing_cell.set(None);
                                                                    }
                                                                }
                                                            },
                                                            onblur: move |_| {
                                                                if let Some(editing) = editing_cell() {
                                                                    commit_cell_edit(
                                                                        editing_cell,
                                                                        tabs,
                                                                        active_tab_id,
                                                                        editing,
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        "{cell}"
                                                    }
                                                } else {
                                                    "{cell}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            None => rsx! {
                p { class: "empty-state", "Double-click a table in Explorer or run SQL to see rows here." }
            },
        }
    }
}

fn commit_cell_edit(
    mut editing_cell: Signal<Option<EditingCell>>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    editing: EditingCell,
) {
    let current_id = active_tab_id();
    let current_tab = tabs.read().iter().find(|tab| tab.id == current_id).cloned();
    let Some(current_tab) = current_tab else {
        editing_cell.set(None);
        return;
    };
    let Some(QueryOutput::Table(page)) = current_tab.result.clone() else {
        editing_cell.set(None);
        return;
    };
    let Some(editable) = page.editable.clone() else {
        editing_cell.set(None);
        return;
    };
    let Some(locator) = editable.row_locators.get(editing.row_index).cloned() else {
        editing_cell.set(None);
        return;
    };
    let Some(column_name) = page.columns.get(editing.col_index).cloned() else {
        editing_cell.set(None);
        return;
    };
    let Some(connection) = tab_connection_or_error(tabs, current_id, current_tab.session_id) else {
        editing_cell.set(None);
        return;
    };

    editing_cell.set(None);
    set_active_tab_status(tabs, current_id, format!("Saving {}...", column_name));

    spawn(async move {
        match services::update_table_cell(
            connection.clone(),
            editable.source.clone(),
            locator,
            column_name,
            editing.value,
        )
        .await
        {
            Ok(_) => refresh_tab_result(tabs, current_tab, Some(editable.source)),
            Err(err) => {
                set_active_tab_status(tabs, current_id, format!("Cell update error: {err:?}"));
            }
        }
    });
}
