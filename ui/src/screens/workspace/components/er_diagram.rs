use dioxus::prelude::*;
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub struct ErTable {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ErColumn>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ErForeignKey>,
}

#[derive(Clone, PartialEq)]
pub struct ErColumn {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
}

#[derive(Clone, PartialEq)]
pub struct ErForeignKey {
    pub name: String,
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

#[derive(Clone, PartialEq)]
pub struct ErDiagramState {
    pub tables: Vec<ErTable>,
    pub relationships: Vec<ErRelationship>,
}

#[derive(Clone, PartialEq)]
pub struct ErRelationship {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ErLine {
    pub x1: String,
    pub y1: String,
    pub x2: String,
    pub y2: String,
}

#[component]
pub fn ErDiagramViewer(
    diagram_state: Signal<Option<ErDiagramState>>,
    on_close: Callback<()>,
    on_table_click: Callback<String>,
) -> Element {
    let mut view_offset = use_signal(|| (0.0f64, 0.0f64));
    let mut zoom = use_signal(|| 1.0f64);
    let mut is_dragging = use_signal(|| false);
    let mut drag_start = use_signal(|| (0.0f64, 0.0f64));

    let state = diagram_state();
    let Some(diagram) = state else {
        return rsx! {
            div {
                class: "er-diagram er-diagram--empty",
                div {
                    class: "er-diagram__header",
                    span { class: "er-diagram__title", "ER Diagram" }
                    button {
                        class: "er-diagram__close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div {
                    class: "er-diagram__empty-state",
                    "No tables to display"
                }
            }
        };
    };

    let table_positions = calculate_table_positions(&diagram.tables, &diagram.relationships);
    let relationship_lines: Vec<ErLine> = diagram
        .relationships
        .iter()
        .filter_map(|rel| {
            let (fx, fy) = *table_positions.get(&rel.from_table)?;
            let (tx, ty) = *table_positions.get(&rel.to_table)?;
            Some(ErLine {
                x1: fx.to_string(),
                y1: fy.to_string(),
                x2: tx.to_string(),
                y2: ty.to_string(),
            })
        })
        .collect();

    rsx! {
        div {
            class: "er-diagram",
            div {
                class: "er-diagram__header",
                span {
                    class: "er-diagram__title",
                    "ER Diagram — {diagram.tables.len()} tables, {diagram.relationships.len()} relationships"
                }
                div {
                    class: "er-diagram__controls",
                    button {
                        class: "er-diagram__zoom-btn",
                        onclick: move |_| zoom.set((zoom() * 1.2).min(3.0)),
                        "+"
                    }
                    button {
                        class: "er-diagram__zoom-btn",
                        onclick: move |_| zoom.set((zoom() / 1.2).max(0.3)),
                        "-"
                    }
                    button {
                        class: "er-diagram__zoom-btn",
                        onclick: move |_| {
                            zoom.set(1.0);
                            view_offset.set((0.0, 0.0));
                        },
                        "Reset"
                    }
                }
                button {
                    class: "er-diagram__close",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            div {
                class: "er-diagram__canvas",
                onmousedown: move |event| {
                    is_dragging.set(true);
                    drag_start.set((event.client_coordinates().x, event.client_coordinates().y));
                },
                onmousemove: move |event| {
                    if is_dragging() {
                        let (start_x, start_y) = drag_start();
                        let delta_x = event.client_coordinates().x - start_x;
                        let delta_y = event.client_coordinates().y - start_y;
                        let (current_x, current_y) = view_offset();
                        view_offset.set((current_x + delta_x, current_y + delta_y));
                        drag_start.set((event.client_coordinates().x, event.client_coordinates().y));
                    }
                },
                onmouseup: move |_| is_dragging.set(false),
                onmouseleave: move |_| is_dragging.set(false),
                onwheel: move |_| {
                    zoom.set((zoom() * 1.05).clamp(0.3, 3.0));
                },
                svg {
                    class: "er-diagram__svg",
                    style: format!(
                        "transform: translate({}px, {}px) scale({});",
                        view_offset().0,
                        view_offset().1,
                        zoom()
                    ),
                    defs {
                        marker {
                            id: "arrowhead",
                            marker_width: "10",
                            marker_height: "7",
                            ref_x: "9",
                            ref_y: "3.5",
                            orient: "auto",
                            polygon {
                                points: "0 0, 10 3.5, 0 7",
                                fill: "var(--color-primary)",
                            }
                        }
                    }
                    for line in relationship_lines.iter() {
                        line {
                            x1: "{line.x1}",
                            y1: "{line.y1}",
                            x2: "{line.x2}",
                            y2: "{line.y2}",
                            stroke: "var(--color-primary)",
                            stroke_width: "2",
                            marker_end: "url(#arrowhead)",
                        }
                    }
                }
                div {
                    class: "er-diagram__tables",
                    for table in diagram.tables.iter() {
                        ErTableCard {
                            table: table.clone(),
                            position: table_positions.get(&table.name).copied(),
                            on_click: on_table_click.clone(),
                        }
                    }
                }
            }
            div {
                class: "er-diagram__legend",
                div {
                    class: "er-diagram__legend-item",
                    span {
                        class: "er-diagram__legend-line",
                    }
                    "Foreign Key"
                }
                div {
                    class: "er-diagram__legend-item",
                    span {
                        class: "er-diagram__legend-pk",
                        "PK"
                    }
                    "Primary Key"
                }
            }
        }
    }
}

#[component]
fn ErTableCard(
    table: ErTable,
    position: Option<(f64, f64)>,
    on_click: Callback<String>,
) -> Element {
    let (x, y) = position.unwrap_or((100.0, 100.0));

    rsx! {
        div {
            class: "er-table-card",
            style: format!("left: {}px; top: {}px;", x, y),
            onclick: move |_| on_click.call(table.name.clone()),
            div {
                class: "er-table-card__header",
                span {
                    class: "er-table-card__name",
                    "{table.name}"
                }
                span {
                    class: "er-table-card__schema",
                    "{table.schema}"
                }
            }
            div {
                class: "er-table-card__columns",
                for column in table.columns.iter() {
                    div {
                        class: "er-table-card__column",
                        span {
                            class: if column.is_primary_key { "er-table-card__pk-badge" } else { "" },
                            if column.is_primary_key { "PK" } else { "" }
                        }
                        span {
                            class: "er-table-card__column-name",
                            "{column.name}"
                        }
                        span {
                            class: "er-table-card__column-type",
                            "{column.data_type}"
                        }
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
fn calculate_table_positions(
    tables: &[ErTable],
    relationships: &[ErRelationship],
) -> HashMap<String, (f64, f64)> {
    let mut positions = HashMap::new();

    if tables.is_empty() {
        return positions;
    }

    let table_width = 220.0;
    let table_height = 200.0;
    let horizontal_gap = 80.0;
    let vertical_gap = 80.0;
    let tables_per_row = 4.max((tables.len() as f64 / 3.0).ceil() as usize);

    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for rel in relationships {
        adjacency
            .entry(rel.from_table.as_str())
            .or_default()
            .push(&rel.to_table);
        adjacency
            .entry(rel.to_table.as_str())
            .or_default()
            .push(&rel.from_table);
    }

    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<&str> = Vec::new();

    if let Some(first) = tables.first() {
        stack.push(&first.name);
    }

    let mut layout_order: Vec<&str> = Vec::new();
    while let Some(current) = stack.pop() {
        if visited.contains(current) {
            continue;
        }
        visited.insert(current);
        layout_order.push(current);

        if let Some(neighbors) = adjacency.get(current) {
            for &neighbor in neighbors.iter().rev() {
                if !visited.contains(neighbor) {
                    stack.push(neighbor);
                }
            }
        }
    }

    for table in tables.iter() {
        if !visited.contains(&table.name.as_str()) {
            layout_order.push(&table.name);
        }
    }

    for (index, table_name) in layout_order.iter().enumerate() {
        let row = index / tables_per_row;
        let col = index % tables_per_row;
        let x = col as f64 * (table_width + horizontal_gap) + 40.0;
        let y = row as f64 * (table_height + vertical_gap) + 40.0;
        positions.insert(table_name.to_string(), (x, y));
    }

    positions
}
