use crate::app_state::{APP_STATE, activate_session, remove_session, session_connection};
use crate::screens::workspace::actions::{
    ensure_tab_for_session, mark_table_deleted, mark_table_truncated, run_table_preview_for_tab,
    tab_connection_or_error,
};
use crate::screens::workspace::components::{ActionIcon, IconButton};
use dioxus::prelude::*;
use models::{DatabaseKind, ExplorerNode, ExplorerNodeKind, QueryTabState, TablePreviewSource};
use rfd::{AsyncMessageDialog, MessageButtons, MessageDialogResult, MessageLevel};
use std::collections::HashSet;

const CUSTOM_TYPE_VALUE: &str = "__custom__";

#[derive(Clone, PartialEq)]
pub struct ExplorerConnectionSection {
    pub session_id: u64,
    pub name: String,
    pub kind_label: String,
    pub status: String,
    pub is_active: bool,
    pub nodes: Vec<ExplorerNode>,
}

#[derive(Clone, PartialEq)]
struct CreateTableTarget {
    session_id: u64,
    connection_name: String,
    kind: DatabaseKind,
    schemas: Vec<String>,
}

#[derive(Clone, PartialEq)]
struct DuplicateTableTarget {
    session_id: u64,
    connection_name: String,
    kind: DatabaseKind,
    source: TablePreviewSource,
}

#[derive(Clone, PartialEq)]
struct CreateTableDraft {
    schema: String,
    table_name: String,
    columns: Vec<CreateTableColumnDraft>,
    clickhouse_engine: ClickHouseEnginePreset,
}

#[derive(Clone, PartialEq)]
struct CreateTableColumnDraft {
    name: String,
    data_type: String,
    default_value: String,
    not_null: bool,
    key: bool,
    unique: bool,
    auto_increment: bool,
}

#[derive(Clone, Debug)]
struct CreateTableRequestPayload {
    columns_sql: String,
    clickhouse_engine: Option<String>,
}

