use models::{
    DatabaseConnection, DatabaseError, ExplorerNode, ExplorerNodeKind, QueryOutput, QueryPage,
    TablePreviewSource,
};

use explorer::{describe_table, load_connection_tree};
use query::{execute_query, load_table_preview_page};

const MAX_CONTEXT_TABLES: usize = 4;
const MAX_CONTEXT_ROWS: usize = 3;
const MAX_CONTEXT_COLUMNS: usize = 8;
const MAX_CONTEXT_META_ITEMS: usize = 6;
const MAX_OBSERVED_VALUE_COLUMNS: usize = 5;
const MAX_OBSERVED_VALUES_PER_COLUMN: usize = 3;
const MAX_COUNTED_TABLES: usize = 2;
const MAX_CATALOG_RELATION_NAMES: usize = 8;
const MAX_INLINE_VALUE_LEN: usize = 48;
const MAX_INLINE_DETAILS_LEN: usize = 240;

pub async fn build_acp_database_context(
    connection: DatabaseConnection,
    connection_label: String,
    focus_source: Option<TablePreviewSource>,
) -> Result<String, DatabaseError> {
    let tree = load_connection_tree(connection.clone()).await?;
    let all_sources = collect_table_sources(&tree);
    let prioritized_sources = prioritize_table_sources(all_sources, focus_source.clone());
    let profiled_sources = prioritized_sources
        .into_iter()
        .take(MAX_CONTEXT_TABLES)
        .collect::<Vec<_>>();

    let mut lines = vec![format!("Active database connection: {connection_label}")];
    append_catalog_summary(&mut lines, &tree);

    if !profiled_sources.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Deep table profiles (up to {MAX_CONTEXT_TABLES} relations, preview rows are never the full table):"
        ));
    }

    for (index, source) in profiled_sources.into_iter().enumerate() {
        append_table_profile(
            &mut lines,
            connection.clone(),
            source,
            focus_source.as_ref(),
            index < MAX_COUNTED_TABLES,
        )
        .await;
    }

    Ok(lines.join("\n"))
}

async fn append_table_profile(
    lines: &mut Vec<String>,
    connection: DatabaseConnection,
    source: TablePreviewSource,
    focus_source: Option<&TablePreviewSource>,
    include_row_count: bool,
) {
    let focus_label = if focus_source.is_some_and(|focus| same_source(focus, &source)) {
        " [active focus]"
    } else {
        ""
    };
    lines.push(format!("- {}{focus_label}", source.qualified_name));

    if include_row_count {
        match load_row_count_summary(connection.clone(), &source).await {
            Ok(Some(summary)) => lines.push(format!("  row count: {summary}")),
            Ok(None) => {}
            Err(err) => lines.push(format!("  row count error: {err:?}")),
        }
    }

    match describe_table(
        connection.clone(),
        source.schema.clone(),
        source.table_name.clone(),
    )
    .await
    {
        Ok(QueryOutput::Table(page)) => append_structure_profile(lines, &page),
        Ok(QueryOutput::AffectedRows(_)) => {
            lines.push("  structure: <non-tabular response>".to_string());
        }
        Err(err) => {
            lines.push(format!("  structure error: {err:?}"));
        }
    }

    match load_table_preview_page(connection, source, MAX_CONTEXT_ROWS as u32, 0, None, None).await
    {
        Ok(QueryOutput::Table(page)) => {
            append_page_preview(lines, &page);
            append_observed_values(lines, &page);
        }
        Ok(QueryOutput::AffectedRows(_)) => {
            lines.push("  preview: <non-tabular response>".to_string());
        }
        Err(err) => {
            lines.push(format!("  preview error: {err:?}"));
        }
    }
}

async fn load_row_count_summary(
    connection: DatabaseConnection,
    source: &TablePreviewSource,
) -> Result<Option<String>, DatabaseError> {
    let sql = format!(
        "select count(*) as row_count from {}",
        source.qualified_name
    );
    match execute_query(connection, sql).await? {
        QueryOutput::Table(page) => {
            Ok(first_cell(&page).map(|value| format!("{value} via COUNT(*)")))
        }
        QueryOutput::AffectedRows(_) => Ok(None),
    }
}

