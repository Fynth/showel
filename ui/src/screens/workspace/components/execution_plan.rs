use dioxus::prelude::*;
use models::{ExecutionPlan, QueryTabState};

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

#[component]
pub fn ExecutionPlanView(
    plan: ExecutionPlan,
    tabs: Signal<Vec<QueryTabState>>,
    active_tab_id: Signal<u64>,
) -> Element {
    let mut view_mode = use_signal(|| PlanViewMode::Tree);
    let mut expanded_nodes = use_signal(std::collections::HashSet::<usize>::new);

    // Initialize all nodes as expanded
    let flattened = plan.flattened_with_depth();
    if expanded_nodes().is_empty() && !flattened.is_empty() {
        let all_indices: std::collections::HashSet<usize> = (0..flattened.len()).collect();
        expanded_nodes.set(all_indices);
    }

    let node_count = flattened.len();
    let has_timing = plan.execution_time_ms.is_some() || plan.planning_time_ms.is_some();

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
                            for (idx, (node, depth)) in flattened.iter().enumerate() {
                                {
                                    let is_expanded = expanded_nodes().contains(&idx);
                                    let has_children = !node.children.is_empty();
                                    let cat = classify_operation(&node.operation);
                                    let badge_class = op_category_class(cat);
                                    let indent = *depth * 24;
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
                                            style: "padding-left: {indent}px;",

                                            // Expand/collapse toggle
                                            if has_children {
                                                button {
                                                    class: "execution-plan__expand",
                                                    onclick: {
                                                        let mut expanded_nodes = expanded_nodes;
                                                        move |_| {
                                                            if expanded_nodes().contains(&idx) {
                                                                expanded_nodes.write().remove(&idx);
                                                            } else {
                                                                expanded_nodes.write().insert(idx);
                                                            }
                                                        }
                                                    },
                                                    if is_expanded { "▼" } else { "▶" }
                                                }
                                            } else {
                                                span { class: "execution-plan__expand execution-plan__expand--leaf", "●" }
                                            }

                                            // Operation badge
                                            span { class: "execution-plan__node-badge {badge_class}",
                                                "{node_op}"
                                            }

                                            // Target
                                            if let Some(target) = &node_target {
                                                span { class: "execution-plan__node-target",
                                                    "on {target}"
                                                }
                                            }

                                            // Cost/rows chips
                                            if node_cost.is_some() || node_rows.is_some() {
                                                span { class: "execution-plan__node-metrics",
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

                                            // Details
                                            for (key, value) in &node_details {
                                                span { class: "execution-plan__node-detail",
                                                    "{key}: {value}"
                                                }
                                            }

                                            // Raw text fallback
                                            if let Some(raw_text) = &raw {
                                                if node_details.is_empty() && node_target.is_none() {
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
                    },
                    PlanViewMode::Raw => rsx! {
                        div { class: "execution-plan__raw",
                            div { class: "execution-plan__raw-sql",
                                span { class: "execution-plan__stat-label", "Query:" }
                                code { "{plan.explained_sql}" }
                            }
                            pre { class: "execution-plan__raw-text",
                                for line in &plan.raw_text {
                                    "{line}\n"
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}
