use models::{
    DatabaseConnection, DatabaseError, ExplorerNode, ExplorerNodeKind, QueryOutput,
    TablePreviewSource,
};

use crate::{explorer::load_connection_tree, query::load_table_preview_page};

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
        lines.push("Sample rows:".to_string());
    }

    for source in table_sources {
        lines.push(format!("- {}", source.qualified_name));
        match load_table_preview_page(
            connection.clone(),
            source.clone(),
            MAX_CONTEXT_ROWS as u32,
            0,
        )
        .await
        {
            Ok(QueryOutput::Table(page)) => {
                if page.columns.is_empty() {
                    lines.push("  columns: <none>".to_string());
                    continue;
                }

                lines.push(format!("  columns: {}", page.columns.join(", ")));
                if page.rows.is_empty() {
                    lines.push("  rows: <empty>".to_string());
                    continue;
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
