use dioxus::prelude::*;
use models::{ExecutionPlan, ExecutionPlanNode, QueryTabState};
use std::collections::HashSet;

/// Color category for a plan node operation
#[derive(Clone, Copy, PartialEq, Eq)]
enum OpCategory {
    Scan,
    Index,
    Join,
    Sort,
    Aggregate,
    Other,
}

fn classify_operation(op: &str) -> OpCategory {
    let lower = op.to_lowercase();
    if lower.contains("seq scan")
        || lower.contains("table scan")
        || lower.contains("scan table")
        || lower.contains("all")
        || lower.contains("readfrom")
    {
        OpCategory::Scan
    } else if lower.contains("index") {
        OpCategory::Index
    } else if lower.contains("join") || lower.contains("nested loop") {
        OpCategory::Join
    } else if lower.contains("sort") || lower.contains("order") {
        OpCategory::Sort
    } else if lower.contains("aggregate") || lower.contains("group") || lower.contains("hash") {
        OpCategory::Aggregate
    } else {
        OpCategory::Other
    }
}

fn op_category_class(cat: OpCategory) -> &'static str {
    match cat {
        OpCategory::Scan => "execution-plan__node-badge--scan",
        OpCategory::Index => "execution-plan__node-badge--index",
        OpCategory::Join => "execution-plan__node-badge--join",
        OpCategory::Sort => "execution-plan__node-badge--sort",
        OpCategory::Aggregate => "execution-plan__node-badge--aggregate",
        OpCategory::Other => "execution-plan__node-badge--other",
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlanViewMode {
    Tree,
    Raw,
}

type NodePath = Vec<usize>;

#[derive(Clone, Debug)]
struct VisiblePlanNode<'a> {
    node: &'a ExecutionPlanNode,
    path: NodePath,
    depth: usize,
    ancestor_has_more: Vec<bool>,
    is_last_sibling: bool,
    has_children: bool,
    is_expanded: bool,
}

fn collect_expandable_paths(nodes: &[ExecutionPlanNode]) -> HashSet<NodePath> {
    fn visit(nodes: &[ExecutionPlanNode], path: &mut Vec<usize>, result: &mut HashSet<NodePath>) {
        for (index, node) in nodes.iter().enumerate() {
            path.push(index);
            if !node.children.is_empty() {
                result.insert(path.clone());
                visit(&node.children, path, result);
            }
            path.pop();
        }
    }

    let mut result = HashSet::new();
    visit(nodes, &mut Vec::new(), &mut result);
    result
}

fn visible_plan_nodes<'a>(
    nodes: &'a [ExecutionPlanNode],
    expanded_paths: &HashSet<NodePath>,
) -> Vec<VisiblePlanNode<'a>> {
    fn visit<'a>(
        nodes: &'a [ExecutionPlanNode],
        expanded_paths: &HashSet<NodePath>,
        path: &mut Vec<usize>,
        ancestor_has_more: &mut Vec<bool>,
        depth: usize,
        result: &mut Vec<VisiblePlanNode<'a>>,
    ) {
        for (index, node) in nodes.iter().enumerate() {
            let is_last_sibling = index + 1 == nodes.len();
            path.push(index);
            let has_children = !node.children.is_empty();
            let is_expanded = has_children && expanded_paths.contains(path);

            result.push(VisiblePlanNode {
                node,
                path: path.clone(),
                depth,
                ancestor_has_more: ancestor_has_more.clone(),
                is_last_sibling,
                has_children,
                is_expanded,
            });

            if is_expanded {
                ancestor_has_more.push(!is_last_sibling);
                visit(
                    &node.children,
                    expanded_paths,
                    path,
                    ancestor_has_more,
                    depth + 1,
                    result,
                );
                ancestor_has_more.pop();
            }

            path.pop();
        }
    }

    let mut result = Vec::new();
    visit(
        nodes,
        expanded_paths,
        &mut Vec::new(),
        &mut Vec::new(),
        0,
        &mut result,
    );
    result
}

