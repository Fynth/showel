use crate::screens::workspace::actions::{
    run_table_preview_for_tab, set_active_tab_sql, tab_connection_or_error,
};
use dioxus::prelude::*;
use models::{ExplorerNode, ExplorerNodeKind, QueryTabState, TablePreviewSource};

#[component]
pub fn SidebarConnectionTree(
    tree_nodes: Vec<ExplorerNode>,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    rsx! {
        if tree_nodes.is_empty() {
            p { class: "empty-state", "No schemas or objects loaded." }
        } else {
            div { class: "tree",
                for node in tree_nodes {
                    ExplorerNodeView {
                        node,
                        depth: 0,
                        tabs,
                        active_tab_id,
                    }
                }
            }
        }
    }
}

#[component]
fn ExplorerNodeView(
    node: ExplorerNode,
    depth: usize,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let label = match node.kind {
        ExplorerNodeKind::Schema => format!("schema: {}", node.name),
        ExplorerNodeKind::Table => format!("table: {}", node.name),
        ExplorerNodeKind::View => format!("view: {}", node.name),
    };
    let row_style = format!("--tree-depth: {depth};");
    let is_actionable = matches!(node.kind, ExplorerNodeKind::Table | ExplorerNodeKind::View);
    let preview_sql = format!("select * from {} limit 100;", node.qualified_name);
    let table_name = node.name.clone();
    let preview_source = TablePreviewSource {
        schema: node.schema.clone(),
        table_name: node.name.clone(),
        qualified_name: node.qualified_name.clone(),
    };

    rsx! {
        div { class: "tree__branch",
            if is_actionable {
                div {
                    class: "tree__row tree__row--interactive",
                    style: "{row_style}",
                    button {
                        class: "tree__button",
                        onclick: {
                            let sql = preview_sql.clone();
                            let object_name = table_name.clone();
                            move |_| {
                                set_active_tab_sql(
                                    tabs,
                                    active_tab_id(),
                                    sql.clone(),
                                    format!("Preview query ready for {object_name}. Double-click to load rows."),
                                );
                            }
                        },
                        ondoubleclick: {
                            let source = preview_source.clone();
                            move |_| {
                                let current_id = active_tab_id();
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
                        "{label}"
                    }
                }
            } else {
                p {
                    class: "tree__row tree__row--schema",
                    style: "{row_style}",
                    "{label}"
                }
            }
            for child in node.children {
                ExplorerNodeView {
                    node: child,
                    depth: depth + 1,
                    tabs,
                    active_tab_id,
                }
            }
        }
    }
}
