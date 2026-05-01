use dioxus::prelude::*;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChartType {
    Bar,
    Line,
    Pie,
}

impl ChartType {
    fn label(&self) -> &'static str {
        match self {
            ChartType::Bar => "Bar",
            ChartType::Line => "Line",
            ChartType::Pie => "Pie",
        }
    }
}

struct ChartSeries {
    label: String,
    values: Vec<f64>,
}

// ---------------------------------------------------------------------------
// Data helpers
// ---------------------------------------------------------------------------

fn is_numeric(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return false;
    }
    // Strip common formatting: commas, currency symbols, percentage, spaces
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, ',' | '$' | '€' | '£' | '%' | ' '))
        .collect();
    cleaned.parse::<f64>().is_ok()
}

fn parse_numeric(s: &str) -> f64 {
    let cleaned: String = s
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '$' | '€' | '£' | '%' | ' '))
        .collect();
    cleaned.parse::<f64>().unwrap_or(0.0)
}

fn extract_chart_data(columns: &[String], rows: &[Vec<String>]) -> (Vec<String>, Vec<usize>) {
    let mut labels: Vec<String> = Vec::new();
    let mut numeric_col_indices: Vec<usize> = Vec::new();

    if columns.is_empty() || rows.is_empty() {
        return (labels, numeric_col_indices);
    }

    for row in rows {
        labels.push(row.first().cloned().unwrap_or_default());
    }

    for col_idx in 1..columns.len() {
        let numeric_count = rows
            .iter()
            .filter(|row| row.get(col_idx).map(|v| is_numeric(v)).unwrap_or(false))
            .count();
        let non_empty = rows
            .iter()
            .filter(|row| {
                row.get(col_idx)
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
            })
            .count();
        if non_empty > 0 && numeric_count as f64 / non_empty as f64 > 0.3 {
            numeric_col_indices.push(col_idx);
        }
    }

    (labels, numeric_col_indices)
}

fn build_series(columns: &[String], rows: &[Vec<String>], col_idx: usize) -> ChartSeries {
    let label = columns.get(col_idx).cloned().unwrap_or_default();
    let values: Vec<f64> = rows
        .iter()
        .map(|row| row.get(col_idx).map(|v| parse_numeric(v)).unwrap_or(0.0))
        .collect();
    ChartSeries { label, values }
}

fn chart_color(index: usize, total: usize) -> &'static str {
    const COLORS: &[&str] = &[
        "var(--color-primary, #6366f1)",
        "var(--color-info, #3b82f6)",
        "var(--color-success, #22c55e)",
        "var(--color-warning, #f59e0b)",
        "#c9b7ff",
        "#ffbe7b",
    ];
    if total <= 1 {
        return COLORS[0];
    }
    COLORS[index % COLORS.len()]
}

// ---------------------------------------------------------------------------
// Pre-computed tick data for axes
// ---------------------------------------------------------------------------

struct YTick {
    y: f64,
    value: f64,
}