fn node_path_key(path: &[usize]) -> String {
    path.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::{collect_expandable_paths, visible_plan_nodes};
    use models::ExecutionPlanNode;
    use std::collections::HashSet;

    fn sample_plan_nodes() -> Vec<ExecutionPlanNode> {
        vec![
            ExecutionPlanNode::new("Root")
                .with_child(
                    ExecutionPlanNode::new("Child A").with_child(ExecutionPlanNode::new("Leaf")),
                )
                .with_child(ExecutionPlanNode::new("Child B")),
            ExecutionPlanNode::new("Other Root"),
        ]
    }

    #[test]
    fn visible_plan_nodes_hide_descendants_of_collapsed_nodes() {
        let nodes = sample_plan_nodes();
        let mut expanded = collect_expandable_paths(&nodes);
        expanded.remove(&vec![0]);

        let visible = visible_plan_nodes(&nodes, &expanded);
        let labels = visible
            .iter()
            .map(|entry| entry.node.operation.as_str())
            .collect::<Vec<_>>();

        assert_eq!(labels, vec!["Root", "Other Root"]);
    }

    #[test]
    fn visible_plan_nodes_keep_tree_metadata_for_connectors() {
        let nodes = sample_plan_nodes();
        let expanded = collect_expandable_paths(&nodes);

        let visible = visible_plan_nodes(&nodes, &expanded);
        let labels = visible
            .iter()
            .map(|entry| entry.node.operation.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec!["Root", "Child A", "Leaf", "Child B", "Other Root"]
        );
        assert_eq!(visible[2].path, vec![0, 0, 0]);
        assert_eq!(visible[2].depth, 2);
        assert_eq!(visible[2].ancestor_has_more, vec![true, true]);
        assert!(visible[3].is_last_sibling);
    }

    #[test]
    fn collect_expandable_paths_skips_leaf_nodes() {
        let nodes = sample_plan_nodes();
        let paths = collect_expandable_paths(&nodes);

        assert_eq!(paths, HashSet::from([vec![0], vec![0, 0]]));
    }
}

