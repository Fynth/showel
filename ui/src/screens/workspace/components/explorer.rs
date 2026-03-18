use crate::app_state::{activate_session, remove_session};
use crate::screens::workspace::actions::{
    ensure_tab_for_session, run_table_preview_for_tab, set_active_tab_sql, tab_connection_or_error,
};
use crate::screens::workspace::components::{ActionIcon, IconButton};
use dioxus::prelude::*;
use models::{ExplorerNode, ExplorerNodeKind, QueryTabState, TablePreviewSource};

#[derive(Clone, PartialEq)]
pub struct ExplorerConnectionSection {
    pub session_id: u64,
    pub name: String,
    pub kind_label: String,
    pub status: String,
    pub is_active: bool,
    pub nodes: Vec<ExplorerNode>,
}

#[component]
pub fn SidebarConnectionTree(
    sections: Vec<ExplorerConnectionSection>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
) -> Element {
    let selected_node = use_signal(String::new);
    let mut filter_query = use_signal(String::new);
    let query = filter_query();
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
            }

            if sections.is_empty() {
                p { class: "empty-state", "No active connections." }
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

                if filtered_sections.is_empty() {
                    p { class: "empty-state", "No matching tables or views." }
                } else {
                    for section in filtered_sections {
                        ExplorerConnectionView {
                            section,
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
fn ExplorerConnectionView(
    section: ExplorerConnectionSection,
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
                            span { class: "tree__connection-title", "{section.name}" }
                        }
                        span {
                            class: "tree__connection-meta",
                            "{section.status} · {object_count} objects"
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
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
    next_tab_id: Signal<u64>,
    selected_node: Signal<String>,
) -> Element {
    let preview_sql = format!("select * from {} limit 100;", node.qualified_name);
    let object_name = node.name.clone();
    let preview_source = TablePreviewSource {
        schema: node.schema.clone(),
        table_name: node.name.clone(),
        qualified_name: node.qualified_name.clone(),
    };
    let selected = selected_node() == node.qualified_name;
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
        button {
            class: if selected {
                "tree__object tree__object--selected"
            } else {
                "tree__object"
            },
            onclick: {
                let sql = preview_sql.clone();
                let qualified_name = node.qualified_name.clone();
                move |_| {
                    selected_node.set(qualified_name.clone());
                    let target_tab_id =
                        ensure_tab_for_session(tabs, active_tab_id, next_tab_id, session_id);
                    set_active_tab_sql(
                        tabs,
                        target_tab_id,
                        sql.clone(),
                        format!("Preview query ready for {object_name}. Double-click to load rows."),
                    );
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