fn compute_y_ticks(
    global_max: f64,
    margin_top: f64,
    plot_height: f64,
    num_ticks: usize,
) -> Vec<YTick> {
    let step = global_max / num_ticks as f64;
    (0..=num_ticks)
        .map(|i| {
            let frac = i as f64 / num_ticks as f64;
            YTick {
                y: margin_top + plot_height - frac * plot_height,
                value: i as f64 * step,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Bar chart
// ---------------------------------------------------------------------------

fn render_bar_chart(
    labels: &[String],
    series_list: &[ChartSeries],
    width: f64,
    height: f64,
) -> Element {
    let margin_left = 50.0;
    let margin_right = 20.0;
    let margin_top = 20.0;
    let margin_bottom = 50.0;

    let plot_width = width - margin_left - margin_right;
    let plot_height = height - margin_top - margin_bottom;

    let global_max = series_list
        .iter()
        .flat_map(|s| s.values.iter().cloned())
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let n = labels.len().max(1);
    let num_series = series_list.len().max(1);
    let group_width = plot_width / n as f64;
    let bar_width = group_width * 0.7 / num_series as f64;
    let bar_gap = group_width * 0.15;

    let y_ticks = compute_y_ticks(global_max, margin_top, plot_height, 5);

    // Pre-compute bar rects
    struct BarRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: &'static str,
    }
    let mut bars: Vec<BarRect> = Vec::new();
    for (series_idx, series) in series_list.iter().enumerate() {
        let fill = chart_color(series_idx, num_series);
        for (i, value) in series.values.iter().enumerate() {
            let bx = margin_left + i as f64 * group_width + bar_gap + series_idx as f64 * bar_width;
            let bar_h = (*value / global_max) * plot_height;
            let by = margin_top + plot_height - bar_h;
            bars.push(BarRect {
                x: bx,
                y: by,
                w: bar_width,
                h: bar_h,
                fill,
            });
        }
    }

    // Pre-compute x labels
    struct XLabel {
        x: f64,
        text: String,
    }
    let x_labels: Vec<XLabel> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let lx = margin_left + i as f64 * group_width + group_width / 2.0;
            let text = if label.len() > 12 {
                format!("{}…", &label[..11])
            } else {
                label.clone()
            };
            XLabel { x: lx, text }
        })
        .collect();

    // Legend items
    struct LegendItem {
        y: f64,
        color: &'static str,
        label: String,
    }
    let legend_items: Vec<LegendItem> = if series_list.len() > 1 {
        series_list
            .iter()
            .enumerate()
            .map(|(i, s)| LegendItem {
                y: margin_top + i as f64 * 20.0,
                color: chart_color(i, num_series),
                label: s.label.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    rsx! {
        svg {
            width: "100%",
            height: "{height}",
            view_box: "0 0 {width} {height}",

            // Y axis
            line {
                x1: "{margin_left}",
                y1: "{margin_top}",
                x2: "{margin_left}",
                y2: "{margin_top + plot_height}",
                stroke: "var(--color-border, #333)",
                stroke_width: "1",
            }
            // X axis
            line {
                x1: "{margin_left}",
                y1: "{margin_top + plot_height}",
                x2: "{margin_left + plot_width}",
                y2: "{margin_top + plot_height}",
                stroke: "var(--color-border, #333)",
                stroke_width: "1",
            }

            // Y ticks & gridlines
            for tick in &y_ticks {
                line {
                    x1: "{margin_left - 4.0}",
                    y1: "{tick.y}",
                    x2: "{margin_left}",
                    y2: "{tick.y}",
                    stroke: "var(--color-border, #333)",
                    stroke_width: "1",
                }
                line {
                    x1: "{margin_left}",
                    y1: "{tick.y}",
                    x2: "{margin_left + plot_width}",
                    y2: "{tick.y}",
                    stroke: "var(--color-border, #333)",
                    stroke_width: "0.5",
                    stroke_dasharray: "3,3",
                    opacity: "0.3",
                }
                text {
                    x: "{margin_left - 8.0}",
                    y: "{tick.y + 4.0}",
                    font_size: "10",
                    fill: "var(--color-text-muted, #888)",
                    text_anchor: "end",
                    "{tick.value:.1}",
                }
            }

            // Bars
            for bar in &bars {
                rect {
                    x: "{bar.x}",
                    y: "{bar.y}",
                    width: "{bar.w}",
                    height: "{bar.h}",
                    fill: "{bar.fill}",
                    rx: "3",
                }
            }

            // X labels
            for lbl in &x_labels {
                text {
                    x: "{lbl.x}",
                    y: "{height - 8.0}",
                    font_size: "10",
                    fill: "var(--color-text-muted, #888)",
                    text_anchor: "middle",
                    "{lbl.text}",
                }
            }

            // Legend
            for item in &legend_items {
                rect {
                    x: "{width - 140.0}",
                    y: "{item.y}",
                    width: "12",
                    height: "12",
                    fill: "{item.color}",
                    rx: "2",
                }
                text {
                    x: "{width - 124.0}",
                    y: "{item.y + 10.0}",
                    font_size: "10",
                    fill: "var(--color-text-muted, #888)",
                    "{item.label}",
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Line chart
// ---------------------------------------------------------------------------

fn render_line_chart(
    labels: &[String],
    series_list: &[ChartSeries],
    width: f64,
    height: f64,
) -> Element {
    let margin_left = 50.0;
    let margin_right = 20.0;
    let margin_top = 20.0;
    let margin_bottom = 50.0;

    let plot_width = width - margin_left - margin_right;
    let plot_height = height - margin_top - margin_bottom;

    let global_max = series_list
        .iter()
        .flat_map(|s| s.values.iter().cloned())
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let n = labels.len().max(2);
    let num_series = series_list.len();

    let y_ticks = compute_y_ticks(global_max, margin_top, plot_height, 5);

    // Pre-compute polylines and points
    struct PolylineData {
        points_str: String,
        color: &'static str,
    }
    let polylines: Vec<PolylineData> = series_list
        .iter()
        .enumerate()
        .map(|(series_idx, series)| {
            let color = chart_color(series_idx, num_series.max(1));
            let points_str = series
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let px = margin_left + (i as f64 / (n - 1) as f64) * plot_width;
                    let py = margin_top + plot_height - (*v / global_max) * plot_height;
                    format!("{:.1},{:.1}", px, py)
                })
                .collect::<Vec<_>>()
                .join(" ");
            PolylineData { points_str, color }
        })
        .collect();

    struct PointData {
        cx: f64,
        cy: f64,
        color: &'static str,
    }
    let points: Vec<PointData> = series_list
        .iter()
        .enumerate()
        .flat_map(|(series_idx, series)| {
            let color = chart_color(series_idx, num_series.max(1));
            series.values.iter().enumerate().map(move |(i, v)| {
                let cx = margin_left + (i as f64 / (n - 1) as f64) * plot_width;
                let cy = margin_top + plot_height - (*v / global_max) * plot_height;
                PointData { cx, cy, color }
            })
        })
        .collect();

    // X labels
    struct XLabel {
        x: f64,
        text: String,
    }
    let x_labels: Vec<XLabel> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let lx = margin_left + (i as f64 / (n - 1) as f64) * plot_width;
            let text = if label.len() > 12 {
                format!("{}…", &label[..11])
            } else {
                label.clone()
            };
            XLabel { x: lx, text }
        })
        .collect();

    // Legend
    struct LegendItem {
        y: f64,
        color: &'static str,
        label: String,
    }
    let legend_items: Vec<LegendItem> = if series_list.len() > 1 {
        series_list
            .iter()
            .enumerate()
            .map(|(i, s)| LegendItem {
                y: margin_top + i as f64 * 20.0,
                color: chart_color(i, num_series.max(1)),
                label: s.label.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    rsx! {
        svg {
            width: "100%",
            height: "{height}",
            view_box: "0 0 {width} {height}",

            // Y axis
            line {
                x1: "{margin_left}", y1: "{margin_top}",
                x2: "{margin_left}", y2: "{margin_top + plot_height}",
                stroke: "var(--color-border, #333)", stroke_width: "1",
            }
            // X axis
            line {
                x1: "{margin_left}", y1: "{margin_top + plot_height}",
                x2: "{margin_left + plot_width}", y2: "{margin_top + plot_height}",
                stroke: "var(--color-border, #333)", stroke_width: "1",
            }

            // Y ticks
            for tick in &y_ticks {
                line {
                    x1: "{margin_left - 4.0}", y1: "{tick.y}",
                    x2: "{margin_left}", y2: "{tick.y}",
                    stroke: "var(--color-border, #333)", stroke_width: "1",
                }
                line {
                    x1: "{margin_left}", y1: "{tick.y}",
                    x2: "{margin_left + plot_width}", y2: "{tick.y}",
                    stroke: "var(--color-border, #333)", stroke_width: "0.5",
                    stroke_dasharray: "3,3", opacity: "0.3",
                }
                text {
                    x: "{margin_left - 8.0}", y: "{tick.y + 4.0}",
                    font_size: "10", fill: "var(--color-text-muted, #888)",
                    text_anchor: "end",
                    "{tick.value:.1}",
                }
            }

            // Polylines
            for pl in &polylines {
                polyline {
                    points: "{pl.points_str}",
                    fill: "none",
                    stroke: "{pl.color}",
                    stroke_width: "2",
                    stroke_linejoin: "round",
                    stroke_linecap: "round",
                }
            }

            // Data points
            for pt in &points {
                circle {
                    cx: "{pt.cx}",
                    cy: "{pt.cy}",
                    r: "3.5",
                    fill: "{pt.color}",
                    stroke: "var(--color-panel, #fff)",
                    stroke_width: "1.5",
                }
            }

            // X labels
            for lbl in &x_labels {
                text {
                    x: "{lbl.x}", y: "{height - 8.0}",
                    font_size: "10", fill: "var(--color-text-muted, #888)",
                    text_anchor: "middle",
                    "{lbl.text}",
                }
            }

            // Legend
            for item in &legend_items {
                rect {
                    x: "{width - 140.0}", y: "{item.y}",
                    width: "12", height: "12",
                    fill: "{item.color}", rx: "2",
                }
                text {
                    x: "{width - 124.0}", y: "{item.y + 10.0}",
                    font_size: "10", fill: "var(--color-text-muted, #888)",
                    "{item.label}",
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pie chart
// ---------------------------------------------------------------------------

fn pie_slice_path(cx: f64, cy: f64, r: f64, start_angle_deg: f64, end_angle_deg: f64) -> String {
    let start_rad = start_angle_deg.to_radians();
    let end_rad = end_angle_deg.to_radians();

    let x1 = cx + r * start_rad.cos();
    let y1 = cy + r * start_rad.sin();
    let x2 = cx + r * end_rad.cos();
    let y2 = cy + r * end_rad.sin();

    let large_arc = if (end_angle_deg - start_angle_deg) > 180.0 {
        1
    } else {
        0
    };

    format!(
        "M {:.1} {:.1} L {:.1} {:.1} A {:.1} {:.1} 0 {} 1 {:.1} {:.1} Z",
        cx, cy, x1, y1, r, r, large_arc, x2, y2
    )
}

struct PieSliceData {
    path: String,
    color: &'static str,
}

struct PieLegendItem {
    y: f64,
    color: &'static str,
    text: String,
}

fn render_pie_chart(labels: &[String], series: &ChartSeries, width: f64, height: f64) -> Element {
    let total: f64 = series.values.iter().sum();
    if total <= 0.0 {
        return rsx! {
            div { class: "chart__empty", "No positive values for pie chart." }
        };
    }

    let cx = width * 0.35;
    let cy = height / 2.0;
    let r = (height - 40.0).min(width * 0.35) / 2.0;
    let legend_x = width * 0.68;
    let n = labels.len();

    let mut slices: Vec<PieSliceData> = Vec::new();
    let mut legend_items: Vec<PieLegendItem> = Vec::new();
    let mut current_angle = 0.0_f64;

    for (i, value) in series.values.iter().enumerate() {
        let slice_angle = (*value / total) * 360.0;
        let path = pie_slice_path(cx, cy, r, current_angle, current_angle + slice_angle);
        let color = chart_color(i, n);
        slices.push(PieSliceData { path, color });

        let pct = *value / total * 100.0;
        let display_label = if labels[i].len() > 15 {
            format!("{}…", &labels[i][..14])
        } else {
            labels[i].clone()
        };
        legend_items.push(PieLegendItem {
            y: 20.0 + i as f64 * 22.0,
            color,
            text: format!("{display_label} ({pct:.1}%)"),
        });

        current_angle += slice_angle;
    }

    rsx! {
        svg {
            width: "100%",
            height: "{height}",
            view_box: "0 0 {width} {height}",

            for slice in &slices {
                path {
                    d: "{slice.path}",
                    fill: "{slice.color}",
                    stroke: "var(--color-panel, #fff)",
                    stroke_width: "1.5",
                }
            }

            for item in &legend_items {
                rect {
                    x: "{legend_x}",
                    y: "{item.y}",
                    width: "12",
                    height: "12",
                    fill: "{item.color}",
                    rx: "2",
                }
                text {
                    x: "{legend_x + 18.0}",
                    y: "{item.y + 10.0}",
                    font_size: "10",
                    fill: "var(--color-text-muted, #888)",
                    "{item.text}",
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ResultChart component
// ---------------------------------------------------------------------------

#[component]
pub fn ResultChart(columns: Vec<String>, rows: Vec<Vec<String>>, visible: Signal<bool>) -> Element {
    let mut chart_type = use_signal(|| ChartType::Bar);
    let mut selected_y_column = use_signal(|| 0_usize);

    // Clone data for the memo so the originals stay available for the rsx! block.
    let columns_for_memo = columns.clone();
    let rows_for_memo = rows.clone();

    let (labels, numeric_cols) =
        use_memo(move || extract_chart_data(&columns_for_memo, &rows_for_memo))();

    // Reset selected column if it goes out of bounds.
    let numeric_cols_for_effect = numeric_cols.clone();
    use_effect(move || {
        if !numeric_cols_for_effect.is_empty()
            && selected_y_column() >= numeric_cols_for_effect.len()
        {
            selected_y_column.set(0);
        }
    });

    if !visible() {
        return rsx! {};
    }

    let has_numeric = !numeric_cols.is_empty();
    let chart_width = 800.0_f64;
    let chart_height = 320.0_f64;

    // Pre-build series data so the rsx! block is simple.
    let col_idx = numeric_cols.get(selected_y_column()).copied().unwrap_or(0);

    let all_series: Vec<ChartSeries> = numeric_cols
        .iter()
        .map(|&ci| build_series(&columns, &rows, ci))
        .collect();

    let primary_series = build_series(&columns, &rows, col_idx);

    rsx! {
        div {
            class: "chart",
            div {
                class: "chart__header",
                span {
                    class: "chart__title",
                    "Chart"
                }
                div {
                    class: "chart__controls",
                    // Chart type toggles
                    div {
                        class: "chart__toggle-group",
                        for ct in &[ChartType::Bar, ChartType::Line, ChartType::Pie] {
                            button {
                                class: if chart_type() == *ct {
                                    "button button--ghost button--small button--active"
                                } else {
                                    "button button--ghost button--small"
                                },
                                onclick: move |_| chart_type.set(*ct),
                                "{ct.label()}"
                            }
                        }
                    }
                    // Y column selector (only when there are multiple numeric columns)
                    if has_numeric && numeric_cols.len() > 1 {
                        select {
                            class: "input chart__select",
                            value: "{selected_y_column()}",
                            oninput: move |event| {
                                if let Ok(idx) = event.value().parse::<usize>() {
                                    selected_y_column.set(idx);
                                }
                            },
                            for (i, &ci) in numeric_cols.iter().enumerate() {
                                option {
                                    value: "{i}",
                                    selected: i == selected_y_column(),
                                    "{columns.get(ci).map(|c| c.as_str()).unwrap_or(\"?\")}"
                                }
                            }
                        }
                    }
                }
            }
            div {
                class: "chart__body",
                if !has_numeric {
                    div {
                        class: "chart__empty",
                        "No numeric data for chart"
                    }
                } else {
                    match chart_type() {
                        ChartType::Bar => render_bar_chart(&labels, &all_series, chart_width, chart_height),
                        ChartType::Line => render_line_chart(&labels, &all_series, chart_width, chart_height),
                        ChartType::Pie => render_pie_chart(&labels, &primary_series, chart_width, chart_height),
                    }
                }
            }
        }
    }
}