#[component]
pub fn ExecutionPlanView(
    plan: ExecutionPlan,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let _ = (tabs, active_tab_id);
    let mut view_mode = use_signal(|| PlanViewMode::Tree);
    let mut expanded_nodes = use_signal(HashSet::<NodePath>::new);
    let mut expanded_plan_key = use_signal(String::new);

    let flattened = plan.flattened_with_depth();
    let raw_text = plan.raw_text.join("\n");
    let all_expandable_paths = collect_expandable_paths(&plan.root_nodes);
    let plan_state_key = format!(
        "{}\u{1f}{}\u{1f}{}",
        plan.explained_sql,
        flattened.len(),
        raw_text
    );
    let needs_reset = {
        let last_key = expanded_plan_key.peek();
        last_key.as_str() != plan_state_key.as_str()
    };
    let expanded_snapshot = if needs_reset {
        all_expandable_paths.clone()
    } else {
        expanded_nodes()
    };
    if needs_reset {
        expanded_plan_key.set(plan_state_key);
        expanded_nodes.set(expanded_snapshot.clone());
    }

    let node_count = flattened.len();
    let has_timing = plan.execution_time_ms.is_some() || plan.planning_time_ms.is_some();
    let visible_nodes = visible_plan_nodes(&plan.root_nodes, &expanded_snapshot);

    rsx! {
        div { class: "execution-plan",
            // Header
            div { class: "execution-plan__header",
                div { class: "execution-plan__header-left",
                    span { class: "execution-plan__title",
                        "Execution Plan"
                    }
                    if plan.is_analyze {
                        span { class: "execution-plan__badge execution-plan__badge--analyze",
                            "ANALYZE"
                        }
                    }
                    span { class: "execution-plan__stat",
                        "{node_count} operations"
                    }
                }
                div { class: "execution-plan__header-right",
                    // View mode toggle
                    button {
                        class: if view_mode() == PlanViewMode::Tree {
                            "execution-plan__toggle execution-plan__toggle--active"
                        } else {
                            "execution-plan__toggle"
                        },
                        onclick: move |_| view_mode.set(PlanViewMode::Tree),
                        "Tree"
                    }
                    button {
                        class: if view_mode() == PlanViewMode::Raw {
                            "execution-plan__toggle execution-plan__toggle--active"
                        } else {
                            "execution-plan__toggle"
                        },
                        onclick: move |_| view_mode.set(PlanViewMode::Raw),
                        "Raw"
                    }
                }
            }

            // Summary stats
            if plan.total_cost.is_some() || has_timing {
                div { class: "execution-plan__stats",
                    if let Some(cost) = plan.total_cost {
                        div { class: "execution-plan__stat-chip",
                            span { class: "execution-plan__stat-label", "Total cost" }
                            span { class: "execution-plan__stat-value", "{cost:.2}" }
                        }
                    }
                    if let Some(pt) = plan.planning_time_ms {
                        div { class: "execution-plan__stat-chip",
                            span { class: "execution-plan__stat-label", "Planning" }
                            span { class: "execution-plan__stat-value", "{pt:.2} ms" }
                        }
                    }
                    if let Some(et) = plan.execution_time_ms {
                        div { class: "execution-plan__stat-chip",
                            span { class: "execution-plan__stat-label", "Execution" }
                            span { class: "execution-plan__stat-value", "{et:.2} ms" }
                        }
                    }
                }
            }

            // View content
            div { class: "execution-plan__content",
                match view_mode() {
                    PlanViewMode::Tree => rsx! {
                        div { class: "execution-plan__tree",
                            for node_view in &visible_nodes {
                                {
                                    let node = node_view.node;
                                    let node_path = node_view.path.clone();
                                    let node_key = node_path_key(&node_path);
                                    let is_expanded = node_view.is_expanded;
                                    let has_children = node_view.has_children;
                                    let cat = classify_operation(&node.operation);
                                    let badge_class = op_category_class(cat);
                                    let depth = node_view.depth;
                                    let ancestor_has_more = node_view.ancestor_has_more.clone();
                                    let is_last_sibling = node_view.is_last_sibling;
                                    let node_op = node.operation.clone();
                                    let node_target = node.target.clone();
                                    let node_details = node.details.clone();
                                    let node_cost = node.estimated_cost;
                                    let node_rows = node.estimated_rows;
                                    let node_actual_rows = node.actual_rows;
                                    let node_actual_time = node.actual_time_ms;
                                    let raw = node.raw_text.clone();

                                    rsx! {
                                        div {
                                            class: "execution-plan__node",
                                            key: "{node_key}",

                                            div { class: "execution-plan__tree-row",
                                                div { class: "execution-plan__guides",
                                                    for has_more in &ancestor_has_more {
                                                        span {
                                                            class: if *has_more {
                                                                "execution-plan__guide execution-plan__guide--continue"
                                                            } else {
                                                                "execution-plan__guide"
                                                            }
                                                        }
                                                    }
                                                    span {
                                                        class: if depth == 0 {
                                                            "execution-plan__guide execution-plan__guide--root"
                                                        } else if is_last_sibling {
                                                            "execution-plan__guide execution-plan__guide--branch-end"
                                                        } else {
                                                            "execution-plan__guide execution-plan__guide--branch-mid"
                                                        }
                                                    }
                                                }

                                                if has_children {
                                                    button {
                                                        class: "execution-plan__expand",
                                                        onclick: {
                                                            let mut expanded_nodes = expanded_nodes;
                                                            let path = node_path.clone();
                                                            move |_| {
                                                                if expanded_nodes().contains(&path) {
                                                                    expanded_nodes.write().remove(&path);
                                                                } else {
                                                                    expanded_nodes.write().insert(path.clone());
                                                                }
                                                            }
                                                        },
                                                        if is_expanded { "▼" } else { "▶" }
                                                    }
                                                } else {
                                                    span { class: "execution-plan__expand execution-plan__expand--leaf", "●" }
                                                }

                                                div { class: "execution-plan__node-content",
                                                    div { class: "execution-plan__node-header",
                                                        span { class: "execution-plan__node-badge {badge_class}",
                                                            "{node_op}"
                                                        }

                                                        if let Some(target) = &node_target {
                                                            span { class: "execution-plan__node-target",
                                                                "on {target}"
                                                            }
                                                        }
                                                    }

                                                    if node_cost.is_some() || node_rows.is_some() || node_actual_rows.is_some() || node_actual_time.is_some() {
                                                        div { class: "execution-plan__node-metrics",
                                                            if let Some(c) = node_cost {
                                                                span { class: "execution-plan__metric", "cost: {c:.2}" }
                                                            }
                                                            if let Some(r) = node_rows {
                                                                span { class: "execution-plan__metric", "rows: {r}" }
                                                            }
                                                            if let Some(r) = node_actual_rows {
                                                                span { class: "execution-plan__metric execution-plan__metric--actual",
                                                                    "actual: {r}"
                                                                }
                                                            }
                                                            if let Some(t) = node_actual_time {
                                                                span { class: "execution-plan__metric execution-plan__metric--actual",
                                                                    "time: {t:.2}ms"
                                                                }
                                                            }
                                                        }
                                                    }

                                                    if !node_details.is_empty() {
                                                        div { class: "execution-plan__node-details",
                                                            for (key, value) in &node_details {
                                                                span { class: "execution-plan__node-detail",
                                                                    "{key}: {value}"
                                                                }
                                                            }
                                                        }
                                                    }

                                                    if let Some(raw_text) = &raw {
                                                        if node_details.is_empty() && node_target.is_none() {
                                                            div { class: "execution-plan__node-details",
                                                                span { class: "execution-plan__node-raw",
                                                                    "{raw_text}"
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
                        }
                    },
                    PlanViewMode::Raw => rsx! {
                        div { class: "execution-plan__raw",
                            div { class: "execution-plan__raw-sql",
                                span { class: "execution-plan__stat-label", "Query:" }
                                code { "{plan.explained_sql}" }
                            }
                            pre { class: "execution-plan__raw-text",
                                "{raw_text}"
                            }
                        }
                    },
                }
            }
        }
    }
}
