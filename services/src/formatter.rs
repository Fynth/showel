use models::DatabaseKind;
use sqlformat::{Dialect, FormatOptions, Indent, QueryParams};

pub fn format_sql(kind: Option<DatabaseKind>, sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let options = FormatOptions {
        indent: Indent::Spaces(2),
        uppercase: Some(true),
        lines_between_queries: 1,
        joins_as_top_level: true,
        max_inline_block: 40,
        max_inline_arguments: Some(4),
        dialect: match kind {
            Some(DatabaseKind::Postgres) => Dialect::PostgreSql,
            _ => Dialect::Generic,
        },
        ..FormatOptions::default()
    };

    let mut formatted = sqlformat::format(trimmed, &QueryParams::None, &options);
    if !formatted.ends_with('\n') {
        formatted.push('\n');
    }
    formatted
}