fn first_cell(page: &QueryPage) -> Option<String> {
    page.rows
        .first()
        .and_then(|row| row.first())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn append_structure_profile(lines: &mut Vec<String>, page: &QueryPage) {
    let mut definition = None::<String>;
    let mut table_meta = Vec::new();
    let mut columns = Vec::new();
    let mut other_meta = Vec::new();

    for row in &page.rows {
        let section = row.first().map(String::as_str).unwrap_or_default();
        let name = row.get(1).map(String::as_str).unwrap_or_default();
        let row_type = row.get(2).map(String::as_str).unwrap_or_default();
        let target = row.get(3).map(String::as_str).unwrap_or_default();
        let details = row.get(4).map(String::as_str).unwrap_or_default();

        match section {
            "table" if row_type.eq_ignore_ascii_case("definition") => {
                definition = Some(inline_excerpt(details, MAX_INLINE_DETAILS_LEN));
            }
            "table" => {
                table_meta.push(format_structure_item(name, row_type, target, details));
            }
            "column" => {
                columns.push(format_structure_item(name, row_type, target, details));
            }
            _ => {
                other_meta.push(format!(
                    "{section} {}",
                    format_structure_item(name, row_type, target, details)
                ));
            }
        }
    }

    if let Some(definition) = definition {
        lines.push(format!("  definition: {definition}"));
    }

    if !table_meta.is_empty() {
        lines.push("  relation details:".to_string());
        append_limited_items(lines, &table_meta, MAX_CONTEXT_META_ITEMS);
    }

    if !columns.is_empty() {
        lines.push("  columns:".to_string());
        append_limited_items(lines, &columns, MAX_CONTEXT_COLUMNS);
    }

    if !other_meta.is_empty() {
        lines.push("  schema details:".to_string());
        append_limited_items(lines, &other_meta, MAX_CONTEXT_META_ITEMS);
    }
}

fn append_page_preview(lines: &mut Vec<String>, page: &QueryPage) {
    if page.columns.is_empty() {
        lines.push("  columns: <none>".to_string());
        return;
    }

    lines.push(format!("  preview columns: {}", page.columns.join(", ")));
    if page.rows.is_empty() {
        lines.push("  preview: <empty>".to_string());
        return;
    }

    if page.has_next || page.offset > 0 {
        lines.push(format!(
            "  preview: showing {} row(s) from offset {} only; do not treat this as the full table",
            page.rows.len(),
            page.offset
        ));
    } else {
        lines.push(format!(
            "  preview: showing {} row(s); totals are unknown unless counted explicitly",
            page.rows.len()
        ));
    }

    for row in page.rows.iter().take(MAX_CONTEXT_ROWS) {
        let cells = page
            .columns
            .iter()
            .zip(row.iter())
            .map(|(column, value)| {
                format!("{column}={}", inline_excerpt(value, MAX_INLINE_VALUE_LEN))
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("  row: {cells}"));
    }
}

fn append_observed_values(lines: &mut Vec<String>, page: &QueryPage) {
    if page.columns.is_empty() || page.rows.is_empty() {
        return;
    }

    let mut observed = Vec::new();
    for (column_index, column_name) in page
        .columns
        .iter()
        .enumerate()
        .take(MAX_OBSERVED_VALUE_COLUMNS)
    {
        let mut values = Vec::<String>::new();
        for row in page.rows.iter().take(MAX_CONTEXT_ROWS) {
            let Some(value) = row.get(column_index) else {
                continue;
            };
            let excerpt = inline_excerpt(value, MAX_INLINE_VALUE_LEN);
            if excerpt.is_empty() || values.iter().any(|existing| existing == &excerpt) {
                continue;
            }
            values.push(excerpt);
            if values.len() >= MAX_OBSERVED_VALUES_PER_COLUMN {
                break;
            }
        }
        if !values.is_empty() {
            observed.push(format!("{column_name} = {}", values.join(" | ")));
        }
    }

    if observed.is_empty() {
        return;
    }

    lines.push("  observed values:".to_string());
    append_limited_items(lines, &observed, MAX_OBSERVED_VALUE_COLUMNS);
}

fn append_catalog_summary(lines: &mut Vec<String>, nodes: &[ExplorerNode]) {
    let schema_count = nodes
        .iter()
        .filter(|node| matches!(node.kind, ExplorerNodeKind::Schema))
        .count();
    let relation_count = nodes.iter().map(count_relations).sum::<usize>();
    lines.push(format!(
        "Catalog summary: {schema_count} schema(s), {relation_count} relation(s)."
    ));
    lines.push("Schema overview:".to_string());

    if nodes.is_empty() {
        lines.push("- <empty catalog>".to_string());
        return;
    }

    for node in nodes {
        match node.kind {
            ExplorerNodeKind::Schema => {
                let table_count = node
                    .children
                    .iter()
                    .filter(|child| matches!(child.kind, ExplorerNodeKind::Table))
                    .count();
                let view_count = node
                    .children
                    .iter()
                    .filter(|child| matches!(child.kind, ExplorerNodeKind::View))
                    .count();
                let relation_names = node
                    .children
                    .iter()
                    .take(MAX_CATALOG_RELATION_NAMES)
                    .map(|child| child.name.clone())
                    .collect::<Vec<_>>();
                let overflow = node.children.len().saturating_sub(relation_names.len());
                let mut summary = format!(
                    "- schema {}: {} table(s), {} view(s)",
                    node.name, table_count, view_count
                );
                if !relation_names.is_empty() {
                    summary.push_str(&format!(" -> {}", relation_names.join(", ")));
                }
                if overflow > 0 {
                    summary.push_str(&format!(", +{overflow} more"));
                }
                lines.push(summary);
            }
            ExplorerNodeKind::Table | ExplorerNodeKind::View => {
                let kind = match node.kind {
                    ExplorerNodeKind::Table => "table",
                    ExplorerNodeKind::View => "view",
                    ExplorerNodeKind::Schema => unreachable!(),
                };
                lines.push(format!("- {kind}: {}", node.qualified_name));
            }
        }
    }
}

fn count_relations(node: &ExplorerNode) -> usize {
    match node.kind {
        ExplorerNodeKind::Schema => node.children.iter().map(count_relations).sum(),
        ExplorerNodeKind::Table | ExplorerNodeKind::View => 1,
    }
}

fn append_limited_items(lines: &mut Vec<String>, items: &[String], limit: usize) {
    for item in items.iter().take(limit) {
        lines.push(format!("    - {item}"));
    }
    let overflow = items.len().saturating_sub(limit);
    if overflow > 0 {
        lines.push(format!("    - ... {overflow} more"));
    }
}

fn format_structure_item(name: &str, row_type: &str, target: &str, details: &str) -> String {
    let mut parts = Vec::new();
    if !row_type.trim().is_empty() {
        parts.push(inline_excerpt(row_type, MAX_INLINE_VALUE_LEN));
    }
    if !target.trim().is_empty() {
        parts.push(inline_excerpt(target, MAX_INLINE_VALUE_LEN));
    }
    if !details.trim().is_empty() {
        parts.push(inline_excerpt(details, MAX_INLINE_DETAILS_LEN));
    }

    if parts.is_empty() {
        name.to_string()
    } else if name.trim().is_empty() {
        parts.join(" · ")
    } else {
        format!("{name}: {}", parts.join(" · "))
    }
}

fn inline_excerpt(value: &str, max_len: usize) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = single_line.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }
    let clipped = trimmed.chars().take(max_len).collect::<String>();
    format!("{clipped}...")
}