#[derive(Clone, PartialEq)]
struct DuplicateTableDraft {
    table_name: String,
    copy_data: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClickHouseEnginePreset {
    MergeTree,
    ReplacingMergeTree,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TableMutationKind {
    Truncate,
    Drop,
}

#[component]
pub fn SidebarConnectionTree(
    sections: Vec<ExplorerConnectionSection>,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
) -> Element {
    let selected_node = use_signal(String::new);
    let mut show_create_table = use_signal(|| false);
    let mut filter_query = use_signal(String::new);
    let query = filter_query();
    let active_create_target = active_create_table_target(&sections);
    let filtered_sections = filter_connection_sections(&sections, &query);
    let entity_count = filtered_sections
        .iter()
        .map(|section| count_objects(&section.nodes))
        .sum::<usize>();

    rsx! {
        div { class: "tree",
            div {
                class: "tree__header",
                div {
                    class: "tree__header-copy",
                    span { class: "tree__header-label", "Entities" }
                    span { class: "tree__header-count", "{entity_count}" }
                }
                div {
                    class: "tree__header-actions",
                    IconButton {
                        icon: ActionIcon::CreateTable,
                        label: "Create table".to_string(),
                        small: true,
                        disabled: active_create_target.is_none(),
                        onclick: move |_| show_create_table.set(true),
                    }
                }
            }

            if sections.is_empty() {
                div {
                    class: "tree__body",
                    p { class: "empty-state", "No active connections." }
                }
            } else {
                div {
                    class: "tree__filter",
                    input {
                        class: "input tree__filter-input",
                        value: "{query}",
                        placeholder: "Filter entities",
                        oninput: move |event| filter_query.set(event.value()),
                    }
                }

                div {
                    class: "tree__body",
                    if filtered_sections.is_empty() {
                        p { class: "empty-state", "No matching tables or views." }
                    } else {
                        for section in filtered_sections {
                            ExplorerConnectionView {
                                section,
                                tree_reload,
                                tabs,
                                active_tab_id,
                                next_tab_id,
                                selected_node,
                            }
                        }
                    }
                }
            }

            if show_create_table() {
                if let Some(target) = active_create_target.clone() {
                    CreateTableModal {
                        target,
                        tree_reload,
                        show_create_table,
                    }
                }
            }
        }
    }
}

#[component]
fn CreateTableModal(
    target: CreateTableTarget,
    tree_reload: Signal<u64>,
    mut show_create_table: Signal<bool>,
) -> Element {
    let mut draft = use_signal(|| default_create_table_draft(&target));
    let mut create_error = use_signal(String::new);
    let mut create_inflight = use_signal(|| false);
    let current_draft = draft();
    let can_submit = create_table_form_valid(target.kind, &current_draft) && !create_inflight();
    let preview_sql = create_table_preview_sql(target.kind, &current_draft);

    rsx! {
        div {
            class: "settings-modal__backdrop",
            onclick: move |_| show_create_table.set(false),
            div {
                class: "settings-modal table-modal",
                onclick: move |event| event.stop_propagation(),
                div {
                    class: "settings-modal__header",
                    div {
                        class: "settings-modal__header-copy",
                        h2 { class: "settings-modal__title", "Create Table" }
                        p {
                            class: "settings-modal__hint",
                            "Create a new table in {target.connection_name}."
                        }
                    }
                    button {
                        class: "button button--ghost button--small",
                        onclick: move |_| show_create_table.set(false),
                        "Close"
                    }
                }

                div {
                    class: "table-modal__body",
                    div {
                        class: "table-modal__grid",
                        if target.kind == DatabaseKind::Sqlite {
                            div {
                                class: "field",
                                span { class: "field__label", "Schema" }
                                input {
                                    class: "input",
                                    value: current_draft.schema.clone(),
                                    readonly: true,
                                }
                            }
                        } else if !target.schemas.is_empty() {
                            div {
                                class: "field",
                                span { class: "field__label", "Schema" }
                                select {
                                    class: "input",
                                    value: current_draft.schema.clone(),
                                    oninput: move |event| {
                                        let value = event.value();
                                        draft.with_mut(|draft| draft.schema = value);
                                    },
                                    for schema in target.schemas.iter().cloned() {
                                        option {
                                            value: schema.clone(),
                                            "{schema}"
                                        }
                                    }
                                }
                            }
                        }

                        div {
                            class: "field",
                            span { class: "field__label", "Table name" }
                            input {
                                class: "input",
                                value: current_draft.table_name.clone(),
                                placeholder: "events",
                                oninput: move |event| {
                                    let value = event.value();
                                    draft.with_mut(|draft| draft.table_name = value);
                                },
                            }
                        }
                    }

                    div {
                        class: "table-modal__section",
                        div {
                            class: "table-modal__section-header",
                            div {
                                class: "table-modal__section-copy",
                                span { class: "field__label", "Columns" }
                                p {
                                    class: "table-modal__hint",
                                    if target.kind == DatabaseKind::ClickHouse {
                                        "Unchecked Required wraps the type in Nullable(...). Sort key columns become ORDER BY."
                                    } else {
                                        "Configure each column with UI fields instead of writing column SQL by hand."
                                    }
                                }
                            }
                            button {
                                class: "button button--ghost button--small",
                                disabled: create_inflight(),
                                onclick: move |_| {
                                    draft.with_mut(|draft| draft.columns.push(new_create_table_column(target.kind)));
                                },
                                "Add column"
                            }
                        }

                        if current_draft.columns.is_empty() {
                            p {
                                class: "empty-state",
                                "Add at least one column."
                            }
                        } else {
                            div {
                                class: "table-modal__columns",
                                for (index, column) in current_draft.columns.iter().cloned().enumerate() {
                                    div {
                                        key: "{index}",
                                        class: "table-modal__column-card",
                                        div {
                                            class: "table-modal__column-header",
                                            div {
                                                class: "table-modal__column-title",
                                                if column.name.trim().is_empty() {
                                                    "Column {index + 1}"
                                                } else {
                                                    "{column.name}"
                                                }
                                            }
                                            button {
                                                class: "button button--ghost button--small",
                                                disabled: current_draft.columns.len() <= 1 || create_inflight(),
                                                onclick: move |_| {
                                                    draft.with_mut(|draft| {
                                                        if draft.columns.len() > 1 && index < draft.columns.len() {
                                                            draft.columns.remove(index);
                                                        }
                                                    });
                                                },
                                                "Remove"
                                            }
                                        }

                                        div {
                                            class: "table-modal__column-grid",
                                            div {
                                                class: "field",
                                                span { class: "field__label", "Name" }
                                                input {
                                                    class: "input",
                                                    value: column.name.clone(),
                                                    placeholder: if index == 0 { "id" } else { "name" },
                                                    oninput: move |event| {
                                                        let value = event.value();
                                                        draft.with_mut(|draft| {
                                                            if let Some(column) = draft.columns.get_mut(index) {
                                                                column.name = value;
                                                            }
                                                        });
                                                    },
                                                }
                                            }

                                            div {
                                                class: "field",
                                                span { class: "field__label", "Type" }
                                                select {
                                                    class: "input",
                                                    value: selected_create_table_type_value(target.kind, &column.data_type),
                                                    oninput: move |event| {
                                                        let value = event.value();
                                                        draft.with_mut(|draft| {
                                                            if let Some(column) = draft.columns.get_mut(index) {
                                                                column.data_type = apply_selected_create_table_type(
                                                                    target.kind,
                                                                    &column.data_type,
                                                                    &value,
                                                                );
                                                            }
                                                        });
                                                    },
                                                    for data_type in create_table_type_options(target.kind) {
                                                        option {
                                                            value: *data_type,
                                                            "{data_type}"
                                                        }
                                                    }
                                                    option {
                                                        value: CUSTOM_TYPE_VALUE,
                                                        "Custom"
                                                    }
                                                }
                                                if is_custom_create_table_type(target.kind, &column.data_type) {
                                                    input {
                                                        class: "input",
                                                        value: column.data_type.clone(),
                                                        placeholder: create_table_type_placeholder(target.kind, index),
                                                        oninput: move |event| {
                                                            let value = event.value();
                                                            draft.with_mut(|draft| {
                                                                if let Some(column) = draft.columns.get_mut(index) {
                                                                    column.data_type = value;
                                                                }
                                                            });
                                                        },
                                                    }
                                                }
                                            }

                                            div {
                                                class: "field",
                                                span { class: "field__label", "Default" }
                                                input {
                                                    class: "input",
                                                    value: column.default_value.clone(),
                                                    placeholder: create_table_default_placeholder(target.kind, index),
                                                    oninput: move |event| {
                                                        let value = event.value();
                                                        draft.with_mut(|draft| {
                                                            if let Some(column) = draft.columns.get_mut(index) {
                                                                column.default_value = value;
                                                            }
                                                        });
                                                    },
                                                }
                                            }
                                        }

                                        div {
                                            class: "table-modal__column-toggles",
                                            label {
                                                class: "settings-modal__toggle",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: column.not_null,
                                                    oninput: move |event| {
                                                        let checked = event.checked();
                                                        draft.with_mut(|draft| {
                                                            if let Some(column) = draft.columns.get_mut(index) {
                                                                column.not_null = checked;
                                                            }
                                                        });
                                                    },
                                                }
                                                span { "{column_required_label(target.kind)}" }
                                            }

                                            label {
                                                class: "settings-modal__toggle",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: column.key,
                                                    oninput: move |event| {
                                                        let checked = event.checked();
                                                        draft.with_mut(|draft| {
                                                            if let Some(column) = draft.columns.get_mut(index) {
                                                                column.key = checked;
                                                            }
                                                        });
                                                    },
                                                }
                                                span { "{column_key_label(target.kind)}" }
                                            }

                                            if target.kind != DatabaseKind::ClickHouse {
                                                label {
                                                    class: "settings-modal__toggle",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: column.unique,
                                                        oninput: move |event| {
                                                            let checked = event.checked();
                                                            draft.with_mut(|draft| {
                                                                if let Some(column) = draft.columns.get_mut(index) {
                                                                    column.unique = checked;
                                                                }
                                                            });
                                                        },
                                                    }
                                                    span { "Unique" }
                                                }

                                                label {
                                                    class: "settings-modal__toggle",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: column.auto_increment,
                                                        oninput: move |event| {
                                                            let checked = event.checked();
                                                            draft.with_mut(|draft| {
                                                                if let Some(column) = draft.columns.get_mut(index) {
                                                                    column.auto_increment = checked;
                                                                    if checked {
                                                                        column.key = true;
                                                                    }
                                                                }
                                                            });
                                                        },
                                                    }
                                                    span { "{column_auto_increment_label(target.kind)}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if target.kind == DatabaseKind::ClickHouse {
                        div {
                            class: "table-modal__grid",
                            div {
                                class: "field",
                                span { class: "field__label", "Engine" }
                                select {
                                    class: "input",
                                    value: current_draft.clickhouse_engine.as_value(),
                                    oninput: move |event| {
                                        let value = event.value();
                                        draft.with_mut(|draft| {
                                            draft.clickhouse_engine = ClickHouseEnginePreset::from_value(&value);
                                        });
                                    },
                                    option { value: ClickHouseEnginePreset::MergeTree.as_value(), "MergeTree" }
                                    option { value: ClickHouseEnginePreset::ReplacingMergeTree.as_value(), "ReplacingMergeTree" }
                                    option { value: ClickHouseEnginePreset::Log.as_value(), "Log" }
                                }
                            }

                            div {
                                class: "field",
                                span { class: "field__label", "Order key" }
                                p {
                                    class: "table-modal__hint table-modal__hint--boxed",
                                    "{clickhouse_order_by_summary(&current_draft.columns)}"
                                }
                            }
                        }
                    }

                    div {
                        class: "table-modal__preview",
                        span { class: "field__label", "Preview" }
                        pre {
                            class: "table-modal__preview-sql",
                            if preview_sql.trim().is_empty() {
                                "-- SQL preview will appear here"
                            } else {
                                "{preview_sql}"
                            }
                        }
                    }

                    if !create_error().trim().is_empty() {
                        p {
                            class: "table-modal__error",
                            "{create_error}"
                        }
                    }

                    div {
                        class: "table-modal__actions",
                        button {
                            class: "button button--ghost button--small",
                            disabled: create_inflight(),
                            onclick: move |_| show_create_table.set(false),
                            "Cancel"
                        }
                        button {
                            class: "button button--primary button--small",
                            disabled: !can_submit,
                            onclick: {
                                let target = target.clone();
                                move |_| {
                                    let draft_value = draft();
                                    let table_name = draft_value.table_name.trim().to_string();
                                    if table_name.is_empty() {
                                        create_error.set("Table name is required.".to_string());
                                        return;
                                    }

                                    let request = match build_create_table_request(target.kind, &draft_value) {
                                        Ok(request) => request,
                                        Err(err) => {
                                            create_error.set(err);
                                            return;
                                        }
                                    };

                                    let schema = normalized_schema_input(target.kind, &draft_value.schema);
                                    create_error.set(String::new());
                                    create_inflight.set(true);

                                    spawn(async move {
                                        let Some(connection) = session_connection(target.session_id) else {
                                            create_error.set("The connection was closed.".to_string());
                                            create_inflight.set(false);
                                            return;
                                        };

                                        let result = query::create_table(
                                            connection,
                                            schema,
                                            table_name,
                                            request.columns_sql,
                                            request.clickhouse_engine,
                                        )
                                        .await;

                                        create_inflight.set(false);
                                        match result {
                                            Ok(()) => {
                                                show_create_table.set(false);
                                                tree_reload += 1;
                                            }
                                            Err(err) => {
                                                create_error.set(err.to_string());
                                            }
                                        }
                                    });
                                }
                            },
                            if create_inflight() { "Creating..." } else { "Create table" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DuplicateTableModal(
    target: DuplicateTableTarget,
    tree_reload: Signal<u64>,
    selected_node: Signal<String>,
    mut show_duplicate_table: Signal<bool>,
) -> Element {
    let mut draft = use_signal(|| default_duplicate_table_draft(&target));
    let mut duplicate_error = use_signal(String::new);
    let mut duplicate_inflight = use_signal(|| false);
    let current_draft = draft();
    let can_submit = duplicate_table_form_valid(&target, &current_draft) && !duplicate_inflight();
    let preview_sql = duplicate_table_preview_sql(&target, &current_draft);

    rsx! {
        div {
            class: "settings-modal__backdrop",
            onclick: move |_| {
                if !duplicate_inflight() {
                    show_duplicate_table.set(false);
                }
            },
            div {
                class: "settings-modal table-modal",
                onclick: move |event| event.stop_propagation(),
                div {
                    class: "settings-modal__header",
                    div {
                        class: "settings-modal__header-copy",
                        h2 { class: "settings-modal__title", "Duplicate Table" }
                        p {
                            class: "settings-modal__hint",
                            "Create a copy of {target.source.qualified_name} in {target.connection_name}."
                        }
                    }
                    button {
                        class: "button button--ghost button--small",
                        disabled: duplicate_inflight(),
                        onclick: move |_| show_duplicate_table.set(false),
                        "Close"
                    }
                }

                div {
                    class: "table-modal__body",
                    div {
                        class: "table-modal__grid",
                        div {
                            class: "field",
                            span { class: "field__label", "Source table" }
                            input {
                                class: "input",
                                value: target.source.qualified_name.clone(),
                                readonly: true,
                            }
                        }
                        div {
                            class: "field",
                            span { class: "field__label", "New table name" }
                            input {
                                class: "input",
                                value: current_draft.table_name.clone(),
                                placeholder: "products_copy",
                                oninput: move |event| {
                                    let value = event.value();
                                    draft.with_mut(|draft| draft.table_name = value);
                                },
                            }
                        }
                    }

                    div {
                        class: "table-modal__section",
                        label {
                            class: "settings-modal__toggle",
                            input {
                                r#type: "checkbox",
                                checked: current_draft.copy_data,
                                oninput: move |event| {
                                    let checked = event.checked();
                                    draft.with_mut(|draft| draft.copy_data = checked);
                                },
                            }
                            span { "Copy existing rows into the duplicated table" }
                        }
                        p {
                            class: "table-modal__hint table-modal__hint--boxed",
                            match target.kind {
                                DatabaseKind::Sqlite => {
                                    "SQLite duplicates the table definition and can optionally copy all rows. Indexes and triggers are not copied."
                                }
                                DatabaseKind::Postgres => {
                                    "PostgreSQL duplicates the table with LIKE INCLUDING ALL and can optionally copy all rows."
                                }
                                DatabaseKind::ClickHouse => {
                                    "ClickHouse duplicates the CREATE TABLE definition and can optionally copy all rows with INSERT SELECT."
                                }
                            }
                        }
                    }

                    div {
                        class: "table-modal__preview",
                        span { class: "field__label", "Preview" }
                        pre {
                            class: "table-modal__preview-sql",
                            "{preview_sql}"
                        }
                    }

                    if !duplicate_error().is_empty() {
                        p {
                            class: "table-modal__error",
                            "{duplicate_error}"
                        }
                    }

                    div {
                        class: "table-modal__actions",
                        button {
                            class: "button button--ghost",
                            disabled: duplicate_inflight(),
                            onclick: move |_| show_duplicate_table.set(false),
                            "Cancel"
                        }
                        button {
                            class: "button button--primary",
                            disabled: !can_submit,
                            onclick: move |_| {
                                if duplicate_inflight() {
                                    return;
                                }

                                let draft_value = draft();
                                let source = target.source.clone();
                                let target_kind = target.kind;
                                let next_table_name = draft_value.table_name.trim().to_string();
                                if next_table_name.is_empty() {
                                    duplicate_error.set("Enter a new table name.".to_string());
                                    return;
                                }
                                if next_table_name == source.table_name {
                                    duplicate_error.set(
                                        "New table name must be different from the source table."
                                            .to_string(),
                                    );
                                    return;
                                }

                                let Some(connection) = session_connection(target.session_id) else {
                                    duplicate_error.set(
                                        "The connection was closed before the table could be duplicated."
                                            .to_string(),
                                    );
                                    return;
                                };

                                duplicate_error.set(String::new());
                                duplicate_inflight.set(true);

                                spawn(async move {
                                    let result = query::duplicate_table(
                                        connection,
                                        source.clone(),
                                        next_table_name.clone(),
                                        draft_value.copy_data,
                                    )
                                    .await;

                                    duplicate_inflight.set(false);
                                    match result {
                                        Ok(()) => {
                                            selected_node.set(duplicated_qualified_name(
                                                &source,
                                                target_kind,
                                                &next_table_name,
                                            ));
                                            tree_reload += 1;
                                            show_duplicate_table.set(false);
                                        }
                                        Err(err) => {
                                            duplicate_error.set(err.to_string());
                                        }
                                    }
                                });
                            },
                            if duplicate_inflight() {
                                "Duplicating..."
                            } else {
                                "Duplicate table"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ExplorerConnectionView(
    section: ExplorerConnectionSection,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    selected_node: Signal<String>,
) -> Element {
    let mut expanded = use_signal(|| true);
    let object_count = count_objects(&section.nodes);

    rsx! {
        div { class: if section.is_active {
                "tree__connection tree__connection--active"
            } else {
                "tree__connection"
            },
            div {
                class: "tree__connection-header",
                button {
                    class: "tree__connection-toggle",
                    onclick: {
                        let session_id = section.session_id;
                        move |_| {
                            activate_session(session_id);
                            expanded.toggle();
                        }
                    },
                    span {
                        class: if expanded() {
                            "tree__chevron tree__chevron--open"
                        } else {
                            "tree__chevron"
                        },
                        ">"
                    }
                    div {
                        class: "tree__connection-copy",
                        div {
                            class: "tree__connection-topline",
                            span { class: "tree__connection-kind", "{section.kind_label}" }
                            span {
                                class: "tree__connection-title",
                                title: "{section.name}",
                                "{section.name}"
                            }
                            span {
                                class: "tree__connection-meta",
                                title: "{section.status} · {object_count} objects",
                                "{section.status} · {object_count} objects"
                            }
                        }
                    }
                }
                div {
                    class: "tree__connection-actions",
                    IconButton {
                        icon: ActionIcon::Close,
                        label: "Disconnect".to_string(),
                        small: true,
                        onclick: {
                            let session_id = section.session_id;
                            move |_| disconnect_session(tabs, active_tab_id, session_id)
                        },
                    }
                }
            }

            if expanded() {
                div { class: "tree__connection-body",
                    if section.nodes.is_empty() {
                        p { class: "empty-state", "No objects loaded for this connection." }
                    } else {
                        for node in section.nodes {
                            ExplorerSchemaView {
                                node,
                                session_id: section.session_id,
                                tree_reload,
                                tabs,
                                active_tab_id,
                                next_tab_id,
                                selected_node,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ExplorerSchemaView(
    node: ExplorerNode,
    session_id: u64,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    selected_node: Signal<String>,
) -> Element {
    let mut expanded = use_signal(|| true);
    let (tables, views) = split_children(&node.children);
    let object_count = tables.len() + views.len();

    rsx! {
        div { class: "tree__schema",
            button {
                class: "tree__schema-toggle",
                onclick: move |_| expanded.toggle(),
                span {
                    class: if expanded() {
                        "tree__chevron tree__chevron--open"
                    } else {
                        "tree__chevron"
                    },
                    ">"
                }
                div {
                    class: "tree__schema-copy",
                    span { class: "tree__schema-title", "{node.name}" }
                    span {
                        class: "tree__schema-meta",
                        "{object_count} objects"
                    }
                }
            }

            if expanded() {
                div { class: "tree__schema-body",
                    if !tables.is_empty() {
                        ExplorerGroupView {
                            title: "Tables".to_string(),
                            session_id,
                            tree_reload,
                            nodes: tables,
                            tabs,
                            active_tab_id,
                            next_tab_id,
                            selected_node,
                        }
                    }
                    if !views.is_empty() {
                        ExplorerGroupView {
                            title: "Views".to_string(),
                            session_id,
                            tree_reload,
                            nodes: views,
                            tabs,
                            active_tab_id,
                            next_tab_id,
                            selected_node,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ExplorerGroupView(
    title: String,
    session_id: u64,
    tree_reload: Signal<u64>,
    nodes: Vec<ExplorerNode>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    selected_node: Signal<String>,
) -> Element {
    rsx! {
        div { class: "tree__group",
            div { class: "tree__group-header", "{title}" }
            div { class: "tree__group-items",
                for node in nodes {
                    ExplorerObjectRow {
                        node,
                        session_id,
                        tree_reload,
                        tabs,
                        active_tab_id,
                        next_tab_id,
                        selected_node,
                    }
                }
            }
        }
    }
}

#[component]
fn ExplorerObjectRow(
    node: ExplorerNode,
    session_id: u64,
    tree_reload: Signal<u64>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    selected_node: Signal<String>,
) -> Element {
    let mut table_mutation_inflight = use_signal(|| None::<TableMutationKind>);
    let mut show_duplicate_table = use_signal(|| false);
    let (connection_name, connection_kind) = APP_STATE
        .read()
        .session(session_id)
        .map(|session| (session.name.clone(), session.kind))
        .unwrap_or_else(|| ("Connection".to_string(), DatabaseKind::Sqlite));
    let preview_source = TablePreviewSource {
        schema: node.schema.clone(),
        table_name: node.name.clone(),
        qualified_name: node.qualified_name.clone(),
    };
    let selected = selected_node() == node.qualified_name;
    let can_duplicate_table = node.kind == ExplorerNodeKind::Table;
    let can_truncate_table = node.kind == ExplorerNodeKind::Table;
    let can_drop_table = node.kind == ExplorerNodeKind::Table;
    let kind_badge = match node.kind {
        ExplorerNodeKind::Table => "T",
        ExplorerNodeKind::View => "V",
        ExplorerNodeKind::Schema => "",
    };
    let kind_label = match node.kind {
        ExplorerNodeKind::Table => "Table",
        ExplorerNodeKind::View => "View",
        ExplorerNodeKind::Schema => "Schema",
    };

    rsx! {
        div {
            class: if selected {
                "tree__object-row tree__object-row--selected"
            } else {
                "tree__object-row"
            },
            button {
                class: if selected {
                    "tree__object tree__object--selected"
                } else {
                    "tree__object"
                },
                onclick: {
                    let qualified_name = node.qualified_name.clone();
                    move |_| {
                        selected_node.set(qualified_name.clone());
                        activate_session(session_id);
                    }
                },
                ondoubleclick: {
                    let source = preview_source.clone();
                    let qualified_name = node.qualified_name.clone();
                    move |_| {
                        selected_node.set(qualified_name.clone());
                        let current_id =
                            ensure_tab_for_session(tabs, active_tab_id, next_tab_id, session_id);
                        let current_tab = tabs
                            .read()
                            .iter()
                            .find(|tab| tab.id == current_id)
                            .cloned();
                        let Some(current_tab) = current_tab else {
                            return;
                        };

                        let Some(connection) =
                            tab_connection_or_error(tabs, current_id, current_tab.session_id)
                        else {
                            return;
                        };

                        run_table_preview_for_tab(
                            tabs,
                            current_id,
                            connection,
                            source.clone(),
                            0,
                            current_tab.page_size,
                        );
                    }
                },
                div {
                    class: "tree__object-badge",
                    "{kind_badge}"
                }
                div {
                    class: "tree__object-copy",
                    div {
                        class: "tree__object-name",
                        title: "{node.qualified_name}",
                        "{node.name}"
                    }
                    div { class: "tree__object-kind", "{kind_label}" }
                }
            }
            if can_duplicate_table || can_truncate_table || can_drop_table {
                div { class: "tree__object-actions",
                    if can_duplicate_table {
                        IconButton {
                            icon: ActionIcon::Duplicate,
                            label: format!("Duplicate table {}", node.name),
                            small: true,
                            disabled: table_mutation_inflight().is_some(),
                            onclick: {
                                move |event: MouseEvent| {
                                    event.stop_propagation();
                                    show_duplicate_table.set(true);
                                }
                            },
                        }
                    }
                    if can_truncate_table {
                        IconButton {
                            icon: ActionIcon::Truncate,
                            label: table_mutation_button_label(
                                TableMutationKind::Truncate,
                                &node.name,
                                table_mutation_inflight() == Some(TableMutationKind::Truncate),
                            ),
                            small: true,
                            disabled: table_mutation_inflight().is_some(),
                            onclick: {
                                let source = preview_source.clone();
                                move |event: MouseEvent| {
                                    event.stop_propagation();
                                    if table_mutation_inflight().is_some() {
                                        return;
                                    }

                                    let source = source.clone();

                                    spawn(async move {
                                        let confirmation = AsyncMessageDialog::new()
                                            .set_title(table_mutation_dialog_title(
                                                TableMutationKind::Truncate,
                                            ))
                                            .set_description(table_mutation_confirmation_description(
                                                TableMutationKind::Truncate,
                                                connection_kind,
                                                &source,
                                            ))
                                            .set_buttons(MessageButtons::YesNo)
                                            .set_level(MessageLevel::Warning)
                                            .show()
                                            .await;

                                        if confirmation != MessageDialogResult::Yes {
                                            return;
                                        }

                                        let Some(connection) = session_connection(session_id) else {
                                            let _ = AsyncMessageDialog::new()
                                                .set_title(table_mutation_error_title(
                                                    TableMutationKind::Truncate,
                                                ))
                                                .set_description(table_mutation_connection_closed_description(
                                                    TableMutationKind::Truncate,
                                                ))
                                                .set_buttons(MessageButtons::Ok)
                                                .set_level(MessageLevel::Error)
                                                .show()
                                                .await;
                                            return;
                                        };

                                        let refresh_connection = connection.clone();
                                        table_mutation_inflight
                                            .set(Some(TableMutationKind::Truncate));
                                        let result =
                                            query::truncate_table(connection, source.clone()).await;
                                        table_mutation_inflight.set(None);

                                        match result {
                                            Ok(()) => {
                                                mark_table_truncated(
                                                    tabs,
                                                    session_id,
                                                    refresh_connection,
                                                    source.clone(),
                                                );
                                            }
                                            Err(err) => {
                                                let _ = AsyncMessageDialog::new()
                                                    .set_title(table_mutation_error_title(
                                                        TableMutationKind::Truncate,
                                                    ))
                                                    .set_description(format!(
                                                        "Failed to truncate {}.\n\n{}",
                                                        source.qualified_name,
                                                        err
                                                    ))
                                                    .set_buttons(MessageButtons::Ok)
                                                    .set_level(MessageLevel::Error)
                                                    .show()
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            },
                        }
                    }
                    IconButton {
                        icon: ActionIcon::Delete,
                        label: table_mutation_button_label(
                            TableMutationKind::Drop,
                            &node.name,
                            table_mutation_inflight() == Some(TableMutationKind::Drop),
                        ),
                        small: true,
                        disabled: table_mutation_inflight().is_some(),
                        onclick: {
                            let source = preview_source.clone();
                            let selected_qualified_name = node.qualified_name.clone();
                            move |event: MouseEvent| {
                                event.stop_propagation();
                                if table_mutation_inflight().is_some() {
                                    return;
                                }

                                let source = source.clone();
                                let selected_qualified_name = selected_qualified_name.clone();

                                spawn(async move {
                                    let confirmation = AsyncMessageDialog::new()
                                        .set_title(table_mutation_dialog_title(
                                            TableMutationKind::Drop,
                                        ))
                                        .set_description(table_mutation_confirmation_description(
                                            TableMutationKind::Drop,
                                            connection_kind,
                                            &source,
                                        ))
                                        .set_buttons(MessageButtons::YesNo)
                                        .set_level(MessageLevel::Warning)
                                        .show()
                                        .await;

                                    if confirmation != MessageDialogResult::Yes {
                                        return;
                                    }

                                    let Some(connection) = session_connection(session_id) else {
                                        let _ = AsyncMessageDialog::new()
                                            .set_title(table_mutation_error_title(
                                                TableMutationKind::Drop,
                                            ))
                                            .set_description(table_mutation_connection_closed_description(
                                                TableMutationKind::Drop,
                                            ))
                                            .set_buttons(MessageButtons::Ok)
                                            .set_level(MessageLevel::Error)
                                            .show()
                                            .await;
                                        return;
                                    };

                                    table_mutation_inflight.set(Some(TableMutationKind::Drop));
                                    let result = query::drop_table(connection, source.clone()).await;
                                    table_mutation_inflight.set(None);

                                    match result {
                                        Ok(()) => {
                                            if selected_node() == selected_qualified_name {
                                                selected_node.set(String::new());
                                            }
                                            mark_table_deleted(tabs, session_id, source.clone());
                                            tree_reload += 1;
                                        }
                                        Err(err) => {
                                            let _ = AsyncMessageDialog::new()
                                                .set_title(table_mutation_error_title(
                                                    TableMutationKind::Drop,
                                                ))
                                                .set_description(format!(
                                                    "Failed to drop {}.\n\n{}",
                                                    source.qualified_name,
                                                    err
                                                ))
                                                .set_buttons(MessageButtons::Ok)
                                                .set_level(MessageLevel::Error)
                                                .show()
                                                .await;
                                        }
                                    }
                                });
                            }
                        },
                    }
                }
            }
            if show_duplicate_table() {
                DuplicateTableModal {
                    target: DuplicateTableTarget {
                        session_id,
                        connection_name: connection_name.clone(),
                        kind: connection_kind,
                        source: preview_source.clone(),
                    },
                    tree_reload,
                    selected_node,
                    show_duplicate_table,
                }
            }
        }
    }
}

fn table_mutation_button_label(
    action: TableMutationKind,
    table_name: &str,
    inflight: bool,
) -> String {
    match (action, inflight) {
        (TableMutationKind::Truncate, true) => "Truncating table".to_string(),
        (TableMutationKind::Truncate, false) => format!("Truncate table {table_name}"),
        (TableMutationKind::Drop, true) => "Dropping table".to_string(),
        (TableMutationKind::Drop, false) => format!("Drop table {table_name}"),
    }
}

fn table_mutation_dialog_title(action: TableMutationKind) -> &'static str {
    match action {
        TableMutationKind::Truncate => "Truncate table",
        TableMutationKind::Drop => "Drop table",
    }
}

fn table_mutation_error_title(action: TableMutationKind) -> &'static str {
    match action {
        TableMutationKind::Truncate => "Truncate table failed",
        TableMutationKind::Drop => "Drop table failed",
    }
}

fn table_mutation_connection_closed_description(action: TableMutationKind) -> &'static str {
    match action {
        TableMutationKind::Truncate => {
            "The connection was closed before the table could be truncated."
        }
        TableMutationKind::Drop => "The connection was closed before the table could be dropped.",
    }
}

fn table_mutation_confirmation_description(
    action: TableMutationKind,
    kind: DatabaseKind,
    source: &TablePreviewSource,
) -> String {
    match action {
        TableMutationKind::Truncate => {
            let sql = match kind {
                DatabaseKind::Sqlite => format!("DELETE FROM {}", source.qualified_name),
                DatabaseKind::Postgres | DatabaseKind::ClickHouse => {
                    format!("TRUNCATE TABLE {}", source.qualified_name)
                }
            };
            format!(
                "Truncate {}?\n\nThis removes all rows but keeps the table structure by running {}.",
                source.table_name, sql,
            )
        }
        TableMutationKind::Drop => format!(
            "Drop {}?\n\nThis permanently removes the table by running DROP TABLE IF EXISTS {}. Dependent objects may prevent the operation.",
            source.table_name, source.qualified_name,
        ),
    }
}

fn active_create_table_target(sections: &[ExplorerConnectionSection]) -> Option<CreateTableTarget> {
    let section = sections
        .iter()
        .find(|section| section.is_active)
        .or_else(|| sections.first())?;
    let kind = APP_STATE.read().session(section.session_id)?.kind;
    let mut schemas = section
        .nodes
        .iter()
        .filter(|node| node.kind == ExplorerNodeKind::Schema)
        .map(|node| node.name.clone())
        .collect::<Vec<_>>();
    schemas.sort();
    schemas.dedup();

    if schemas.is_empty() {
        schemas.push(default_schema_name(kind));
    }

    Some(CreateTableTarget {
        session_id: section.session_id,
        connection_name: section.name.clone(),
        kind,
        schemas,
    })
}

fn default_create_table_draft(target: &CreateTableTarget) -> CreateTableDraft {
    CreateTableDraft {
        schema: preferred_schema_name(target.kind, &target.schemas),
        table_name: String::new(),
        columns: default_create_table_columns(target.kind),
        clickhouse_engine: ClickHouseEnginePreset::default_for(target.kind),
    }
}

fn default_duplicate_table_draft(target: &DuplicateTableTarget) -> DuplicateTableDraft {
    DuplicateTableDraft {
        table_name: format!("{}_copy", target.source.table_name.trim()),
        copy_data: true,
    }
}

fn create_table_form_valid(kind: DatabaseKind, draft: &CreateTableDraft) -> bool {
    !draft.table_name.trim().is_empty() && build_create_table_request(kind, draft).is_ok()
}

fn duplicate_table_form_valid(target: &DuplicateTableTarget, draft: &DuplicateTableDraft) -> bool {
    let table_name = draft.table_name.trim();
    !table_name.is_empty() && table_name != target.source.table_name.trim()
}

fn normalized_schema_input(kind: DatabaseKind, value: &str) -> Option<String> {
    let value = value.trim();
    match kind {
        DatabaseKind::Sqlite => Some(if value.is_empty() { "main" } else { value }.to_string()),
        _ if value.is_empty() => None,
        _ => Some(value.to_string()),
    }
}

fn default_schema_name(kind: DatabaseKind) -> String {
    match kind {
        DatabaseKind::Sqlite => "main".to_string(),
        DatabaseKind::Postgres => "public".to_string(),
        DatabaseKind::ClickHouse => "default".to_string(),
    }
}

fn preferred_schema_name(kind: DatabaseKind, schemas: &[String]) -> String {
    let preferred = default_schema_name(kind);
    schemas
        .iter()
        .find(|schema| schema.eq_ignore_ascii_case(&preferred))
        .cloned()
        .or_else(|| schemas.first().cloned())
        .unwrap_or(preferred)
}

fn create_table_preview_sql(kind: DatabaseKind, draft: &CreateTableDraft) -> String {
    let table_name = draft.table_name.trim();
    let schema = normalized_schema_input(kind, &draft.schema);
    let table_name = if table_name.is_empty() {
        "<table_name>"
    } else {
        table_name
    };
    let qualified_name = quoted_table_name_preview(kind, schema.as_deref(), table_name);
    let definition = preview_create_table_columns_sql(kind, &draft.columns);

    match kind {
        DatabaseKind::ClickHouse => format!(
            "CREATE TABLE {qualified_name} {definition}\n{}",
            preview_clickhouse_engine_clause(draft)
        ),
        _ => format!("CREATE TABLE {qualified_name} {definition}"),
    }
}

fn duplicate_table_preview_sql(
    target: &DuplicateTableTarget,
    draft: &DuplicateTableDraft,
) -> String {
    let table_name = draft.table_name.trim();
    if table_name.is_empty() {
        return "-- enter a new table name".to_string();
    }

    let source_name = target.source.qualified_name.trim();
    let target_name = duplicated_qualified_name(&target.source, target.kind, table_name);

    match target.kind {
        DatabaseKind::Sqlite => {
            let create_sql =
                format!("CREATE TABLE {target_name} /* definition copied from {source_name} */");
            if draft.copy_data {
                format!("{create_sql};\nINSERT INTO {target_name} SELECT * FROM {source_name};")
            } else {
                format!("{create_sql};")
            }
        }
        DatabaseKind::Postgres => {
            let create_sql =
                format!("CREATE TABLE {target_name} (LIKE {source_name} INCLUDING ALL)");
            if draft.copy_data {
                format!("{create_sql};\nINSERT INTO {target_name} SELECT * FROM {source_name};")
            } else {
                format!("{create_sql};")
            }
        }
        DatabaseKind::ClickHouse => {
            let create_sql =
                format!("CREATE TABLE {target_name} /* definition copied from {source_name} */");
            if draft.copy_data {
                format!("{create_sql};\nINSERT INTO {target_name} SELECT * FROM {source_name};")
            } else {
                format!("{create_sql};")
            }
        }
    }
}

fn duplicated_qualified_name(
    source: &TablePreviewSource,
    kind: DatabaseKind,
    table_name: &str,
) -> String {
    match kind {
        DatabaseKind::Sqlite => quote_sql_identifier(table_name.trim()),
        DatabaseKind::Postgres | DatabaseKind::ClickHouse => {
            quoted_table_name_preview(kind, source.schema.as_deref(), table_name.trim())
        }
    }
}

fn default_create_table_columns(kind: DatabaseKind) -> Vec<CreateTableColumnDraft> {
    match kind {
        DatabaseKind::Sqlite => vec![
            CreateTableColumnDraft {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                default_value: String::new(),
                not_null: true,
                key: true,
                unique: false,
                auto_increment: true,
            },
            CreateTableColumnDraft {
                name: "name".to_string(),
                data_type: "TEXT".to_string(),
                default_value: String::new(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: false,
            },
            CreateTableColumnDraft {
                name: "created_at".to_string(),
                data_type: "TEXT".to_string(),
                default_value: "CURRENT_TIMESTAMP".to_string(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: false,
            },
        ],
        DatabaseKind::Postgres => vec![
            CreateTableColumnDraft {
                name: "id".to_string(),
                data_type: "BIGINT".to_string(),
                default_value: String::new(),
                not_null: true,
                key: true,
                unique: false,
                auto_increment: true,
            },
            CreateTableColumnDraft {
                name: "name".to_string(),
                data_type: "TEXT".to_string(),
                default_value: String::new(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: false,
            },
            CreateTableColumnDraft {
                name: "created_at".to_string(),
                data_type: "TIMESTAMPTZ".to_string(),
                default_value: "now()".to_string(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: false,
            },
        ],
        DatabaseKind::ClickHouse => vec![
            CreateTableColumnDraft {
                name: "id".to_string(),
                data_type: "UInt64".to_string(),
                default_value: String::new(),
                not_null: true,
                key: true,
                unique: false,
                auto_increment: false,
            },
            CreateTableColumnDraft {
                name: "name".to_string(),
                data_type: "String".to_string(),
                default_value: String::new(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: false,
            },
            CreateTableColumnDraft {
                name: "created_at".to_string(),
                data_type: "DateTime".to_string(),
                default_value: "now()".to_string(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: false,
            },
        ],
    }
}

fn new_create_table_column(kind: DatabaseKind) -> CreateTableColumnDraft {
    CreateTableColumnDraft {
        name: String::new(),
        data_type: create_table_default_type(kind, false).to_string(),
        default_value: String::new(),
        not_null: false,
        key: false,
        unique: false,
        auto_increment: false,
    }
}

fn create_table_default_type(kind: DatabaseKind, is_identity: bool) -> &'static str {
    match kind {
        DatabaseKind::Sqlite => {
            if is_identity {
                "INTEGER"
            } else {
                "TEXT"
            }
        }
        DatabaseKind::Postgres => {
            if is_identity {
                "BIGINT"
            } else {
                "TEXT"
            }
        }
        DatabaseKind::ClickHouse => {
            if is_identity {
                "UInt64"
            } else {
                "String"
            }
        }
    }
}

fn create_table_type_placeholder(kind: DatabaseKind, index: usize) -> &'static str {
    match (kind, index) {
        (DatabaseKind::Sqlite, 0) => "INTEGER",
        (DatabaseKind::Sqlite, _) => "TEXT",
        (DatabaseKind::Postgres, 0) => "BIGINT",
        (DatabaseKind::Postgres, _) => "TEXT",
        (DatabaseKind::ClickHouse, 0) => "UInt64",
        (DatabaseKind::ClickHouse, _) => "String",
    }
}

fn create_table_type_options(kind: DatabaseKind) -> &'static [&'static str] {
    match kind {
        DatabaseKind::Sqlite => &[
            "INTEGER", "TEXT", "REAL", "NUMERIC", "BLOB", "BOOLEAN", "DATE", "DATETIME", "JSON",
        ],
        DatabaseKind::Postgres => &[
            "SMALLINT",
            "INTEGER",
            "BIGINT",
            "NUMERIC",
            "REAL",
            "DOUBLE PRECISION",
            "BOOLEAN",
            "TEXT",
            "VARCHAR(255)",
            "UUID",
            "JSONB",
            "DATE",
            "TIMESTAMPTZ",
            "BYTEA",
        ],
        DatabaseKind::ClickHouse => &[
            "Int32",
            "Int64",
            "UInt32",
            "UInt64",
            "Float32",
            "Float64",
            "Decimal(18,2)",
            "Bool",
            "String",
            "FixedString(16)",
            "UUID",
            "Date",
            "Date32",
            "DateTime",
            "DateTime64(3)",
            "JSON",
        ],
    }
}

fn selected_create_table_type_value(kind: DatabaseKind, data_type: &str) -> &str {
    create_table_type_options(kind)
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(data_type.trim()))
        .unwrap_or(CUSTOM_TYPE_VALUE)
}

fn is_custom_create_table_type(kind: DatabaseKind, data_type: &str) -> bool {
    selected_create_table_type_value(kind, data_type) == CUSTOM_TYPE_VALUE
}

fn apply_selected_create_table_type(
    kind: DatabaseKind,
    current_data_type: &str,
    selected_value: &str,
) -> String {
    if selected_value == CUSTOM_TYPE_VALUE {
        if is_custom_create_table_type(kind, current_data_type) {
            current_data_type.to_string()
        } else {
            String::new()
        }
    } else {
        selected_value.to_string()
    }
}

fn create_table_default_placeholder(kind: DatabaseKind, index: usize) -> &'static str {
    match (kind, index) {
        (DatabaseKind::Sqlite, 2) => "CURRENT_TIMESTAMP",
        (DatabaseKind::Postgres, 2) => "now()",
        (DatabaseKind::ClickHouse, 2) => "now()",
        _ => "Optional expression",
    }
}

fn column_required_label(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::ClickHouse => "Required",
        DatabaseKind::Sqlite | DatabaseKind::Postgres => "Not null",
    }
}

fn column_key_label(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::ClickHouse => "Sort key",
        DatabaseKind::Sqlite | DatabaseKind::Postgres => "Primary key",
    }
}

fn column_auto_increment_label(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Sqlite => "Auto id",
        DatabaseKind::Postgres => "Identity",
        DatabaseKind::ClickHouse => "",
    }
}

fn preview_create_table_columns_sql(
    kind: DatabaseKind,
    columns: &[CreateTableColumnDraft],
) -> String {
    if columns.is_empty() {
        return "(\n  -- add at least one column\n)".to_string();
    }

    let key_count = columns.iter().filter(|column| column.key).count();
    let mut lines = columns
        .iter()
        .map(|column| preview_create_table_column_sql(kind, column, key_count))
        .collect::<Vec<_>>();

    if matches!(kind, DatabaseKind::Sqlite | DatabaseKind::Postgres) && key_count > 1 {
        let keys = columns
            .iter()
            .filter(|column| column.key)
            .map(|column| quote_sql_identifier(preview_column_name(column)))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("PRIMARY KEY ({keys})"));
    }

    format_columns_block(lines)
}

fn preview_create_table_column_sql(
    kind: DatabaseKind,
    column: &CreateTableColumnDraft,
    key_count: usize,
) -> String {
    let name = match kind {
        DatabaseKind::ClickHouse => quote_clickhouse_identifier(preview_column_name(column)),
        DatabaseKind::Sqlite | DatabaseKind::Postgres => {
            quote_sql_identifier(preview_column_name(column))
        }
    };
    let data_type = column
        .data_type
        .trim()
        .to_string()
        .chars()
        .collect::<String>();
    let data_type = if data_type.trim().is_empty() {
        create_table_default_type(kind, column.auto_increment).to_string()
    } else if kind == DatabaseKind::ClickHouse && !column.not_null {
        clickhouse_preview_data_type(&data_type)
    } else {
        data_type
    };

    let default_value = column.default_value.trim();
    match kind {
        DatabaseKind::Sqlite => {
            if column.auto_increment {
                format!("{name} INTEGER PRIMARY KEY AUTOINCREMENT")
            } else {
                let mut parts = vec![format!("{name} {data_type}")];
                if !default_value.is_empty() {
                    parts.push(format!("DEFAULT {default_value}"));
                }
                if column.not_null && !(key_count == 1 && column.key) {
                    parts.push("NOT NULL".to_string());
                }
                if column.unique && !column.key {
                    parts.push("UNIQUE".to_string());
                }
                if key_count == 1 && column.key {
                    parts.push("PRIMARY KEY".to_string());
                }
                parts.join(" ")
            }
        }
        DatabaseKind::Postgres => {
            let mut parts = vec![format!("{name} {data_type}")];
            if column.auto_increment {
                parts.push("GENERATED BY DEFAULT AS IDENTITY".to_string());
            }
            if !default_value.is_empty() {
                parts.push(format!("DEFAULT {default_value}"));
            }
            if column.not_null && !(key_count == 1 && column.key) {
                parts.push("NOT NULL".to_string());
            }
            if column.unique && !column.key {
                parts.push("UNIQUE".to_string());
            }
            if key_count == 1 && column.key {
                parts.push("PRIMARY KEY".to_string());
            }
            parts.join(" ")
        }
        DatabaseKind::ClickHouse => {
            let mut parts = vec![format!("{name} {data_type}")];
            if !default_value.is_empty() {
                parts.push(format!("DEFAULT {default_value}"));
            }
            parts.join(" ")
        }
    }
}

fn preview_column_name(column: &CreateTableColumnDraft) -> &str {
    if column.name.trim().is_empty() {
        "<column_name>"
    } else {
        column.name.trim()
    }
}

fn build_create_table_request(
    kind: DatabaseKind,
    draft: &CreateTableDraft,
) -> Result<CreateTableRequestPayload, String> {
    let columns = resolve_create_table_columns(kind, &draft.columns)?;
    let columns_sql = format_columns_block(
        columns
            .iter()
            .map(|column| column.sql.clone())
            .collect::<Vec<_>>(),
    );
    let clickhouse_engine = if kind == DatabaseKind::ClickHouse {
        Some(build_clickhouse_engine_clause(draft, &columns))
    } else {
        None
    };

    Ok(CreateTableRequestPayload {
        columns_sql,
        clickhouse_engine,
    })
}

#[derive(Clone)]
struct ResolvedCreateTableColumn {
    name: String,
    sql: String,
    key: bool,
}

fn resolve_create_table_columns(
    kind: DatabaseKind,
    columns: &[CreateTableColumnDraft],
) -> Result<Vec<ResolvedCreateTableColumn>, String> {
    if columns.is_empty() {
        return Err("Add at least one column.".to_string());
    }

    let mut seen = HashSet::new();
    let key_count = columns.iter().filter(|column| column.key).count();
    let auto_increment_count = columns
        .iter()
        .filter(|column| column.auto_increment)
        .count();

    if matches!(kind, DatabaseKind::Sqlite | DatabaseKind::Postgres) && auto_increment_count > 1 {
        return Err("Only one auto-generated key column is supported.".to_string());
    }
    if matches!(kind, DatabaseKind::Sqlite | DatabaseKind::Postgres)
        && auto_increment_count == 1
        && key_count != 1
    {
        return Err("Auto-generated keys require exactly one primary key column.".to_string());
    }

    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let name = column.name.trim();
            if name.is_empty() {
                return Err(format!("Column {} needs a name.", index + 1));
            }

            let normalized_name = name.to_ascii_lowercase();
            if !seen.insert(normalized_name) {
                return Err(format!("Duplicate column name: {name}."));
            }

            let data_type = column.data_type.trim();
            if data_type.is_empty() {
                return Err(format!("Column {name} needs a type."));
            }

            match kind {
                DatabaseKind::Sqlite => {
                    resolve_sqlite_create_table_column(column, data_type, key_count)
                }
                DatabaseKind::Postgres => {
                    resolve_postgres_create_table_column(column, data_type, key_count)
                }
                DatabaseKind::ClickHouse => {
                    resolve_clickhouse_create_table_column(column, data_type)
                }
            }
        })
        .collect()
}

fn resolve_sqlite_create_table_column(
    column: &CreateTableColumnDraft,
    data_type: &str,
    key_count: usize,
) -> Result<ResolvedCreateTableColumn, String> {
    let name = column.name.trim();
    let default_value = column.default_value.trim();
    let quoted_name = quote_sql_identifier(name);

    if column.auto_increment {
        if !column.key {
            return Err(format!(
                "SQLite auto id requires {} to be the primary key.",
                name
            ));
        }
        if key_count != 1 {
            return Err("SQLite auto id only works with a single-column primary key.".to_string());
        }
        if !sqlite_identity_type_supported(data_type) {
            return Err(format!(
                "SQLite auto id requires an INTEGER-like type for {name}."
            ));
        }
        if !default_value.is_empty() {
            return Err(format!(
                "SQLite auto id column {name} cannot also define DEFAULT."
            ));
        }

        return Ok(ResolvedCreateTableColumn {
            name: name.to_string(),
            sql: format!("{quoted_name} INTEGER PRIMARY KEY AUTOINCREMENT"),
            key: true,
        });
    }

    let mut parts = vec![format!("{quoted_name} {data_type}")];
    if !default_value.is_empty() {
        parts.push(format!("DEFAULT {default_value}"));
    }
    if column.not_null && !(key_count == 1 && column.key) {
        parts.push("NOT NULL".to_string());
    }
    if column.unique && !column.key {
        parts.push("UNIQUE".to_string());
    }
    if key_count == 1 && column.key {
        parts.push("PRIMARY KEY".to_string());
    }

    Ok(ResolvedCreateTableColumn {
        name: name.to_string(),
        sql: parts.join(" "),
        key: column.key,
    })
}

fn resolve_postgres_create_table_column(
    column: &CreateTableColumnDraft,
    data_type: &str,
    key_count: usize,
) -> Result<ResolvedCreateTableColumn, String> {
    let name = column.name.trim();
    let default_value = column.default_value.trim();
    let quoted_name = quote_sql_identifier(name);

    if column.auto_increment && !column.key {
        return Err(format!(
            "PostgreSQL identity requires {} to be part of the primary key.",
            name
        ));
    }
    if column.auto_increment && !postgres_identity_type_supported(data_type) {
        return Err(format!(
            "PostgreSQL identity requires an integer type for {name}."
        ));
    }
    if column.auto_increment && !default_value.is_empty() {
        return Err(format!(
            "PostgreSQL identity column {name} cannot also define DEFAULT."
        ));
    }

    let mut parts = vec![format!("{quoted_name} {data_type}")];
    if column.auto_increment {
        parts.push("GENERATED BY DEFAULT AS IDENTITY".to_string());
    }
    if !default_value.is_empty() {
        parts.push(format!("DEFAULT {default_value}"));
    }
    if column.not_null && !(key_count == 1 && column.key) {
        parts.push("NOT NULL".to_string());
    }
    if column.unique && !column.key {
        parts.push("UNIQUE".to_string());
    }
    if key_count == 1 && column.key {
        parts.push("PRIMARY KEY".to_string());
    }

    Ok(ResolvedCreateTableColumn {
        name: name.to_string(),
        sql: parts.join(" "),
        key: column.key,
    })
}

fn resolve_clickhouse_create_table_column(
    column: &CreateTableColumnDraft,
    data_type: &str,
) -> Result<ResolvedCreateTableColumn, String> {
    let name = column.name.trim();
    let default_value = column.default_value.trim();
    let quoted_name = quote_clickhouse_identifier(name);
    let resolved_type = if column.not_null {
        data_type.to_string()
    } else {
        wrap_clickhouse_nullable(data_type)
    };

    let mut parts = vec![format!("{quoted_name} {resolved_type}")];
    if !default_value.is_empty() {
        parts.push(format!("DEFAULT {default_value}"));
    }

    Ok(ResolvedCreateTableColumn {
        name: name.to_string(),
        sql: parts.join(" "),
        key: column.key,
    })
}

fn format_columns_block(lines: Vec<String>) -> String {
    if lines.is_empty() {
        "(\n  -- add at least one column\n)".to_string()
    } else {
        format!("(\n{}\n)", lines.join(",\n"))
    }
}

fn build_clickhouse_engine_clause(
    draft: &CreateTableDraft,
    columns: &[ResolvedCreateTableColumn],
) -> String {
    let key_columns = columns
        .iter()
        .filter(|column| column.key)
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    let order_by = clickhouse_order_by_expression(&key_columns);
    match draft.clickhouse_engine {
        ClickHouseEnginePreset::MergeTree => {
            format!("ENGINE = MergeTree() ORDER BY {order_by}")
        }
        ClickHouseEnginePreset::ReplacingMergeTree => {
            format!("ENGINE = ReplacingMergeTree() ORDER BY {order_by}")
        }
        ClickHouseEnginePreset::Log => "ENGINE = Log".to_string(),
    }
}

fn preview_clickhouse_engine_clause(draft: &CreateTableDraft) -> String {
    let key_columns = draft
        .columns
        .iter()
        .filter(|column| column.key)
        .map(preview_column_name)
        .collect::<Vec<_>>();
    let order_by = clickhouse_order_by_expression(&key_columns);
    match draft.clickhouse_engine {
        ClickHouseEnginePreset::MergeTree => {
            format!("ENGINE = MergeTree() ORDER BY {order_by}")
        }
        ClickHouseEnginePreset::ReplacingMergeTree => {
            format!("ENGINE = ReplacingMergeTree() ORDER BY {order_by}")
        }
        ClickHouseEnginePreset::Log => "ENGINE = Log".to_string(),
    }
}

fn clickhouse_order_by_summary(columns: &[CreateTableColumnDraft]) -> String {
    let keys = columns
        .iter()
        .filter(|column| column.key)
        .map(|column| column.name.trim())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();

    if keys.is_empty() {
        "No sort key selected. MergeTree engines will use ORDER BY tuple().".to_string()
    } else {
        format!("ORDER BY {}", clickhouse_order_by_expression(&keys))
    }
}

fn clickhouse_order_by_expression(keys: &[&str]) -> String {
    match keys {
        [] => "tuple()".to_string(),
        [single] => quote_clickhouse_identifier(single),
        _ => format!(
            "({})",
            keys.iter()
                .map(|name| quote_clickhouse_identifier(name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn quoted_table_name_preview(kind: DatabaseKind, schema: Option<&str>, table_name: &str) -> String {
    match kind {
        DatabaseKind::Sqlite | DatabaseKind::Postgres => match schema {
            Some(schema) => format!(
                "{}.{}",
                quote_sql_identifier(schema),
                quote_sql_identifier(table_name)
            ),
            None => quote_sql_identifier(table_name),
        },
        DatabaseKind::ClickHouse => {
            let schema = schema.unwrap_or("default");
            format!(
                "{}.{}",
                quote_clickhouse_identifier(schema),
                quote_clickhouse_identifier(table_name)
            )
        }
    }
}

fn quote_sql_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_clickhouse_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
}

fn wrap_clickhouse_nullable(data_type: &str) -> String {
    if data_type.trim_start().starts_with("Nullable(") {
        data_type.to_string()
    } else {
        format!("Nullable({data_type})")
    }
}

fn clickhouse_preview_data_type(data_type: &str) -> String {
    wrap_clickhouse_nullable(data_type.trim())
}

fn sqlite_identity_type_supported(data_type: &str) -> bool {
    data_type.to_ascii_lowercase().contains("int")
}

fn postgres_identity_type_supported(data_type: &str) -> bool {
    matches!(
        data_type.trim().to_ascii_lowercase().as_str(),
        "smallint" | "integer" | "bigint" | "int2" | "int4" | "int8"
    )
}

impl ClickHouseEnginePreset {
    fn default_for(kind: DatabaseKind) -> Self {
        match kind {
            DatabaseKind::ClickHouse => Self::MergeTree,
            DatabaseKind::Sqlite | DatabaseKind::Postgres => Self::Log,
        }
    }

    fn as_value(self) -> &'static str {
        match self {
            Self::MergeTree => "merge_tree",
            Self::ReplacingMergeTree => "replacing_merge_tree",
            Self::Log => "log",
        }
    }

    fn from_value(value: &str) -> Self {
        match value {
            "replacing_merge_tree" => Self::ReplacingMergeTree,
            "log" => Self::Log,
            _ => Self::MergeTree,
        }
    }
}

fn filter_connection_sections(
    sections: &[ExplorerConnectionSection],
    query: &str,
) -> Vec<ExplorerConnectionSection> {
    let query = query.trim();
    if query.is_empty() {
        return sections.to_vec();
    }

    let normalized = query.to_ascii_lowercase();
    sections
        .iter()
        .filter_map(|section| {
            let section_matches = matches_query(&section.name, &normalized)
                || matches_query(&section.kind_label, &normalized);
            let nodes = if section_matches {
                section.nodes.clone()
            } else {
                filter_nodes(&section.nodes, &normalized)
            };

            if section_matches || !nodes.is_empty() {
                let mut section = section.clone();
                section.nodes = nodes;
                Some(section)
            } else {
                None
            }
        })
        .collect()
}

fn filter_nodes(nodes: &[ExplorerNode], query: &str) -> Vec<ExplorerNode> {
    nodes
        .iter()
        .filter_map(|node| filter_node(node, query))
        .collect()
}

fn filter_node(node: &ExplorerNode, query: &str) -> Option<ExplorerNode> {
    match node.kind {
        ExplorerNodeKind::Schema => {
            let schema_matches = matches_query(&node.name, query);
            let mut filtered = node.clone();
            filtered.children = if schema_matches {
                node.children.clone()
            } else {
                filter_nodes(&node.children, query)
            };

            if schema_matches || !filtered.children.is_empty() {
                Some(filtered)
            } else {
                None
            }
        }
        ExplorerNodeKind::Table | ExplorerNodeKind::View => {
            if matches_query(&node.name, query) || matches_query(&node.qualified_name, query) {
                Some(node.clone())
            } else {
                None
            }
        }
    }
}

fn matches_query(value: &str, query: &str) -> bool {
    value.to_ascii_lowercase().contains(query)
}

fn split_children(children: &[ExplorerNode]) -> (Vec<ExplorerNode>, Vec<ExplorerNode>) {
    let mut tables = Vec::new();
    let mut views = Vec::new();

    for child in children {
        match child.kind {
            ExplorerNodeKind::Table => tables.push(child.clone()),
            ExplorerNodeKind::View => views.push(child.clone()),
            ExplorerNodeKind::Schema => {}
        }
    }

    tables.sort_by(|left, right| left.name.cmp(&right.name));
    views.sort_by(|left, right| left.name.cmp(&right.name));

    (tables, views)
}

fn disconnect_session(
    mut tabs: Signal<Vec<QueryTabState>>,
    mut active_tab_id: Signal<u64>,
    session_id: u64,
) {
    tabs.with_mut(|all_tabs| all_tabs.retain(|tab| tab.session_id != session_id));
    if let Some(first_tab) = tabs.read().first() {
        active_tab_id.set(first_tab.id);
        activate_session(first_tab.session_id);
    } else {
        active_tab_id.set(0);
    }
    remove_session(session_id);
}

fn count_objects(nodes: &[ExplorerNode]) -> usize {
    nodes.iter().map(|node| node.children.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::{
        ClickHouseEnginePreset, CreateTableColumnDraft, CreateTableDraft,
        build_create_table_request, duplicated_qualified_name, preview_clickhouse_engine_clause,
        selected_create_table_type_value,
    };
    use models::{DatabaseKind, TablePreviewSource};

    #[test]
    fn builds_sqlite_create_table_from_ui_fields() {
        let draft = CreateTableDraft {
            schema: "main".to_string(),
            table_name: "events".to_string(),
            columns: vec![
                CreateTableColumnDraft {
                    name: "id".to_string(),
                    data_type: "INTEGER".to_string(),
                    default_value: String::new(),
                    not_null: true,
                    key: true,
                    unique: false,
                    auto_increment: true,
                },
                CreateTableColumnDraft {
                    name: "name".to_string(),
                    data_type: "TEXT".to_string(),
                    default_value: String::new(),
                    not_null: true,
                    key: false,
                    unique: true,
                    auto_increment: false,
                },
            ],
            clickhouse_engine: ClickHouseEnginePreset::Log,
        };

        let request = build_create_table_request(DatabaseKind::Sqlite, &draft).expect("request");
        assert_eq!(
            request.columns_sql,
            "(\n\"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n\"name\" TEXT NOT NULL UNIQUE\n)"
        );
        assert!(request.clickhouse_engine.is_none());
    }

    #[test]
    fn rejects_postgres_identity_without_primary_key() {
        let draft = CreateTableDraft {
            schema: "public".to_string(),
            table_name: "events".to_string(),
            columns: vec![CreateTableColumnDraft {
                name: "id".to_string(),
                data_type: "BIGINT".to_string(),
                default_value: String::new(),
                not_null: true,
                key: false,
                unique: false,
                auto_increment: true,
            }],
            clickhouse_engine: ClickHouseEnginePreset::Log,
        };

        let err = build_create_table_request(DatabaseKind::Postgres, &draft).unwrap_err();
        assert!(err.contains("primary key"));
    }

    #[test]
    fn previews_clickhouse_engine_from_sort_key_columns() {
        let draft = CreateTableDraft {
            schema: "default".to_string(),
            table_name: "events".to_string(),
            columns: vec![
                CreateTableColumnDraft {
                    name: "tenant_id".to_string(),
                    data_type: "UInt64".to_string(),
                    default_value: String::new(),
                    not_null: true,
                    key: true,
                    unique: false,
                    auto_increment: false,
                },
                CreateTableColumnDraft {
                    name: "created_at".to_string(),
                    data_type: "DateTime".to_string(),
                    default_value: "now()".to_string(),
                    not_null: true,
                    key: true,
                    unique: false,
                    auto_increment: false,
                },
            ],
            clickhouse_engine: ClickHouseEnginePreset::MergeTree,
        };

        assert_eq!(
            preview_clickhouse_engine_clause(&draft),
            "ENGINE = MergeTree() ORDER BY (`tenant_id`, `created_at`)"
        );
    }

    #[test]
    fn preserves_custom_type_selection_separately_from_presets() {
        assert_eq!(
            selected_create_table_type_value(DatabaseKind::Postgres, "JSONB"),
            "JSONB"
        );
        assert_eq!(
            selected_create_table_type_value(DatabaseKind::Postgres, "citext"),
            "__custom__"
        );
    }

    #[test]
    fn duplicate_target_name_matches_explorer_qualified_name_format() {
        let sqlite_source = TablePreviewSource {
            schema: Some("main".to_string()),
            table_name: "products".to_string(),
            qualified_name: r#""products""#.to_string(),
        };
        let postgres_source = TablePreviewSource {
            schema: Some("public".to_string()),
            table_name: "products".to_string(),
            qualified_name: r#""public"."products""#.to_string(),
        };

        assert_eq!(
            duplicated_qualified_name(&sqlite_source, DatabaseKind::Sqlite, "products_copy"),
            r#""products_copy""#
        );
        assert_eq!(
            duplicated_qualified_name(&postgres_source, DatabaseKind::Postgres, "products_copy"),
            r#""public"."products_copy""#
        );
    }
}
