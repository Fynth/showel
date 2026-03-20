use models::{
    DatabaseConnection, DatabaseError, ExplorerNode, ExplorerNodeKind, QueryOutput, QueryPage,
    TablePreviewSource,
};

use explorer::load_connection_tree;
use query::load_table_preview_page;

const MAX_CONTEXT_TABLES: usize = 4;
const MAX_CONTEXT_ROWS: usize = 3;

pub async fn build_acp_database_context(
    connection: DatabaseConnection,
    connection_label: String,
) -> Result<String, DatabaseError> {
    let tree = load_connection_tree(connection.clone()).await?;
    let mut lines = vec![
        format!("Active database connection: {connection_label}"),
        "Schema overview:".to_string(),
    ];

    append_tree_summary(&mut lines, &tree, 0);

    let table_sources = collect_table_sources(&tree)
        .into_iter()
        .take(MAX_CONTEXT_TABLES)
        .collect::<Vec<_>>();

    if !table_sources.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Preview rows (first up to {MAX_CONTEXT_ROWS} rows only, never the full table):"
        ));
    }

    for source in table_sources {
        lines.push(format!("- {} [preview only]", source.qualified_name));
        match load_table_preview_page(
            connection.clone(),
            source.clone(),
            MAX_CONTEXT_ROWS as u32,
            0,
            None,
            None,
        )
        .await
        {
            Ok(QueryOutput::Table(page)) => append_page_preview(&mut lines, &page),
            Ok(QueryOutput::AffectedRows(_)) => {
                lines.push("  rows: <non-tabular preview>".to_string());
            }
            Err(err) => {
                lines.push(format!("  preview error: {err:?}"));
            }
        }
    }

    Ok(lines.join("\n"))
}

fn append_page_preview(lines: &mut Vec<String>, page: &QueryPage) {
    if page.columns.is_empty() {
        lines.push("  columns: <none>".to_string());
        return;
    }

    lines.push(format!("  columns: {}", page.columns.join(", ")));
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
            .map(|(column, value)| format!("{column}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("  row: {cells}"));
    }
}

fn append_tree_summary(lines: &mut Vec<String>, nodes: &[ExplorerNode], depth: usize) {
    let indent = "  ".repeat(depth);
    for node in nodes {
        let kind = match node.kind {
            ExplorerNodeKind::Schema => "schema",
            ExplorerNodeKind::Table => "table",
            ExplorerNodeKind::View => "view",
        };
        lines.push(format!("{indent}- {kind}: {}", node.name));
        if !node.children.is_empty() {
            append_tree_summary(lines, &node.children, depth + 1);
        }
    }
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
    use super::append_page_preview;
    use models::QueryPage;

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
}
