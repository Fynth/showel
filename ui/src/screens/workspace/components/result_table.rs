use crate::screens::workspace::actions::{
    append_next_tab_page, apply_active_tab_filter, clear_active_tab_filter, load_tab_page,
    refresh_tab_result, rows_toolbar_summary, set_active_tab_status, tab_connection_or_error,
    toggle_active_tab_sort,
};
use crate::screens::workspace::components::{ActionIcon, IconButton};
use dioxus::prelude::*;
use models::{
    EditableTableContext, PendingCellChange, PendingDeleteRow, PendingInsertRow,
    PendingTableChanges, QueryFilter, QueryFilterMode, QueryFilterOperator, QueryFilterRule,
    QueryOutput, QuerySort, QueryTabState,
};
use serde_json::{Map, Value};

#[derive(Clone, PartialEq)]
struct EditingCell {
    row_ref: EditableRowRef,
    col_index: usize,
    value: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowDetailsView {
    Fields,
    Json,
}

#[derive(Clone, PartialEq, Eq)]
enum EditableRowRef {
    Existing(String),
    PendingInsert(u64),
}

#[derive(Clone, PartialEq)]
struct DisplayRow {
    row_ref: EditableRowRef,
    values: Vec<String>,
}

#[component]
pub fn ResultTable(
    result: Option<QueryOutput>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let mut editing_cell = use_signal(|| None::<EditingCell>);
    let mut filter_draft = use_signal(|| QueryFilter {
        mode: QueryFilterMode::And,
        rules: Vec::new(),
    });
    let mut filter_sync_key = use_signal(String::new);
    let mut filter_panel_open = use_signal(|| false);
    let mut selected_row_index = use_signal(|| None::<usize>);
    let mut selected_row_sync_key = use_signal(String::new);
    let mut show_row_details = use_signal(|| true);
    let mut row_details_view = use_signal(|| RowDetailsView::Fields);
    let mut editing_row_values = use_signal(Vec::<(usize, String)>::new);
    let mut editing_row_ref = use_signal(|| None::<EditableRowRef>);

    let current_editing = editing_cell();
    let active_tab = tabs
        .read()
        .iter()
        .find(|tab| tab.id == active_tab_id())
        .cloned();
    let active_filter = active_tab.as_ref().and_then(|tab| tab.filter.clone());
    let has_active_filter = active_filter.is_some();
    let active_sort = active_tab.as_ref().and_then(|tab| tab.sort.clone());
    let active_error = active_tab
        .as_ref()
        .and_then(|tab| result_error_message(&tab.status));
    let pending_changes = active_tab
        .as_ref()
        .map(|tab| tab.pending_table_changes.clone())
        .unwrap_or_default();
    let has_pending_changes = !pending_changes.is_empty();
    let is_loading_more = active_tab.as_ref().is_some_and(|tab| tab.is_loading_more);
    let sort_enabled = active_tab.as_ref().is_some_and(can_sort_tab);
    let filter_enabled = active_tab.as_ref().is_some_and(can_filter_tab);
    let current_columns = result_columns(result.as_ref());
    let next_filter_draft = filter_draft_from_state(active_filter.as_ref(), &current_columns);
    let next_filter_sync_key = filter_sync_key_for_tab(active_tab.as_ref(), &current_columns);
    let next_row_sync_key = row_sync_key_for_tab(active_tab.as_ref(), result.as_ref());

    use_effect(move || {
        if filter_sync_key() != next_filter_sync_key {
            filter_sync_key.set(next_filter_sync_key.clone());
            filter_draft.set(next_filter_draft.clone());
            filter_panel_open.set(has_active_filter);
        }

        if filter_panel_should_auto_open(has_active_filter, &filter_draft()) && !filter_panel_open()
        {
            filter_panel_open.set(true);
        }
    });

    use_effect(move || {
        if selected_row_sync_key() != next_row_sync_key {
            selected_row_sync_key.set(next_row_sync_key.clone());
            selected_row_index.set(None);
            row_details_view.set(RowDetailsView::Fields);
        }
    });

    rsx! {
        match result {
            Some(QueryOutput::AffectedRows(rows)) => rsx! {
                div {
                    class: "results",
                    p { class: "results__summary", "Rows affected: {rows}" }
                }
            },
            Some(QueryOutput::Table(page)) => {
                let display_rows = materialize_display_rows(&page, &pending_changes);
                let draft_rows = pending_changes.inserted_rows.len();
                let selected_row = selected_row_index().and_then(|index| {
                    display_rows
                        .get(index)
                        .cloned()
                        .map(|row| (index, row))
                });
                let details_visible = show_row_details() && selected_row.is_some();
                let has_selected_row = selected_row.is_some();
                let selected_row_label = selected_row
                    .as_ref()
                    .map(|(row_index, row)| display_row_label(page.offset, draft_rows, *row_index, row));
                let details_json = selected_row
                    .as_ref()
                    .map(|(_, row)| format_row_json(&page.columns, &row.values))
                    .unwrap_or_default();
                let status_text = active_tab
                    .as_ref()
                    .map(|tab| tab.status.clone())
                    .unwrap_or_else(|| "Ready".to_string());
                let can_paginate = active_tab
                    .as_ref()
                    .is_some_and(|tab| tab.last_run_sql.is_some() || tab.preview_source.is_some());
                let has_previous_page = page.has_previous && can_paginate && !is_loading_more && !has_pending_changes;
                let has_next_page = page.has_next && can_paginate && !is_loading_more && !has_pending_changes;

                rsx! {
                    if page.columns.is_empty() && display_rows.is_empty() {
                        p { class: "empty-state", "Query returned no rows." }
                    } else {
                        div {
                            class: "results",
                            div {
                                class: if details_visible {
                                    "results__layout results__layout--with-details"
                                } else {
                                    "results__layout"
                                },
                                div {
                                    class: "results__main",
                                    div {
                                        class: "results__toolbar",
                                        div {
                                            class: "results__toolbar-copy",
                                            span {
                                                class: "results__toolbar-chip",
                                                "{rows_toolbar_summary(page.offset, page.rows.len(), page.page_size)}"
                                            }
                                            span {
                                                class: "results__toolbar-chip",
                                                "Status: {status_text}"
                                            }
                                            p {
                                                class: "results__toolbar-meta",
                                                if let Some(row_label) = selected_row_label.as_ref() {
                                                    "{row_label} selected"
                                                } else if has_pending_changes {
                                                    "{pending_changes_summary(&pending_changes)}"
                                                } else {
                                                    "Select a row for details."
                                                }
                                            }
                                        }
                                        div {
                                        class: "results__toolbar-actions",
                                        if filter_enabled {
                                            IconButton {
                                                icon: ActionIcon::Filter,
                                                label: "Filters".to_string(),
                                                active: filter_panel_open(),
                                                small: true,
                                                onclick: move |_| filter_panel_open.toggle(),
                                            }
                                        }
                                        IconButton {
                                            icon: ActionIcon::Previous,
                                            label: "Previous page".to_string(),
                                            small: true,
                                            disabled: !has_previous_page,
                                            onclick: {
                                                let current_tab = active_tab.clone();
                                                move |_| {
                                                    let Some(current_tab) = current_tab.clone() else {
                                                        return;
                                                    };
                                                    load_tab_page(
                                                        tabs,
                                                        current_tab.clone(),
                                                        page.offset.saturating_sub(current_tab.page_size as u64),
                                                    );
                                                }
                                            },
                                        }
                                        IconButton {
                                            icon: ActionIcon::Next,
                                            label: "Next page".to_string(),
                                            small: true,
                                            disabled: !has_next_page,
                                            onclick: {
                                                let current_tab = active_tab.clone();
                                                move |_| {
                                                    let Some(current_tab) = current_tab.clone() else {
                                                        return;
                                                    };
                                                    append_next_tab_page(tabs, current_tab);
                                                }
                                            },
                                        }
                                        if page.editable.is_some() {
                                            IconButton {
                                                icon: ActionIcon::InsertRow,
                                                label: "Insert draft row".to_string(),
                                                small: true,
                                                onclick: move |_| insert_empty_row(tabs, active_tab_id),
                                            }
                                            IconButton {
                                                icon: ActionIcon::Apply,
                                                label: "Apply pending changes".to_string(),
                                                small: true,
                                                disabled: !has_pending_changes,
                                                onclick: move |_| apply_pending_changes(tabs, active_tab_id),
                                            }
                                            IconButton {
                                                icon: ActionIcon::Undo,
                                                label: "Discard pending changes".to_string(),
                                                small: true,
                                                disabled: !has_pending_changes,
                                                onclick: move |_| discard_pending_changes(tabs, active_tab_id),
                                            }
                                            IconButton {
                                                icon: ActionIcon::Delete,
                                                label: "Delete selected row".to_string(),
                                                small: true,
                                                disabled: !has_selected_row,
                                                onclick: {
                                                    let selected_row_index = selected_row_index();
                                                    move |_| {
                                                        if let Some(row_index) = selected_row_index {
                                                            delete_selected_row(tabs, active_tab_id, row_index);
                                                        }
                                                    }
                                                },
                                            }
                                        }
                                        IconButton {
                                            icon: ActionIcon::Details,
                                            label: if details_visible {
                                                "Hide row details".to_string()
                                            } else {
                                                "Show row details".to_string()
                                            },
                                            active: details_visible,
                                            small: true,
                                            disabled: !has_selected_row,
                                            onclick: move |_| show_row_details.toggle(),
                                        }
                                    }
                                    }

                                    if filter_enabled && filter_panel_open() {
                                        div {
                                            class: "results__filters",
                                            div {
                                                class: "results__filters-topbar",
                                                select {
                                                    class: "input results__filter-mode",
                                                    value: filter_mode_value(filter_draft().mode),
                                                    oninput: move |event| update_filter_mode(filter_draft, event.value()),
                                                    option { value: "and", "Match all (AND)" }
                                                    option { value: "or", "Match any (OR)" }
                                                }
                                                IconButton {
                                                    icon: ActionIcon::AddRule,
                                                    label: "Add filter rule".to_string(),
                                                    small: true,
                                                    onclick: {
                                                        let columns = page.columns.clone();
                                                        move |_| add_filter_rule(filter_draft, &columns)
                                                    },
                                                }
                                                IconButton {
                                                    icon: ActionIcon::FilterApply,
                                                    label: "Apply filters".to_string(),
                                                    small: true,
                                                    onclick: move |_| {
                                                        apply_active_tab_filter(tabs, active_tab_id(), filter_draft());
                                                    },
                                                    disabled: !has_meaningful_rules(&filter_draft()),
                                                }
                                                IconButton {
                                                    icon: ActionIcon::FilterClear,
                                                    label: "Clear filters".to_string(),
                                                    small: true,
                                                    onclick: {
                                                        let columns = page.columns.clone();
                                                        move |_| {
                                                            filter_draft.set(blank_filter(&columns));
                                                            clear_active_tab_filter(tabs, active_tab_id());
                                                            filter_panel_open.set(false);
                                                        }
                                                    },
                                                    disabled: !has_active_filter && !has_meaningful_rules(&filter_draft()),
                                                }
                                            }

                                            div {
                                                class: "results__filters-body",
                                                for (rule_index, rule) in filter_draft().rules.iter().cloned().enumerate() {
                                                    div {
                                                        class: "results__filter-row",
                                                        select {
                                                            class: "input results__filter-select",
                                                            value: "{rule.column_name}",
                                                            oninput: move |event| {
                                                                update_filter_rule_column(
                                                                    filter_draft,
                                                                    rule_index,
                                                                    event.value(),
                                                                );
                                                            },
                                                            for column in page.columns.iter().cloned() {
                                                                option { value: column.clone(), "{column}" }
                                                            }
                                                        }
                                                        select {
                                                            class: "input results__filter-operator",
                                                            value: filter_operator_value(rule.operator),
                                                            oninput: move |event| {
                                                                update_filter_rule_operator(
                                                                    filter_draft,
                                                                    rule_index,
                                                                    event.value(),
                                                                );
                                                            },
                                                            for operator in supported_filter_operators() {
                                                                option {
                                                                    value: filter_operator_value(operator),
                                                                    "{filter_operator_label(operator)}"
                                                                }
                                                            }
                                                        }
                                                        if rule.operator.is_nullary() {
                                                            div {
                                                                class: "results__filter-null",
                                                                "No value required"
                                                            }
                                                        } else {
                                                            input {
                                                                class: "input results__filter-input",
                                                                value: "{rule.value}",
                                                                placeholder: "Enter filter value",
                                                                oninput: move |event| {
                                                                    update_filter_rule_value(
                                                                        filter_draft,
                                                                        rule_index,
                                                                        event.value(),
                                                                    );
                                                                },
                                                            }
                                                        }
                                                        IconButton {
                                                            icon: ActionIcon::Clear,
                                                            label: "Remove filter rule".to_string(),
                                                            small: true,
                                                            onclick: {
                                                                let columns = page.columns.clone();
                                                                move |_| remove_filter_rule(
                                                                    filter_draft,
                                                                    rule_index,
                                                                    &columns,
                                                                )
                                                            },
                                                            disabled: filter_draft().rules.len() <= 1,
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    div {
                                        class: "results__table-wrap",
                                        onscroll: move |event| {
                                            let scroll_state = event.data();
                                            let remaining_scroll = scroll_state.scroll_height() as f64
                                                - (scroll_state.scroll_top()
                                                    + scroll_state.client_height() as f64);

                                            if remaining_scroll > 96.0 {
                                                return;
                                            }

                                            let current_tab = tabs
                                                .read()
                                                .iter()
                                                .find(|tab| tab.id == active_tab_id())
                                                .cloned();

                                            let Some(current_tab) = current_tab else {
                                                return;
                                            };

                                            append_next_tab_page(tabs, current_tab);
                                        },
                                        table {
                                            class: "results__table",
                                            thead {
                                                tr {
                                                    for column in page.columns.iter().cloned() {
                                                        th {
                                                            class: "results__head",
                                                            if sort_enabled {
                                                                button {
                                                                    class: sort_button_class(active_sort.as_ref(), &column),
                                                                    disabled: has_pending_changes,
                                                                    onclick: {
                                                                        let column_name = column.clone();
                                                                        move |_| toggle_active_tab_sort(
                                                                            tabs,
                                                                            active_tab_id(),
                                                                            column_name.clone(),
                                                                        )
                                                                    },
                                                                    span { class: "results__head-label", "{column}" }
                                                                    span {
                                                                        class: "results__sort-indicator",
                                                                        "{sort_indicator(active_sort.as_ref(), &column)}"
                                                                    }
                                                                }
                                                            } else {
                                                                span { class: "results__head-label", "{column}" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            tbody {
                                                for (row_index, row) in display_rows.iter().cloned().enumerate() {
                                                    tr {
                                                        class: row_class(selected_row_index() == Some(row_index), &row),
                                                        key: "{display_row_key(&row)}",
                                                        onclick: {
                                                            let rows = display_rows.clone();
                                                            move |_| {
                                                                selected_row_index.set(Some(row_index));
                                                                show_row_details.set(true);
                                                                if let Some(row) = rows.get(row_index).cloned() {
                                                                    let values: Vec<(usize, String)> = row.values.iter()
                                                                        .enumerate()
                                                                        .map(|(i, v): (usize, &String)| (i, v.clone()))
                                                                        .collect();
                                                                    editing_row_values.set(values);
                                                                    editing_row_ref.set(Some(row.row_ref.clone()));
                                                                }
                                                            }
                                                        },
                                                        for (col_index, cell) in row.values.iter().cloned().enumerate() {
                                                            td {
                                                                class: cell_class(
                                                                    page.editable.is_some(),
                                                                    &row,
                                                                    page.columns.get(col_index),
                                                                    &pending_changes,
                                                                ),
                                                                ondoubleclick: {
                                                                    let cell_value = cell.clone();
                                                                    let editable = page.editable.is_some();
                                                                    let row_ref = row.row_ref.clone();
                                                                    move |_| {
                                                                        if editable {
                                                                            editing_cell.set(Some(EditingCell {
                                                                                row_ref: row_ref.clone(),
                                                                                col_index,
                                                                                value: cell_value.clone(),
                                                                            }));
                                                                        }
                                                                    }
                                                                },
                                                                if let Some(current_edit) = current_editing.clone() {
                                                                    if current_edit.row_ref == row.row_ref && current_edit.col_index == col_index {
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
                                                                            onkeydown: move |event| {
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
                                                                        div {
                                                                            class: "results__cell-content",
                                                                            title: "{cell}",
                                                                            "{cell}"
                                                                        }
                                                                    }
                                                                } else {
                                                                    div {
                                                                        class: "results__cell-content",
                                                                        title: "{cell}",
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
                                    }

                                    if is_loading_more {
                                        div {
                                            class: "results__load-more",
                                            "Loading more rows..."
                                        }
                                    }

                                    if details_visible {
                                    aside {
                                        class: "results__details",
                                        div {
                                            class: "results__details-header",
                                            div {
                                                class: "results__details-copy",
                                                h3 {
                                                    class: "results__details-title",
                                                    if let Some(row_label) = selected_row_label.as_ref() {
                                                        "{row_label}"
                                                    } else {
                                                        "Row Details"
                                                    }
                                                }
                                                p {
                                                    class: "results__details-hint",
                                                    "Full values for the selected row."
                                                }
                                            }
                                            IconButton {
                                                icon: ActionIcon::Close,
                                                label: "Close row details".to_string(),
                                                small: true,
                                                onclick: move |_| show_row_details.set(false),
                                            }
                                        }
                                        div {
                                            class: "results__details-actions",
                                            button {
                                                class: if row_details_view() == RowDetailsView::Fields {
                                                    "button button--ghost button--small button--active"
                                                } else {
                                                    "button button--ghost button--small"
                                                },
                                                onclick: move |_| row_details_view.set(RowDetailsView::Fields),
                                                "Fields"
                                            }
                                            button {
                                                class: if row_details_view() == RowDetailsView::Json {
                                                    "button button--ghost button--small button--active"
                                                } else {
                                                    "button button--ghost button--small"
                                                },
                                                onclick: move |_| row_details_view.set(RowDetailsView::Json),
                                                "JSON"
                                            }
                                            button {
                                                class: "button button--primary button--small",
                                                onclick: move |_| {
                                                    let editing_values = editing_row_values();
                                                    let editing_ref = editing_row_ref();
                                                    if let Some(row_ref) = editing_ref.clone() {
                                                        for (col_index, value) in editing_values.iter().cloned() {
                                                            let cell_edit = EditingCell {
                                                                row_ref: row_ref.clone(),
                                                                col_index,
                                                                value: value.clone(),
                                                            };
                                                            commit_cell_edit(
                                                                editing_cell,
                                                                tabs,
                                                                active_tab_id,
                                                                cell_edit,
                                                            );
                                                        }
                                                    }
                                                },
                                                "Save"
                                            }
                                        }
                                        if let Some((_, _row)) = selected_row.as_ref() {
                                            if row_details_view() == RowDetailsView::Fields {
                                                div {
                                                    class: "results__details-list",
                                                    for (col_index, value) in editing_row_values().iter().cloned() {
                                                        div {
                                                            class: "results__details-field",
                                                            p { class: "results__details-label", "{page.columns.get(col_index).unwrap_or(&\"?\".to_string())}" }
                                                            input {
                                                                class: "input results__details-input",
                                                                value: "{value}",
                                                                oninput: move |event| {
                                                                    editing_row_values.with_mut(|values| {
                                                                        if let Some(v) = values.iter_mut().find(|(i, _)| *i == col_index) {
                                                                            v.1 = event.value();
                                                                        }
                                                                    });
                                                                },
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                pre {
                                                    class: "results__details-json",
                                                    "{details_json}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None => rsx! {
                if let Some(error) = active_error {
                    div {
                        class: "results results--error",
                        div {
                            class: "results__error",
                            p { class: "results__error-title", "Query failed" }
                            pre { class: "results__error-body", "{error}" }
                        }
                    }
                } else {
                    p { class: "empty-state", "Double-click a table in Explorer or run SQL to see rows here." }
                }
            },
        }
    }
}

fn result_error_message(status: &str) -> Option<String> {
    [
        "Error: ",
        "Preview error: ",
        "Structure error: ",
        "Load more error: ",
    ]
    .iter()
    .find_map(|prefix| status.strip_prefix(prefix))
    .map(str::trim)
    .filter(|message| !message.is_empty())
    .map(ToOwned::to_owned)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        filter_panel_should_auto_open, filter_panel_should_collapse_after_clear,
        result_error_message,
    };
    use crate::screens::workspace::actions::rows_toolbar_summary;
    use models::{QueryFilter, QueryFilterMode, QueryFilterOperator, QueryFilterRule};

    #[test]
    fn extracts_query_error_from_status() {
        assert_eq!(
            result_error_message("Error: SQLite error: near \"from\": syntax error"),
            Some("SQLite error: near \"from\": syntax error".to_string())
        );
    }

    #[test]
    fn ignores_non_error_status() {
        assert_eq!(result_error_message("Loaded rows 1-10"), None);
    }

    #[test]
    fn summarizes_empty_page_without_invalid_range() {
        assert_eq!(rows_toolbar_summary(0, 0, 100), "0 rows · page size 100");
    }

    #[test]
    fn keeps_filters_collapsed_without_active_filter_or_meaningful_draft() {
        let filter = QueryFilter {
            mode: QueryFilterMode::And,
            rules: vec![QueryFilterRule {
                column_name: "name".to_string(),
                operator: QueryFilterOperator::Contains,
                value: String::new(),
            }],
        };

        assert!(!filter_panel_should_auto_open(false, &filter));
        assert!(filter_panel_should_collapse_after_clear(false, &filter));
    }

    #[test]
    fn opens_filters_for_active_filter_or_meaningful_draft() {
        let meaningful_filter = QueryFilter {
            mode: QueryFilterMode::And,
            rules: vec![QueryFilterRule {
                column_name: "name".to_string(),
                operator: QueryFilterOperator::Contains,
                value: "Ada".to_string(),
            }],
        };

        assert!(filter_panel_should_auto_open(true, &meaningful_filter));
        assert!(filter_panel_should_auto_open(false, &meaningful_filter));
    }

    #[test]
    fn compact_layout_supports_25_rows_at_default_window() {
        // Budget calculation for default 920px window height
        const WINDOW_HEIGHT: i32 = 920;
        const APP_TOOLBAR: i32 = 44;
        const STATUSBAR: i32 = 26;
        const WORKSPACE_PADDING: i32 = 12;
        const WORKSPACE_HEADER: i32 = 32;
        const TABBAR: i32 = 34;
        const RESULTS_TOOLBAR: i32 = 30;
        const ROW_HEIGHT_PX: i32 = 22;

        let chrome_height = APP_TOOLBAR
            + STATUSBAR
            + WORKSPACE_PADDING
            + WORKSPACE_HEADER
            + TABBAR
            + RESULTS_TOOLBAR;
        let available_height = WINDOW_HEIGHT - chrome_height;
        let visible_rows = available_height / ROW_HEIGHT_PX;

        // At compact layout, we should see at least 25 rows
        assert!(
            visible_rows >= 25,
            "Expected >= 25 visible rows (got {visible_rows}) with {available_height}px available"
        );
    }
}

fn can_sort_tab(tab: &QueryTabState) -> bool {
    tab.preview_source.is_some() || tab.last_run_sql.as_deref().is_some_and(is_sortable_sql)
}

fn can_filter_tab(tab: &QueryTabState) -> bool {
    can_sort_tab(tab)
}

fn is_sortable_sql(sql: &str) -> bool {
    matches!(
        sql.split_whitespace().next(),
        Some("select" | "SELECT" | "with" | "WITH")
    )
}

fn sort_button_class(active_sort: Option<&QuerySort>, column: &str) -> &'static str {
    match active_sort {
        Some(sort) if sort.column_name == column => {
            "results__sort-button results__sort-button--active"
        }
        _ => "results__sort-button",
    }
}

fn sort_indicator(active_sort: Option<&QuerySort>, column: &str) -> &'static str {
    match active_sort {
        Some(sort) if sort.column_name == column && sort.descending => "↓",
        Some(sort) if sort.column_name == column => "↑",
        _ => "↕",
    }
}

fn result_columns(result: Option<&QueryOutput>) -> Vec<String> {
    match result {
        Some(QueryOutput::Table(page)) => page.columns.clone(),
        _ => Vec::new(),
    }
}

fn materialize_display_rows(
    page: &models::QueryPage,
    pending_changes: &PendingTableChanges,
) -> Vec<DisplayRow> {
    let mut rows = pending_changes
        .inserted_rows
        .iter()
        .map(|row| DisplayRow {
            row_ref: EditableRowRef::PendingInsert(row.id),
            values: row
                .values
                .iter()
                .map(|value| value.clone().unwrap_or_default())
                .collect(),
        })
        .collect::<Vec<_>>();

    if let Some(editable) = page.editable.as_ref() {
        rows.extend(page.rows.iter().enumerate().filter_map(|(row_index, row)| {
            let locator = editable
                .row_locators
                .get(row_index)
                .cloned()
                .unwrap_or_default();
            if pending_changes
                .deleted_rows
                .iter()
                .any(|d| d.locator == locator)
            {
                return None;
            }
            Some(DisplayRow {
                row_ref: EditableRowRef::Existing(locator),
                values: page
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(col_index, column_name)| {
                        existing_cell_value(
                            pending_changes,
                            editable,
                            row_index,
                            col_index,
                            column_name,
                            row,
                        )
                    })
                    .collect(),
            })
        }));
    } else {
        rows.extend(
            page.rows
                .iter()
                .enumerate()
                .map(|(row_index, row)| DisplayRow {
                    row_ref: EditableRowRef::Existing(format!("result-{row_index}")),
                    values: row.clone(),
                }),
        );
    }

    rows
}

fn existing_cell_value(
    pending_changes: &PendingTableChanges,
    editable: &EditableTableContext,
    row_index: usize,
    col_index: usize,
    column_name: &str,
    row: &[String],
) -> String {
    let base_value = row.get(col_index).cloned().unwrap_or_default();
    let Some(locator) = editable.row_locators.get(row_index) else {
        return base_value;
    };

    pending_changes
        .updated_cells
        .iter()
        .find(|change| change.locator == *locator && change.column_name == column_name)
        .map(|change| change.value.clone())
        .unwrap_or(base_value)
}

fn display_row_label(offset: u64, draft_rows: usize, row_index: usize, row: &DisplayRow) -> String {
    match row.row_ref {
        EditableRowRef::PendingInsert(insert_id) => format!("Draft Row {insert_id}"),
        EditableRowRef::Existing(_) => {
            let persisted_index = row_index.saturating_sub(draft_rows);
            format!("Row {}", offset + persisted_index as u64 + 1)
        }
    }
}

fn display_row_key(row: &DisplayRow) -> String {
    match &row.row_ref {
        EditableRowRef::Existing(locator) => format!("row-{locator}"),
        EditableRowRef::PendingInsert(insert_id) => format!("draft-{insert_id}"),
    }
}

fn row_class(is_selected: bool, row: &DisplayRow) -> &'static str {
    match (&row.row_ref, is_selected) {
        (EditableRowRef::PendingInsert(_), true) => {
            "results__row results__row--draft results__row--selected"
        }
        (EditableRowRef::PendingInsert(_), false) => "results__row results__row--draft",
        (_, true) => "results__row results__row--selected",
        (_, false) => "results__row",
    }
}

fn cell_class(
    editable: bool,
    row: &DisplayRow,
    column_name: Option<&String>,
    pending_changes: &PendingTableChanges,
) -> &'static str {
    let mut is_pending = matches!(row.row_ref, EditableRowRef::PendingInsert(_));
    if let (EditableRowRef::Existing(locator), Some(column_name)) = (&row.row_ref, column_name) {
        is_pending = pending_changes
            .updated_cells
            .iter()
            .any(|change| change.locator == *locator && change.column_name == *column_name);
    }

    match (editable, is_pending) {
        (true, true) => "results__cell results__cell--editable results__cell--pending",
        (true, false) => "results__cell results__cell--editable",
        (false, true) => "results__cell results__cell--pending",
        (false, false) => "results__cell",
    }
}

fn pending_changes_summary(pending_changes: &PendingTableChanges) -> String {
    let inserts = pending_changes.inserted_rows.len();
    let updates = pending_changes.updated_cells.len();
    let deletes = pending_changes.deleted_rows.len();
    let mut parts = Vec::new();
    if inserts > 0 {
        parts.push(if inserts == 1 {
            "1 insert".to_string()
        } else {
            format!("{inserts} inserts")
        });
    }
    if updates > 0 {
        parts.push(if updates == 1 {
            "1 update".to_string()
        } else {
            format!("{updates} updates")
        });
    }
    if deletes > 0 {
        parts.push(if deletes == 1 {
            "1 delete".to_string()
        } else {
            format!("{deletes} deletes")
        });
    }
    if parts.is_empty() {
        "No pending changes".to_string()
    } else {
        format!("{} pending", parts.join(", "))
    }
}

fn filter_draft_from_state(active_filter: Option<&QueryFilter>, columns: &[String]) -> QueryFilter {
    let mut filter = active_filter
        .cloned()
        .unwrap_or_else(|| blank_filter(columns));

    if filter.rules.is_empty() {
        filter
            .rules
            .push(blank_rule(default_filter_column(columns)));
    }

    for rule in &mut filter.rules {
        if rule.column_name.trim().is_empty()
            || !columns.iter().any(|column| column == &rule.column_name)
        {
            rule.column_name = default_filter_column(columns);
        }
    }

    filter
}

fn filter_sync_key_for_tab(active_tab: Option<&QueryTabState>, columns: &[String]) -> String {
    match active_tab {
        Some(tab) => format!("{}|{:?}|{:?}", tab.id, tab.filter.as_ref(), columns),
        None => "no-tab".to_string(),
    }
}

fn row_sync_key_for_tab(
    active_tab: Option<&QueryTabState>,
    result: Option<&QueryOutput>,
) -> String {
    match (active_tab, result) {
        (Some(tab), Some(QueryOutput::Table(page))) => format!(
            "{}|{:?}|{:?}|{}|{}|{}|{}",
            tab.id,
            tab.preview_source
                .as_ref()
                .map(|source| &source.qualified_name),
            tab.last_run_sql.as_ref(),
            page.offset,
            page.rows.len(),
            page.columns.len(),
            tab.pending_table_changes.inserted_rows.len()
        ),
        (Some(tab), _) => format!("{}|no-table", tab.id),
        _ => "no-tab".to_string(),
    }
}

fn blank_filter(columns: &[String]) -> QueryFilter {
    QueryFilter {
        mode: QueryFilterMode::And,
        rules: vec![blank_rule(default_filter_column(columns))],
    }
}

fn blank_rule(default_column: String) -> QueryFilterRule {
    QueryFilterRule {
        column_name: default_column,
        operator: QueryFilterOperator::Contains,
        value: String::new(),
    }
}

fn default_filter_column(columns: &[String]) -> String {
    columns.first().cloned().unwrap_or_default()
}

fn has_meaningful_rules(filter: &QueryFilter) -> bool {
    filter.rules.iter().any(|rule| {
        !rule.column_name.trim().is_empty()
            && (!rule.value.trim().is_empty() || rule.operator.is_nullary())
    })
}

fn filter_panel_should_auto_open(active_filter_present: bool, filter_draft: &QueryFilter) -> bool {
    active_filter_present || has_meaningful_rules(filter_draft)
}

#[cfg(test)]
fn filter_panel_should_collapse_after_clear(
    active_filter_present: bool,
    filter_draft: &QueryFilter,
) -> bool {
    !active_filter_present && !has_meaningful_rules(filter_draft)
}

fn update_filter_mode(mut filter_draft: Signal<QueryFilter>, value: String) {
    filter_draft.with_mut(|filter| {
        filter.mode = if value.eq_ignore_ascii_case("or") {
            QueryFilterMode::Or
        } else {
            QueryFilterMode::And
        };
    });
}

fn add_filter_rule(mut filter_draft: Signal<QueryFilter>, columns: &[String]) {
    filter_draft.with_mut(|filter| {
        filter
            .rules
            .push(blank_rule(default_filter_column(columns)));
    });
}

fn remove_filter_rule(mut filter_draft: Signal<QueryFilter>, index: usize, columns: &[String]) {
    filter_draft.with_mut(|filter| {
        if index < filter.rules.len() {
            filter.rules.remove(index);
        }
        if filter.rules.is_empty() {
            filter
                .rules
                .push(blank_rule(default_filter_column(columns)));
        }
    });
}

fn update_filter_rule_column(
    mut filter_draft: Signal<QueryFilter>,
    index: usize,
    column_name: String,
) {
    filter_draft.with_mut(|filter| {
        if let Some(rule) = filter.rules.get_mut(index) {
            rule.column_name = column_name;
        }
    });
}

fn update_filter_rule_operator(
    mut filter_draft: Signal<QueryFilter>,
    index: usize,
    operator_value: String,
) {
    filter_draft.with_mut(|filter| {
        if let Some(rule) = filter.rules.get_mut(index) {
            rule.operator = parse_filter_operator(&operator_value);
            if rule.operator.is_nullary() {
                rule.value.clear();
            }
        }
    });
}

fn update_filter_rule_value(mut filter_draft: Signal<QueryFilter>, index: usize, value: String) {
    filter_draft.with_mut(|filter| {
        if let Some(rule) = filter.rules.get_mut(index) {
            rule.value = value;
        }
    });
}

fn supported_filter_operators() -> [QueryFilterOperator; 8] {
    [
        QueryFilterOperator::Contains,
        QueryFilterOperator::NotContains,
        QueryFilterOperator::Equals,
        QueryFilterOperator::NotEquals,
        QueryFilterOperator::StartsWith,
        QueryFilterOperator::EndsWith,
        QueryFilterOperator::IsNull,
        QueryFilterOperator::IsNotNull,
    ]
}

fn filter_mode_value(mode: QueryFilterMode) -> &'static str {
    match mode {
        QueryFilterMode::And => "and",
        QueryFilterMode::Or => "or",
    }
}

fn filter_operator_value(operator: QueryFilterOperator) -> &'static str {
    match operator {
        QueryFilterOperator::Contains => "contains",
        QueryFilterOperator::NotContains => "not_contains",
        QueryFilterOperator::Equals => "equals",
        QueryFilterOperator::NotEquals => "not_equals",
        QueryFilterOperator::StartsWith => "starts_with",
        QueryFilterOperator::EndsWith => "ends_with",
        QueryFilterOperator::IsNull => "is_null",
        QueryFilterOperator::IsNotNull => "is_not_null",
    }
}

fn filter_operator_label(operator: QueryFilterOperator) -> &'static str {
    match operator {
        QueryFilterOperator::Contains => "Contains",
        QueryFilterOperator::NotContains => "Does not contain",
        QueryFilterOperator::Equals => "Equals",
        QueryFilterOperator::NotEquals => "Does not equal",
        QueryFilterOperator::StartsWith => "Starts with",
        QueryFilterOperator::EndsWith => "Ends with",
        QueryFilterOperator::IsNull => "Is null",
        QueryFilterOperator::IsNotNull => "Is not null",
    }
}

fn parse_filter_operator(value: &str) -> QueryFilterOperator {
    match value {
        "not_contains" => QueryFilterOperator::NotContains,
        "equals" => QueryFilterOperator::Equals,
        "not_equals" => QueryFilterOperator::NotEquals,
        "starts_with" => QueryFilterOperator::StartsWith,
        "ends_with" => QueryFilterOperator::EndsWith,
        "is_null" => QueryFilterOperator::IsNull,
        "is_not_null" => QueryFilterOperator::IsNotNull,
        _ => QueryFilterOperator::Contains,
    }
}

fn format_row_json(columns: &[String], row: &[String]) -> String {
    let mut object = Map::with_capacity(columns.len());
    for (column, value) in columns.iter().zip(row.iter()) {
        object.insert(column.clone(), detail_json_value(value));
    }

    serde_json::to_string_pretty(&Value::Object(object)).unwrap_or_else(|_| "{}".to_string())
}

fn detail_json_value(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        Value::Null
    } else if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        serde_json::from_str::<Value>(trimmed).unwrap_or_else(|_| Value::String(value.to_string()))
    } else {
        Value::String(value.to_string())
    }
}

fn original_cell_value(
    page: &models::QueryPage,
    locator: &str,
    col_index: usize,
) -> Option<String> {
    let editable = page.editable.as_ref()?;
    let row_index = editable
        .row_locators
        .iter()
        .position(|current_locator| current_locator == locator)?;
    page.rows.get(row_index)?.get(col_index).cloned()
}

fn commit_cell_edit(
    mut editing_cell: Signal<Option<EditingCell>>,
    mut tabs: Signal<Vec<QueryTabState>>,
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
    if page.editable.is_none() {
        editing_cell.set(None);
        return;
    }
    let Some(column_name) = page.columns.get(editing.col_index).cloned() else {
        editing_cell.set(None);
        return;
    };

    editing_cell.set(None);
    tabs.with_mut(|all_tabs| {
        let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) else {
            return;
        };

        match editing.row_ref {
            EditableRowRef::PendingInsert(insert_id) => {
                if let Some(row) = tab
                    .pending_table_changes
                    .inserted_rows
                    .iter_mut()
                    .find(|row| row.id == insert_id)
                    && let Some(value) = row.values.get_mut(editing.col_index)
                {
                    *value = Some(editing.value);
                }
            }
            EditableRowRef::Existing(locator) => {
                let original_value =
                    original_cell_value(&page, locator.as_str(), editing.col_index)
                        .unwrap_or_default();

                if original_value == editing.value {
                    tab.pending_table_changes.updated_cells.retain(|change| {
                        !(change.locator == locator && change.column_name == column_name)
                    });
                } else if let Some(change) = tab
                    .pending_table_changes
                    .updated_cells
                    .iter_mut()
                    .find(|change| change.locator == locator && change.column_name == column_name)
                {
                    change.value = editing.value;
                } else {
                    tab.pending_table_changes
                        .updated_cells
                        .push(PendingCellChange {
                            locator,
                            column_name,
                            value: editing.value,
                        });
                }
            }
        }

        tab.status = pending_changes_summary(&tab.pending_table_changes);
    });
}

fn insert_empty_row(mut tabs: Signal<Vec<QueryTabState>>, active_tab_id: Signal<u64>) {
    let current_id = active_tab_id();
    let current_tab = tabs.read().iter().find(|tab| tab.id == current_id).cloned();
    let Some(current_tab) = current_tab else {
        return;
    };
    let Some(QueryOutput::Table(page)) = current_tab.result.clone() else {
        set_active_tab_status(tabs, current_id, "No editable table is open".to_string());
        return;
    };
    let Some(_) = page.editable.clone() else {
        set_active_tab_status(
            tabs,
            current_id,
            "Row insert is available only for editable table views".to_string(),
        );
        return;
    };
    let editable = page.editable.clone();
    let page_columns = page.columns.clone();
    let mut inserted_row_id = None;
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            let insert_id = tab.pending_table_changes.next_insert_id;
            tab.pending_table_changes.next_insert_id += 1;
            tab.pending_table_changes.inserted_rows.insert(
                0,
                PendingInsertRow {
                    id: insert_id,
                    values: vec![None; page.columns.len()],
                },
            );
            tab.status = pending_changes_summary(&tab.pending_table_changes);
            inserted_row_id = Some(insert_id);
        }
    });
    let (Some(editable), Some(inserted_row_id)) = (editable, inserted_row_id) else {
        return;
    };
    let Some(connection) = tab_connection_or_error(tabs, current_id, current_tab.session_id) else {
        return;
    };

    spawn(async move {
        match query::next_table_primary_key_id(connection, editable.source.clone()).await {
            Ok(Some((column_name, remote_next_id))) => {
                tabs.with_mut(|all_tabs| {
                    let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) else {
                        return;
                    };
                    let Some(column_index) = page_columns
                        .iter()
                        .position(|column| column.eq_ignore_ascii_case(&column_name))
                    else {
                        return;
                    };
                    let next_id = next_pending_auto_id(
                        &tab.pending_table_changes,
                        column_index,
                        remote_next_id,
                    );
                    let Some(row) = tab
                        .pending_table_changes
                        .inserted_rows
                        .iter_mut()
                        .find(|row| row.id == inserted_row_id)
                    else {
                        return;
                    };
                    let Some(value) = row.values.get_mut(column_index) else {
                        return;
                    };
                    if value.as_ref().is_some_and(|value| !value.trim().is_empty()) {
                        return;
                    }
                    *value = Some(next_id.to_string());
                });
            }
            Ok(None) => {}
            Err(err) => {
                set_active_tab_status(
                    tabs,
                    current_id,
                    format!("Draft row added without auto id: {err:?}"),
                );
            }
        }
    });
}

fn apply_pending_changes(mut tabs: Signal<Vec<QueryTabState>>, active_tab_id: Signal<u64>) {
    let current_id = active_tab_id();
    let current_tab = tabs.read().iter().find(|tab| tab.id == current_id).cloned();
    let Some(current_tab) = current_tab else {
        return;
    };
    let Some(QueryOutput::Table(page)) = current_tab.result.clone() else {
        set_active_tab_status(tabs, current_id, "No editable table is open".to_string());
        return;
    };
    let Some(editable) = page.editable.clone() else {
        set_active_tab_status(
            tabs,
            current_id,
            "Changes can be applied only for editable table views".to_string(),
        );
        return;
    };
    let pending_changes = current_tab.pending_table_changes.clone();
    if pending_changes.is_empty() {
        set_active_tab_status(tabs, current_id, "No pending changes".to_string());
        return;
    }

    let Some(connection) = tab_connection_or_error(tabs, current_id, current_tab.session_id) else {
        return;
    };

    let columns = page.columns.clone();
    let summary = pending_changes_summary(&pending_changes);
    set_active_tab_status(tabs, current_id, format!("Applying {summary}..."));

    spawn(async move {
        for row in pending_changes.inserted_rows {
            let column_values = columns
                .iter()
                .cloned()
                .zip(row.values.into_iter())
                .filter_map(|(column_name, value)| value.map(|value| (column_name, value)))
                .collect::<Vec<_>>();

            if let Err(err) = query::insert_table_row_with_values(
                connection.clone(),
                editable.source.clone(),
                column_values,
            )
            .await
            {
                set_active_tab_status(tabs, current_id, format!("Row insert error: {err:?}"));
                return;
            }
        }

        for change in pending_changes.updated_cells {
            if let Err(err) = query::update_table_cell(
                connection.clone(),
                editable.source.clone(),
                change.locator,
                change.column_name,
                change.value,
            )
            .await
            {
                set_active_tab_status(tabs, current_id, format!("Cell update error: {err:?}"));
                return;
            }
        }

        for delete in pending_changes.deleted_rows {
            if let Err(err) =
                query::delete_table_row(connection.clone(), editable.source.clone(), delete.locator)
                    .await
            {
                set_active_tab_status(tabs, current_id, format!("Row delete error: {err:?}"));
                return;
            }
        }

        let mut updated_tab = None;
        tabs.with_mut(|all_tabs| {
            if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                tab.pending_table_changes = PendingTableChanges::default();
                tab.status = format!("Applied changes to {}", editable.source.table_name);
                updated_tab = Some(tab.clone());
            }
        });

        if let Some(updated_tab) = updated_tab {
            refresh_tab_result(tabs, updated_tab, Some(editable.source));
        }
    });
}

fn discard_pending_changes(mut tabs: Signal<Vec<QueryTabState>>, active_tab_id: Signal<u64>) {
    let current_id = active_tab_id();
    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.pending_table_changes = PendingTableChanges::default();
            tab.status = "Discarded pending changes".to_string();
        }
    });
}

fn delete_selected_row(
    mut tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    row_index: usize,
) {
    let current_id = active_tab_id();
    let current_tab = tabs.read().iter().find(|tab| tab.id == current_id).cloned();
    let Some(current_tab) = current_tab else {
        return;
    };
    let Some(QueryOutput::Table(page)) = current_tab.result.clone() else {
        set_active_tab_status(tabs, current_id, "No editable table is open".to_string());
        return;
    };
    let Some(_editable) = page.editable.clone() else {
        set_active_tab_status(
            tabs,
            current_id,
            "Row delete is available only for editable table views".to_string(),
        );
        return;
    };
    let display_rows = materialize_display_rows(&page, &current_tab.pending_table_changes);
    let Some(row) = display_rows.get(row_index).cloned() else {
        set_active_tab_status(
            tabs,
            current_id,
            "The selected row is no longer available".to_string(),
        );
        return;
    };

    if let EditableRowRef::PendingInsert(insert_id) = row.row_ref {
        tabs.with_mut(|all_tabs| {
            if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
                tab.pending_table_changes
                    .inserted_rows
                    .retain(|row| row.id != insert_id);
                tab.status = pending_changes_summary(&tab.pending_table_changes);
            }
        });
        return;
    }

    let EditableRowRef::Existing(locator) = row.row_ref else {
        return;
    };

    tabs.with_mut(|all_tabs| {
        if let Some(tab) = all_tabs.iter_mut().find(|tab| tab.id == current_id) {
            tab.pending_table_changes
                .deleted_rows
                .push(PendingDeleteRow {
                    locator: locator.clone(),
                });
            tab.pending_table_changes
                .updated_cells
                .retain(|change| change.locator != locator);
            tab.status = pending_changes_summary(&tab.pending_table_changes);
        }
    });
}

fn next_pending_auto_id(
    pending_changes: &PendingTableChanges,
    column_index: usize,
    remote_next_id: i64,
) -> i64 {
    let pending_next_id = pending_changes
        .inserted_rows
        .iter()
        .filter_map(|row| row.values.get(column_index))
        .filter_map(|value| value.as_ref())
        .filter_map(|value| value.trim().parse::<i64>().ok())
        .max()
        .map(|max_id| max_id + 1)
        .unwrap_or(remote_next_id);

    pending_next_id.max(remote_next_id)
}