fn prioritize_table_sources(
    sources: Vec<TablePreviewSource>,
    focus_source: Option<TablePreviewSource>,
) -> Vec<TablePreviewSource> {
    let mut prioritized = Vec::new();
    if let Some(focus_source) = focus_source {
        prioritized.push(focus_source);
    }

    for source in sources {
        if prioritized
            .iter()
            .any(|existing| same_source(existing, &source))
        {
            continue;
        }
        prioritized.push(source);
    }

    prioritized
}

fn same_source(left: &TablePreviewSource, right: &TablePreviewSource) -> bool {
    left.qualified_name == right.qualified_name
}

fn collect_table_sources(nodes: &[ExplorerNode]) -> Vec<TablePreviewSource> {
    let mut sources = Vec::new();
    collect_table_sources_inner(nodes, &mut sources);
    sources
}

fn collect_table_sources_inner(nodes: &[ExplorerNode], sources: &mut Vec<TablePreviewSource>) {
    for node in nodes {
        match node.kind {
            ExplorerNodeKind::Table | ExplorerNodeKind::View => sources.push(TablePreviewSource {
                schema: node.schema.clone(),
                table_name: node.name.clone(),
                qualified_name: node.qualified_name.clone(),
            }),
            ExplorerNodeKind::Schema => collect_table_sources_inner(&node.children, sources),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        append_observed_values, append_page_preview, inline_excerpt, prioritize_table_sources,
    };
    use models::{QueryPage, TablePreviewSource};

    #[test]
    fn page_preview_marks_partial_data_as_preview_only() {
        let page = QueryPage {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "Wireless Mouse".to_string()],
                vec!["2".to_string(), "Mechanical Keyboard".to_string()],
                vec!["3".to_string(), "USB-C Hub".to_string()],
            ],
            editable: None,
            offset: 0,
            page_size: 3,
            has_previous: false,
            has_next: true,
        };

        let mut lines = Vec::new();
        append_page_preview(&mut lines, &page);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("do not treat this as the full table")),
            "expected explicit preview-only wording, got: {lines:?}"
        );
    }

    #[test]
    fn page_preview_marks_totals_unknown_without_count() {
        let page = QueryPage {
            columns: vec!["id".to_string()],
            rows: vec![vec!["1".to_string()]],
            editable: None,
            offset: 0,
            page_size: 10,
            has_previous: false,
            has_next: false,
        };

        let mut lines = Vec::new();
        append_page_preview(&mut lines, &page);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("totals are unknown unless counted explicitly")),
            "expected totals warning, got: {lines:?}"
        );
    }

    #[test]
    fn observed_values_collect_distinct_column_examples() {
        let page = QueryPage {
            columns: vec!["category".to_string(), "price".to_string()],
            rows: vec![
                vec!["Electronics".to_string(), "29.99".to_string()],
                vec!["Electronics".to_string(), "89.99".to_string()],
                vec!["Office".to_string(), "89.99".to_string()],
            ],
            editable: None,
            offset: 0,
            page_size: 3,
            has_previous: false,
            has_next: false,
        };

        let mut lines = Vec::new();
        append_observed_values(&mut lines, &page);

        assert!(
            lines
                .iter()
                .any(|line| line.contains("category = Electronics | Office"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("price = 29.99 | 89.99"))
        );
    }

    #[test]
    fn focus_source_moves_to_front_without_duplicates() {
        let sources = vec![
            TablePreviewSource {
                schema: Some("main".to_string()),
                table_name: "orders".to_string(),
                qualified_name: "\"orders\"".to_string(),
            },
            TablePreviewSource {
                schema: Some("main".to_string()),
                table_name: "products".to_string(),
                qualified_name: "\"products\"".to_string(),
            },
        ];
        let focus = Some(TablePreviewSource {
            schema: Some("main".to_string()),
            table_name: "products".to_string(),
            qualified_name: "\"products\"".to_string(),
        });

        let prioritized = prioritize_table_sources(sources, focus);

        assert_eq!(prioritized[0].table_name, "products");
        assert_eq!(prioritized.len(), 2);
    }

    #[test]
    fn inline_excerpt_flattens_multiline_content() {
        let excerpt = inline_excerpt("CREATE TABLE products\n(\n  id INTEGER\n)", 80);
        assert_eq!(excerpt, "CREATE TABLE products ( id INTEGER )");
    }
}
