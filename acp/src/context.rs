use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use models::{
    DatabaseConnection, DatabaseError, ExplorerNode, ExplorerNodeKind, QueryOutput, QueryPage,
    TablePreviewSource,
};

use explorer::{describe_table, load_connection_tree};
use query::load_table_preview_page;

const MAX_PREVIEW_CONTEXT_TABLES: usize = 1;
const MAX_CONTEXT_ROWS: usize = 3;
const MAX_CONTEXT_COLUMNS: usize = 8;
const MAX_CONTEXT_META_ITEMS: usize = 6;
const MAX_OBSERVED_VALUE_COLUMNS: usize = 5;
const MAX_OBSERVED_VALUES_PER_COLUMN: usize = 3;
const MAX_INLINE_VALUE_LEN: usize = 48;
const MAX_INLINE_DETAILS_LEN: usize = 240;
const SCHEMA_CACHE_TTL: Duration = Duration::from_secs(90);

struct CachedSchemaContext {
    catalog_signature: String,
    built_at: Instant,
    lines: Vec<String>,
}

fn schema_context_cache() -> &'static Mutex<HashMap<String, CachedSchemaContext>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedSchemaContext>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn build_acp_database_context(
    connection: DatabaseConnection,
    connection_label: String,
    focus_source: Option<TablePreviewSource>,
) -> Result<String, DatabaseError> {
    let tree = load_connection_tree(connection.clone()).await?;
    let all_sources = collect_table_sources(&tree);
    let prioritized_sources = prioritize_table_sources(all_sources, focus_source.clone());
    let preview_sources = prioritized_sources
        .iter()
        .take(MAX_PREVIEW_CONTEXT_TABLES)
        .cloned()
        .collect::<Vec<_>>();

    let mut lines = vec![format!("Active database connection: {connection_label}")];
    append_catalog_summary(&mut lines, &tree);
    if let Some(focus_source) = focus_source.as_ref() {
        lines.push(format!(
            "Active focus relation: {}",
            focus_source.qualified_name
        ));
    }

    let schema_lines = load_or_build_schema_context_lines(
        connection.clone(),
        &connection_label,
        &tree,
        &prioritized_sources,
    )
    .await?;
    if !schema_lines.is_empty() {
        lines.push(String::new());
        lines.extend(schema_lines);
    }

    if !preview_sources.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Focused data previews (up to {MAX_PREVIEW_CONTEXT_TABLES} relation(s); preview rows are never the full table):"
        ));
    }

    for (index, source) in preview_sources.into_iter().enumerate() {
        let _ = index;
        let is_active_focus = focus_source
            .as_ref()
            .is_some_and(|focus| same_source(focus, &source));
        append_relation_preview_profile(&mut lines, connection.clone(), source, is_active_focus)
            .await;
    }

    Ok(lines.join("\n"))
}

pub async fn warm_acp_database_schema_context(
    connection: DatabaseConnection,
    connection_label: String,
) -> Result<(), DatabaseError> {
    let tree = load_connection_tree(connection.clone()).await?;
    let sources = collect_table_sources(&tree);
    let _ =
        load_or_build_schema_context_lines(connection, &connection_label, &tree, &sources).await?;
    Ok(())
}

async fn load_or_build_schema_context_lines(
    connection: DatabaseConnection,
    connection_label: &str,
    tree: &[ExplorerNode],
    sources: &[TablePreviewSource],
) -> Result<Vec<String>, DatabaseError> {
    let catalog_signature = build_catalog_signature(tree);

    if let Ok(cache) = schema_context_cache().lock()
        && let Some(cached) = cache.get(connection_label)
        && cached.catalog_signature == catalog_signature
        && cached.built_at.elapsed() <= SCHEMA_CACHE_TTL
    {
        return Ok(cached.lines.clone());
    }

    let mut lines = Vec::new();
    if !sources.is_empty() {
        lines.push(format!(
            "Full relation schema map (all {} relation(s) currently visible in the database catalog):",
            sources.len()
        ));
    }

    for source in sources {
        append_relation_schema_profile(&mut lines, connection.clone(), source.clone()).await;
    }

    if let Ok(mut cache) = schema_context_cache().lock() {
        cache.insert(
            connection_label.to_string(),
            CachedSchemaContext {
                catalog_signature,
                built_at: Instant::now(),
                lines: lines.clone(),
            },
        );
    }

    Ok(lines)
}

async fn append_relation_schema_profile(
    lines: &mut Vec<String>,
    connection: DatabaseConnection,
    source: TablePreviewSource,
) {
    lines.push(format!("- {}", source.qualified_name));

    match describe_table(
        connection.clone(),
        source.schema.clone(),
        source.table_name.clone(),
    )
    .await
    {
        Ok(QueryOutput::Table(page)) => append_structure_profile(lines, &page, true),
        Ok(QueryOutput::AffectedRows(_)) => {
            lines.push("  structure: <non-tabular response>".to_string());
        }
        Err(err) => {
            lines.push(format!("  structure error: {err:?}"));
        }
    }
}

