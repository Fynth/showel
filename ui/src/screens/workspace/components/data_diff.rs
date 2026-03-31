use dioxus::prelude::*;
use models::QueryPage;

#[derive(Clone, PartialEq)]
pub struct DiffColumn {
    pub name: String,
    pub left_value: String,
    pub right_value: String,
    pub status: DiffStatus,
}

#[derive(Clone, PartialEq)]
pub enum DiffStatus {
    Equal,
    Different,
    LeftOnly,
    RightOnly,
}

#[derive(Clone, PartialEq)]
pub struct DiffResult {
    pub columns: Vec<String>,
    pub differences: Vec<DiffRow>,
    pub summary: DiffSummary,
}

#[derive(Clone, PartialEq)]
pub struct DiffRow {
    pub row_index: usize,
    pub side: DiffSide,
    pub values: Vec<String>,
}

#[derive(Clone, PartialEq)]
pub enum DiffSide {
    Left,
    Right,
    Both,
}

#[derive(Clone, PartialEq)]
pub struct DiffSummary {
    pub total_rows_left: usize,
    pub total_rows_right: usize,
    pub identical_rows: usize,
    pub different_rows: usize,
    pub left_only_rows: usize,
    pub right_only_rows: usize,
}

#[component]
pub fn DataDiffViewer(
    left_data: Option<QueryPage>,
    right_data: Option<QueryPage>,
    left_label: String,
    right_label: String,
    on_close: Callback<()>,
) -> Element {
    let diff_result = use_memo(move || calculate_diff(left_data.as_ref(), right_data.as_ref()));

    rsx! {
        div {
            class: "data-diff",
            div {
                class: "data-diff__header",
                div {
                    class: "data-diff__title",
                    "Data Diff"
                }
                div {
                    class: "data-diff__labels",
                    span {
                        class: "data-diff__label data-diff__label--left",
                        "{left_label}"
                    }
                    span {
                        class: "data-diff__label data-diff__label--right",
                        "{right_label}"
                    }
                }
                button {
                    class: "data-diff__close",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            if let Some(result) = diff_result.as_ref() {
                div {
                    class: "data-diff__summary",
                    div {
                        class: "data-diff__summary-item",
                        span { class: "data-diff__summary-value", "{result.summary.identical_rows}" },
                        span { class: "data-diff__summary-label", "Identical" }
                    }
                    div {
                        class: "data-diff__summary-item",
                        span { class: "data-diff__summary-value data-diff__summary-value--danger", "{result.summary.different_rows}" },
                        span { class: "data-diff__summary-label", "Different" }
                    }
                    div {
                        class: "data-diff__summary-item",
                        span { class: "data-diff__summary-value data-diff__summary-value--warning", "{result.summary.left_only_rows}" },
                        span { class: "data-diff__summary-label", "Left Only" }
                    }
                    div {
                        class: "data-diff__summary-item",
                        span { class: "data-diff__summary-value data-diff__summary-value--warning", "{result.summary.right_only_rows}" },
                        span { class: "data-diff__summary-label", "Right Only" }
                    }
                }
                div {
                    class: "data-diff__content",
                    table {
                        class: "data-diff__table",
                        thead {
                            tr {
                                th {
                                    class: "data-diff__th data-diff__th--status",
                                    "Status"
                                }
                                th {
                                    class: "data-diff__th data-diff__th--row",
                                    "Row"
                                }
                                for col in result.columns.iter() {
                                    th {
                                        class: "data-diff__th",
                                        "{col}"
                                    }
                                }
                            }
                        }
                        tbody {
                            for diff_row in result.differences.iter() {
                                tr {
                                    class: match diff_row.side {
                                        DiffSide::Left => "data-diff__row data-diff__row--left",
                                        DiffSide::Right => "data-diff__row data-diff__row--right",
                                        DiffSide::Both => "data-diff__row data-diff__row--different",
                                    },
                                    td {
                                        class: "data-diff__td data-diff__td--status",
                                        match diff_row.side {
                                            DiffSide::Left => "←",
                                            DiffSide::Right => "→",
                                            DiffSide::Both => "≠",
                                        }
                                    }
                                    td {
                                        class: "data-diff__td data-diff__td--row",
                                        "{diff_row.row_index}"
                                    }
                                    for value in diff_row.values.iter() {
                                        td {
                                            class: "data-diff__td",
                                            "{value}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div {
                    class: "data-diff__empty-state",
                    "Select two result sets to compare"
                }
            }
        }
    }
}

fn calculate_diff(left: Option<&QueryPage>, right: Option<&QueryPage>) -> Option<DiffResult> {
    let (Some(left), Some(right)) = (left, right) else {
        return None;
    };

    if left.columns != right.columns {
        return None;
    }

    let columns = left.columns.clone();
    let mut differences = Vec::new();

    let left_rows: HashSet<_> = left.rows.iter().map(|row| row.join("\0")).collect();
    let right_rows: HashSet<_> = right.rows.iter().map(|row| row.join("\0")).collect();

    let mut identical_count = 0;
    let mut left_only_count = 0;
    let mut right_only_count = 0;
    let mut different_count = 0;

    for (idx, row) in left.rows.iter().enumerate() {
        let key = row.join("\0");
        if right_rows.contains(&key) {
            identical_count += 1;
        } else {
            let right_idx = right
                .rows
                .iter()
                .position(|r| r.iter().zip(row.iter()).filter(|(a, b)| a != b).count() == 0);
            if right_idx.is_some() {
                different_count += 1;
                differences.push(DiffRow {
                    row_index: idx + 1,
                    side: DiffSide::Both,
                    values: row.clone(),
                });
            } else {
                left_only_count += 1;
                differences.push(DiffRow {
                    row_index: idx + 1,
                    side: DiffSide::Left,
                    values: row.clone(),
                });
            }
        }
    }

    for (idx, row) in right.rows.iter().enumerate() {
        let key = row.join("\0");
        if !left_rows.contains(&key) && differences.iter().all(|d| d.values != *row) {
            right_only_count += 1;
            differences.push(DiffRow {
                row_index: idx + 1,
                side: DiffSide::Right,
                values: row.clone(),
            });
        }
    }

    differences.sort_by_key(|d| d.row_index);

    Some(DiffResult {
        columns,
        differences,
        summary: DiffSummary {
            total_rows_left: left.rows.len(),
            total_rows_right: right.rows.len(),
            identical_rows: identical_count,
            different_rows: different_count,
            left_only_rows: left_only_count,
            right_only_rows: right_only_count,
        },
    })
}

use std::collections::HashSet;
