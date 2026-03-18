use models::{DatabaseKind, SqlFormatSettings, SqlKeywordCase};
use sqlformat::{Dialect, FormatOptions, Indent, QueryParams};

pub fn format_sql(kind: Option<DatabaseKind>, sql: &str, settings: &SqlFormatSettings) -> String {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let wrap_width = settings.max_inline_block.max(20) as usize;

    let options = FormatOptions {
        indent: Indent::Spaces(settings.indent_width.max(1)),
        uppercase: match settings.keyword_case {
            SqlKeywordCase::Preserve => None,
            SqlKeywordCase::Uppercase => Some(true),
            SqlKeywordCase::Lowercase => Some(false),
        },
        lines_between_queries: settings.lines_between_queries,
        inline: settings.inline,
        joins_as_top_level: settings.joins_as_top_level,
        max_inline_block: wrap_width,
        max_inline_arguments: settings.max_inline_arguments.map(|value| value as usize),
        max_inline_top_level: Some(
            settings
                .max_inline_top_level
                .unwrap_or(settings.max_inline_block)
                .max(20) as usize,
        ),
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