async fn append_relation_preview_profile(
    lines: &mut Vec<String>,
    connection: DatabaseConnection,
    source: TablePreviewSource,
    is_active_focus: bool,
) {
    lines.push(relation_heading(&source, is_active_focus));

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

fn relation_heading(source: &TablePreviewSource, is_active_focus: bool) -> String {
    if is_active_focus {
        format!("- {} [active focus]", source.qualified_name)
    } else {
        format!("- {}", source.qualified_name)
    }
}

fn build_catalog_signature(nodes: &[ExplorerNode]) -> String {
    let mut signature = String::new();
    append_catalog_signature_parts(nodes, &mut signature);
    signature
}

fn append_catalog_signature_parts(nodes: &[ExplorerNode], signature: &mut String) {
    for node in nodes {
        signature.push_str(match node.kind {
            ExplorerNodeKind::Schema => "schema:",
            ExplorerNodeKind::Table => "table:",
            ExplorerNodeKind::View => "view:",
        });
        signature.push_str(&node.qualified_name);
        signature.push('|');
        append_catalog_signature_parts(&node.children, signature);
    }
}

fn append_structure_profile(lines: &mut Vec<String>, page: &QueryPage, include_all_items: bool) {
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
        append_limited_items(
            lines,
            &table_meta,
            item_limit(table_meta.len(), include_all_items, MAX_CONTEXT_META_ITEMS),
        );
    }

    if !columns.is_empty() {
        lines.push("  columns:".to_string());
        append_limited_items(
            lines,
            &columns,
            item_limit(columns.len(), include_all_items, MAX_CONTEXT_COLUMNS),
        );
    }

    if !other_meta.is_empty() {
        lines.push("  schema details:".to_string());
        append_limited_items(
            lines,
            &other_meta,
            item_limit(other_meta.len(), include_all_items, MAX_CONTEXT_META_ITEMS),
        );
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
                    .map(|child| child.name.clone())
                    .collect::<Vec<_>>();
                let mut summary = format!(
                    "- schema {}: {} table(s), {} view(s)",
                    node.name, table_count, view_count
                );
                if !relation_names.is_empty() {
                    summary.push_str(&format!(" -> {}", relation_names.join(", ")));
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

fn item_limit(item_count: usize, include_all_items: bool, default_limit: usize) -> usize {
    if include_all_items {
        item_count
    } else {
        default_limit
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
        append_catalog_summary, append_observed_values, append_page_preview,
        append_structure_profile, build_catalog_signature, inline_excerpt,
        prioritize_table_sources, relation_heading,
    };
    use models::{ExplorerNode, ExplorerNodeKind, QueryPage, TablePreviewSource};

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
    fn catalog_summary_lists_all_relation_names() {
        let nodes = vec![ExplorerNode {
            name: "main".to_string(),
            kind: ExplorerNodeKind::Schema,
            schema: Some("main".to_string()),
            qualified_name: "main".to_string(),
            children: (1..=10)
                .map(|index| ExplorerNode {
                    name: format!("table_{index}"),
                    kind: ExplorerNodeKind::Table,
                    schema: Some("main".to_string()),
                    qualified_name: format!("main.table_{index}"),
                    children: Vec::new(),
                })
                .collect(),
        }];

        let mut lines = Vec::new();
        append_catalog_summary(&mut lines, &nodes);
        let summary = lines.join("\n");

        assert!(summary.contains("table_1"));
        assert!(summary.contains("table_10"));
        assert!(!summary.contains("+"));
    }

    #[test]
    fn inline_excerpt_flattens_multiline_content() {
        let excerpt = inline_excerpt("CREATE TABLE products\n(\n  id INTEGER\n)", 80);
        assert_eq!(excerpt, "CREATE TABLE products ( id INTEGER )");
    }

    #[test]
    fn structure_profile_can_include_all_columns_for_full_schema_map() {
        let page = QueryPage {
            columns: vec![
                "section".to_string(),
                "name".to_string(),
                "type".to_string(),
                "target".to_string(),
                "details".to_string(),
            ],
            rows: (1..=10)
                .map(|index| {
                    vec![
                        "column".to_string(),
                        format!("col_{index}"),
                        "text".to_string(),
                        String::new(),
                        String::new(),
                    ]
                })
                .collect(),
            editable: None,
            offset: 0,
            page_size: 0,
            has_previous: false,
            has_next: false,
        };

        let mut lines = Vec::new();
        append_structure_profile(&mut lines, &page, true);
        let summary = lines.join("\n");

        assert!(summary.contains("col_1"));
        assert!(summary.contains("col_10"));
        assert!(!summary.contains("..."));
    }

    #[test]
    fn relation_heading_marks_active_focus() {
        let source = TablePreviewSource {
            schema: Some("main".to_string()),
            table_name: "products".to_string(),
            qualified_name: "\"main\".\"products\"".to_string(),
        };

        assert_eq!(
            relation_heading(&source, true),
            "- \"main\".\"products\" [active focus]"
        );
    }

    #[test]
    fn catalog_signature_changes_when_relation_names_change() {
        let nodes = vec![ExplorerNode {
            name: "main".to_string(),
            kind: ExplorerNodeKind::Schema,
            schema: Some("main".to_string()),
            qualified_name: "main".to_string(),
            children: vec![ExplorerNode {
                name: "products".to_string(),
                kind: ExplorerNodeKind::Table,
                schema: Some("main".to_string()),
                qualified_name: "main.products".to_string(),
                children: Vec::new(),
            }],
        }];
        let mut updated = nodes.clone();
        updated[0].children[0].qualified_name = "main.orders".to_string();
        updated[0].children[0].name = "orders".to_string();

        assert_ne!(
            build_catalog_signature(&nodes),
            build_catalog_signature(&updated)
        );
    }
}
