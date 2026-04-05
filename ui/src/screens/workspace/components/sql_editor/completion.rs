/// Pure SQL completion engine — no Dioxus, async, or side-effect dependencies.
/// Provides context-aware completions for the `SqlEditor` component.

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Table,
    View,
    Column,
    Keyword,
    Function,
    Schema,
    Alias,
}

impl CompletionKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Table => "T",
            Self::View => "V",
            Self::Column => "C",
            Self::Keyword => "K",
            Self::Function => "F",
            Self::Schema => "S",
            Self::Alias => "A",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub insert_text: String,
}

#[derive(Clone, Debug, Default)]
pub struct TableMeta {
    pub schema: Option<String>,
    pub name: String,
    pub qualified_name: String,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SchemaMetadata {
    pub tables: Vec<TableMeta>,
    pub schemas: Vec<String>,
}

// ---------------------------------------------------------------------------
// SQL Keywords and Functions
// ---------------------------------------------------------------------------

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "FULL",
    "CROSS",
    "ON",
    "AND",
    "OR",
    "NOT",
    "IN",
    "IS",
    "NULL",
    "LIKE",
    "BETWEEN",
    "EXISTS",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "TABLE",
    "DROP",
    "ALTER",
    "ADD",
    "COLUMN",
    "INDEX",
    "VIEW",
    "AS",
    "ORDER",
    "BY",
    "ASC",
    "DESC",
    "GROUP",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "UNION",
    "ALL",
    "DISTINCT",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "CAST",
    "COALESCE",
    "TRUE",
    "FALSE",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "DEFAULT",
    "CONSTRAINT",
    "UNIQUE",
    "CHECK",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "TRUNCATE",
    "EXPLAIN",
    "ANALYZE",
    "IF",
    "INTEGER",
    "TEXT",
    "VARCHAR",
    "BOOLEAN",
    "TIMESTAMP",
    "DATE",
    "FLOAT",
    "NUMERIC",
    "BIGINT",
    "SMALLINT",
    "CHAR",
    "BLOB",
    "REAL",
    "DOUBLE",
    "BYTEA",
    "SERIAL",
    "BIGSERIAL",
    "UUID",
    "NATURAL",
    "USING",
    "RETURNING",
    "WITH",
    "RECURSIVE",
    "OVER",
    "PARTITION",
    "ROWS",
    "RANGE",
    "UNBOUNDED",
    "PRECEDING",
    "FOLLOWING",
    "CURRENT",
    "ROW",
    "FETCH",
    "NEXT",
    "ONLY",
    "FIRST",
    "LAST",
    "ILIKE",
    "SIMILAR",
    "TO",
    "ESCAPE",
    "ANY",
    "SOME",
    "LATERAL",
    "TABLESAMPLE",
    "ORDINALITY",
    "MATERIALIZED",
    "CONCURRENTLY",
    "TEMP",
    "TEMPORARY",
    "UNLOGGED",
    "LOGGED",
    "REPLACE",
    "CONFLICT",
    "NOTHING",
    "GRANT",
    "REVOKE",
    "PRIVILEGES",
    "PUBLIC",
    "DATABASE",
    "SCHEMA",
    "SEQUENCE",
    "FUNCTION",
    "PROCEDURE",
    "TRIGGER",
    "RULE",
    "EXTENSION",
    "TYPE",
    "DOMAIN",
    "ENUM",
    "COMMENT",
    "OWN",
    "OWNER",
    "TABLESPACE",
    "CLUSTER",
    "VACUUM",
    "REINDEX",
    "DISCARD",
    "RESET",
    "LISTEN",
    "NOTIFY",
    "UNLISTEN",
    "LOCK",
    "ACCESS",
    "SHARE",
    "EXCLUSIVE",
    "MODE",
    "NOWAIT",
    "SKIP",
    "LOCKED",
    "NULLS",
    "WINDOW",
];

const SQL_FUNCTIONS: &[&str] = &[
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "COALESCE",
    "NULLIF",
    "CAST",
    "CONCAT",
    "LENGTH",
    "LOWER",
    "UPPER",
    "TRIM",
    "SUBSTRING",
    "REPLACE",
    "DATE",
    "NOW",
    "EXTRACT",
    "ROUND",
    "FLOOR",
    "CEIL",
    "ABS",
    "POWER",
    "SQRT",
    "RANDOM",
    "POSITION",
    "CHAR_LENGTH",
    "BIT_LENGTH",
    "OCTET_LENGTH",
    "INITCAP",
    "LEFT",
    "RIGHT",
    "LPAD",
    "RPAD",
    "REPEAT",
    "REVERSE",
    "LTRIM",
    "RTRIM",
    "SPLIT_PART",
    "TO_CHAR",
    "TO_DATE",
    "TO_TIMESTAMP",
    "DATE_TRUNC",
    "AGE",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "ARRAY_AGG",
    "STRING_AGG",
    "BOOL_AND",
    "BOOL_OR",
    "BIT_AND",
    "BIT_OR",
    "EVERY",
    "RANK",
    "DENSE_RANK",
    "ROW_NUMBER",
    "LEAD",
    "LAG",
    "FIRST_VALUE",
    "LAST_VALUE",
    "NTH_VALUE",
    "NTILE",
    "FORMAT",
    "QUOTE_IDENT",
    "QUOTE_LITERAL",
    "MD5",
    "ENCODE",
    "DECODE",
    "TO_NUMBER",
    "TO_HEX",
    "ISFINITE",
    "MAKE_DATE",
    "MAKE_TIME",
    "MAKE_TIMESTAMP",
    "MAKE_INTERVAL",
    "CLOCK_TIMESTAMP",
    "STATEMENT_TIMESTAMP",
    "TRANSACTION_TIMESTAMP",
    "GENERATE_SERIES",
    "GENERATE_SUBSCRIPTS",
    "UNNEST",
    "JSON_ARRAY_ELEMENTS",
    "JSON_ARRAY_ELEMENTS_TEXT",
    "JSON_EACH",
    "JSON_EACH_TEXT",
    "JSON_OBJECT_KEYS",
    "JSONB_ARRAY_ELEMENTS",
    "JSONB_ARRAY_ELEMENTS_TEXT",
    "JSONB_EACH",
    "JSONB_EACH_TEXT",
    "JSONB_OBJECT_KEYS",
    "JSONB_PRETTY",
    "JSONB_SET",
    "JSONB_INSERT",
    "JSONB_STRIP_NULLS",
    "TO_JSONB",
    "ROW_TO_JSON",
    "OVERLAY",
    "REGEXP_REPLACE",
    "REGEXP_MATCHES",
    "REGEXP_SPLIT_TO_TABLE",
    "REGEXP_SPLIT_TO_ARRAY",
];

// ---------------------------------------------------------------------------
// SQL context enum
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum SqlContext {
    SelectClause,
    FromClause,
    WhereClause,
    OrderByClause,
    GroupByClause,
    HavingClause,
    InsertInto,
    UpdateTable,
    UpdateSet,
    DotAccess,
    Default,
    DeleteFrom,
    JoinOn,
    ValuesClause,
}

// ---------------------------------------------------------------------------
// Helpers — boundary chars
// ---------------------------------------------------------------------------

const BOUNDARY_CHARS: &[char] = &[
    ' ', '\t', '\n', '\r', '(', ')', ',', ';', '+', '-', '*', '/', '=', '<', '>', ':', '[', ']',
    '{', '}', '!', '&', '|', '^', '~', '%', '#', '@', '`', '?',
];

#[inline]
fn is_boundary_char(ch: char) -> bool {
    BOUNDARY_CHARS.contains(&ch)
}

// ---------------------------------------------------------------------------
// Helpers — non-code region detection
// ---------------------------------------------------------------------------

/// Returns `true` if `pos` falls inside a single-quoted string literal.
fn is_in_string_literal(text: &str, pos: usize) -> bool {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < pos {
        if bytes[i] == b'\'' {
            // escaped '' → skip both bytes
            if i + 1 < pos && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
            }
            in_string = !in_string;
        }
        i += 1;
    }
    in_string
}

/// Returns `true` if `pos` falls inside a `--` line comment.
fn is_in_line_comment(text: &str, pos: usize) -> bool {
    let line_start = text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_before = &text[line_start..pos];
    let mut in_str = false;
    let mut prev_dash = false;
    for ch in line_before.chars() {
        if ch == '\'' {
            in_str = !in_str;
            prev_dash = false;
            continue;
        }
        if !in_str && ch == '-' {
            if prev_dash {
                return true;
            }
            prev_dash = true;
        } else {
            prev_dash = ch == '-';
        }
    }
    false
}

/// Returns `true` if `pos` falls inside a `/* … */` block comment.
fn is_in_block_comment(text: &str, pos: usize) -> bool {
    let bytes = text.as_bytes();
    let mut in_comment = false;
    let mut i = 0;
    while i + 1 < pos {
        if !in_comment && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            in_comment = true;
            i += 2;
            continue;
        }
        if in_comment && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
            continue;
        }
        i += 1;
    }
    in_comment
}

fn is_in_non_code_region(text: &str, pos: usize) -> bool {
    is_in_string_literal(text, pos)
        || is_in_line_comment(text, pos)
        || is_in_block_comment(text, pos)
}

// ---------------------------------------------------------------------------
// Partial token extraction
// ---------------------------------------------------------------------------

/// Extract the word (including interior dots) being typed immediately before
/// `pos`.  Returns `(partial_token, start_byte_offset)`.
fn extract_current_token(text: &str, pos: usize) -> (String, usize) {
    if pos == 0 {
        return (String::new(), 0);
    }
    let mut start = pos;
    for ch in text[..pos].chars().rev() {
        if is_boundary_char(ch) {
            break;
        }
        start -= ch.len_utf8();
    }
    if start == pos {
        return (String::new(), pos);
    }
    (text[start..pos].to_string(), start)
}

// ---------------------------------------------------------------------------
// Lightweight token type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SqlToken {
    text: String,     // upper-cased
    original: String, // original case
    is_keyword: bool,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

fn build_keyword_set() -> std::collections::HashSet<&'static str> {
    SQL_KEYWORDS.iter().copied().collect()
}

fn build_function_set() -> std::collections::HashSet<&'static str> {
    SQL_FUNCTIONS.iter().copied().collect()
}

fn tokenize_sql(text: &str) -> Vec<SqlToken> {
    let kw_set = build_keyword_set();
    let fn_set = build_function_set();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    macro_rules! flush {
        () => {
            if !current.is_empty() {
                let upper = current.to_uppercase();
                let is_kw = kw_set.contains(upper.as_str()) || fn_set.contains(upper.as_str());
                tokens.push(SqlToken {
                    text: upper,
                    original: std::mem::take(&mut current),
                    is_keyword: is_kw,
                });
            }
        };
    }

    while i < len {
        let ch = chars[i];

        // --- inside a string literal ---
        if in_string {
            if ch == '\'' {
                if i + 1 < len && chars[i + 1] == '\'' {
                    i += 2; // escaped ''
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }

        // --- inside a line comment ---
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        // --- inside a block comment ---
        if in_block_comment {
            if ch == '*' && i + 1 < len && chars[i + 1] == '/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        // --- string literal start ---
        if ch == '\'' {
            flush!();
            in_string = true;
            i += 1;
            continue;
        }

        // --- line comment start ---
        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            flush!();
            in_line_comment = true;
            i += 2;
            continue;
        }

        // --- block comment start ---
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            flush!();
            in_block_comment = true;
            i += 2;
            continue;
        }

        // --- boundary ---
        if is_boundary_char(ch) {
            flush!();
            if ch == '.' {
                tokens.push(SqlToken {
                    text: ".".to_string(),
                    original: ".".to_string(),
                    is_keyword: false,
                });
            }
            i += 1;
            continue;
        }

        current.push(ch);
        i += 1;
    }

    flush!();
    tokens
}

// ---------------------------------------------------------------------------
// Context detection
// ---------------------------------------------------------------------------

fn detect_context(tokens: &[SqlToken]) -> SqlContext {
    if tokens.is_empty() {
        return SqlContext::Default;
    }

    // Walk backwards through keyword tokens.
    let mut i = tokens.len();
    while i > 0 {
        i -= 1;
        let tok = &tokens[i];
        if !tok.is_keyword {
            continue;
        }
        match tok.text.as_str() {
            "SELECT" => {
                if has_kw_after(
                    tokens,
                    i,
                    &["FROM", "WHERE", "ORDER", "GROUP", "HAVING", "LIMIT"],
                ) {
                    continue;
                }
                return SqlContext::SelectClause;
            }
            "FROM" => {
                if i >= 1 && tokens[i - 1].text == "DELETE" {
                    return SqlContext::DeleteFrom;
                }
                return SqlContext::FromClause;
            }
            "WHERE" => return SqlContext::WhereClause,
            "AND" | "OR" => {
                if find_preceding_kw(tokens, i, &["HAVING"]) {
                    return SqlContext::HavingClause;
                }
                return SqlContext::WhereClause;
            }
            "ON" => {
                if find_preceding_kw(
                    tokens,
                    i,
                    &[
                        "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "NATURAL",
                    ],
                ) {
                    return SqlContext::JoinOn;
                }
                return SqlContext::WhereClause;
            }
            "ORDER" => return SqlContext::OrderByClause,
            "BY" => {
                if i > 0 && tokens[i - 1].text == "GROUP" {
                    return SqlContext::GroupByClause;
                }
                return SqlContext::OrderByClause;
            }
            "GROUP" => return SqlContext::GroupByClause,
            "HAVING" => return SqlContext::HavingClause,
            "INSERT" => return SqlContext::InsertInto,
            "INTO" => {
                if i > 0 && tokens[i - 1].text == "INSERT" {
                    return SqlContext::InsertInto;
                }
                continue;
            }
            "UPDATE" => return SqlContext::UpdateTable,
            "SET" => {
                if find_preceding_kw(tokens, i, &["UPDATE"]) {
                    return SqlContext::UpdateSet;
                }
                continue;
            }
            "DELETE" => return SqlContext::DeleteFrom,
            "VALUES" => return SqlContext::ValuesClause,
            kw if matches!(
                kw,
                "JOIN" | "LEFT" | "RIGHT" | "INNER" | "OUTER" | "FULL" | "CROSS" | "NATURAL"
            ) =>
            {
                // After a JOIN keyword, suggest tables — unless ON follows
                if !has_kw_after(tokens, i, &["ON"]) {
                    return SqlContext::FromClause;
                }
                continue;
            }
            _ => continue,
        }
    }
    SqlContext::Default
}

fn has_kw_after(tokens: &[SqlToken], from: usize, kws: &[&str]) -> bool {
    tokens[from + 1..]
        .iter()
        .any(|t| t.is_keyword && kws.contains(&t.text.as_str()))
}

fn find_preceding_kw(tokens: &[SqlToken], from: usize, kws: &[&str]) -> bool {
    tokens[..from]
        .iter()
        .rev()
        .any(|t| t.is_keyword && kws.contains(&t.text.as_str()))
}

// ---------------------------------------------------------------------------
// Alias parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct AliasInfo {
    table_name: String,
    qualified_name: String,
}

type Aliases = std::collections::HashMap<String, AliasInfo>;

/// Walk the token stream and extract `table alias` / `table AS alias` mappings.
fn parse_aliases(tokens: &[SqlToken], schema: &SchemaMetadata) -> Aliases {
    let mut aliases = Aliases::new();
    let mut i = 0;

    while i < tokens.len() {
        let tok = &tokens[i];
        let is_from_trigger = tok.is_keyword && tok.text == "FROM";
        let is_join_trigger = tok.is_keyword && tok.text == "JOIN";
        let is_comma_trigger = tok.text == ",";

        if is_from_trigger || is_join_trigger || is_comma_trigger {
            let mut j = i + 1;

            // Skip join modifiers
            while j < tokens.len()
                && tokens[j].is_keyword
                && matches!(
                    tokens[j].text.as_str(),
                    "LEFT" | "RIGHT" | "INNER" | "OUTER" | "FULL" | "CROSS" | "NATURAL"
                )
            {
                j += 1;
            }
            // Skip JOIN keyword itself (e.g. after LEFT)
            if j < tokens.len() && tokens[j].is_keyword && tokens[j].text == "JOIN" {
                j += 1;
            }

            if j >= tokens.len() {
                i += 1;
                continue;
            }

            let table_ref = read_table_ref(tokens, &mut j);
            if table_ref.is_empty() {
                i += 1;
                continue;
            }

            // Look for AS <alias> or implicit alias
            let mut alias: Option<String> = None;

            if j < tokens.len() && tokens[j].is_keyword && tokens[j].text == "AS" {
                j += 1;
                if j < tokens.len() && !tokens[j].is_keyword {
                    alias = Some(tokens[j].original.clone());
                    j += 1;
                }
            } else if j < tokens.len()
                && !tokens[j].is_keyword
                && tokens[j].text != "."
                && !is_clause_keyword(&tokens[j].text)
            {
                alias = Some(tokens[j].original.clone());
                j += 1;
            }

            if let Some(alias_name) = alias {
                let qualified = find_qualified_name(&table_ref, schema);
                aliases.insert(
                    alias_name.to_lowercase(),
                    AliasInfo {
                        table_name: table_ref,
                        qualified_name: qualified,
                    },
                );
            }
            i = j;
            continue;
        }
        i += 1;
    }
    aliases
}

fn is_clause_keyword(text: &str) -> bool {
    matches!(
        text,
        "WHERE"
            | "ORDER"
            | "GROUP"
            | "HAVING"
            | "LIMIT"
            | "OFFSET"
            | "ON"
            | "SET"
            | "INNER"
            | "LEFT"
            | "RIGHT"
            | "OUTER"
            | "FULL"
            | "CROSS"
            | "NATURAL"
            | "JOIN"
            | "UNION"
            | "VALUES"
            | "RETURNING"
    )
}

/// Read a possibly-qualified table reference starting at `*j`.
fn read_table_ref(tokens: &[SqlToken], j: &mut usize) -> String {
    if *j >= tokens.len() || tokens[*j].is_keyword || tokens[*j].text == "." {
        return String::new();
    }
    let mut parts = vec![tokens[*j].original.clone()];
    *j += 1;
    while *j + 1 < tokens.len() && tokens[*j].text == "." && !tokens[*j + 1].is_keyword {
        *j += 1; // dot
        parts.push(tokens[*j].original.clone());
        *j += 1;
    }
    parts.join(".")
}

fn find_qualified_name(table_ref: &str, schema: &SchemaMetadata) -> String {
    if table_ref.contains('.') {
        return table_ref.to_string();
    }
    for t in &schema.tables {
        if t.name.eq_ignore_ascii_case(table_ref) {
            return t.qualified_name.clone();
        }
    }
    table_ref.to_string()
}

// ---------------------------------------------------------------------------
// Collect tables referenced in FROM / JOIN clauses
// ---------------------------------------------------------------------------

fn collect_from_tables(tokens: &[SqlToken]) -> Vec<String> {
    let mut tables = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_from = tokens[i].is_keyword && tokens[i].text == "FROM";
        let is_join = tokens[i].is_keyword && tokens[i].text == "JOIN";
        if is_from || is_join {
            let mut j = i + 1;
            // skip modifiers
            while j < tokens.len()
                && tokens[j].is_keyword
                && matches!(
                    tokens[j].text.as_str(),
                    "LEFT" | "RIGHT" | "INNER" | "OUTER" | "FULL" | "CROSS" | "NATURAL"
                )
            {
                j += 1;
            }
            if j < tokens.len() && tokens[j].is_keyword && tokens[j].text == "JOIN" {
                j += 1;
            }
            if j < tokens.len() {
                let tr = read_table_ref(tokens, &mut j);
                if !tr.is_empty() {
                    tables.push(tr);
                }
            }
        }
        // comma in FROM list
        if tokens[i].text == "," {
            let mut j = i + 1;
            if j < tokens.len() {
                let tr = read_table_ref(tokens, &mut j);
                if !tr.is_empty() {
                    tables.push(tr);
                }
            }
        }
        i += 1;
    }
    tables
}

// ---------------------------------------------------------------------------
// Column lookups
// ---------------------------------------------------------------------------

fn get_columns_for_ref(ref_name: &str, aliases: &Aliases, schema: &SchemaMetadata) -> Vec<String> {
    if let Some(info) = aliases.get(&ref_name.to_lowercase()) {
        return get_columns_for_qualified(&info.qualified_name, schema);
    }
    let mut cols = get_columns_for_qualified(ref_name, schema);
    if cols.is_empty() {
        // try unqualified
        let unqualified = ref_name.split('.').last().unwrap_or(ref_name);
        cols = get_columns_for_qualified(unqualified, schema);
    }
    cols
}

fn get_columns_for_qualified(table_ref: &str, schema: &SchemaMetadata) -> Vec<String> {
    for t in &schema.tables {
        if t.name.eq_ignore_ascii_case(table_ref)
            || t.qualified_name.eq_ignore_ascii_case(table_ref)
        {
            return t.columns.clone();
        }
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Find UPDATE table
// ---------------------------------------------------------------------------

fn find_update_table(tokens: &[SqlToken]) -> Option<String> {
    for (i, tok) in tokens.iter().enumerate() {
        if tok.is_keyword && tok.text == "UPDATE" {
            for j in i + 1..tokens.len() {
                if !tokens[j].is_keyword {
                    return Some(tokens[j].original.clone());
                }
                break;
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Dot completion
// ---------------------------------------------------------------------------

fn try_dot_completion(
    text_before_cursor: &str,
    cursor: usize,
    aliases: &Aliases,
    schema: &SchemaMetadata,
) -> Option<Vec<CompletionItem>> {
    let (partial, _) = extract_current_token(text_before_cursor, cursor);

    // Case 1: partial token contains a dot, e.g. "users.na"
    if let Some(dot_pos) = partial.rfind('.') {
        let prefix = &partial[..dot_pos];
        let suffix = &partial[dot_pos + 1..];
        let columns = get_columns_for_ref(prefix, aliases, schema);
        let items: Vec<CompletionItem> = columns
            .into_iter()
            .map(|col| CompletionItem {
                label: col.clone(),
                kind: CompletionKind::Column,
                detail: None,
                insert_text: col,
            })
            .collect();
        return Some(filter_and_rank(items, suffix));
    }

    // Case 2: cursor is right after a dot with nothing typed yet, e.g. "users."
    if cursor > 0 && text_before_cursor.as_bytes().get(cursor - 1) == Some(&b'.') {
        let before_dot = &text_before_cursor[..cursor - 1];
        let (ident, _) = extract_current_token(before_dot, before_dot.len());
        if !ident.is_empty() {
            let columns = get_columns_for_ref(&ident, aliases, schema);
            let items: Vec<CompletionItem> = columns
                .into_iter()
                .map(|col| CompletionItem {
                    label: col.clone(),
                    kind: CompletionKind::Column,
                    detail: None,
                    insert_text: col,
                })
                .collect();
            return Some(filter_and_rank(items, ""));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Keyword / function helpers for specific contexts
// ---------------------------------------------------------------------------

fn push_keywords(items: &mut Vec<CompletionItem>) {
    for kw in SQL_KEYWORDS {
        items.push(CompletionItem {
            label: (*kw).to_string(),
            kind: CompletionKind::Keyword,
            detail: None,
            insert_text: (*kw).to_string(),
        });
    }
}

fn push_functions(items: &mut Vec<CompletionItem>) {
    for func in SQL_FUNCTIONS {
        let label = (*func).to_string();
        if items.iter().any(|i| i.label == label) {
            continue;
        }
        items.push(CompletionItem {
            label: label.clone(),
            kind: CompletionKind::Function,
            detail: Some("function".to_string()),
            insert_text: format!("{}()", func),
        });
    }
}

fn push_kw_if_missing(items: &mut Vec<CompletionItem>, kws: &[&str]) {
    for kw in kws {
        if !items.iter().any(|i| i.label == *kw) {
            items.push(CompletionItem {
                label: (*kw).to_string(),
                kind: CompletionKind::Keyword,
                detail: None,
                insert_text: (*kw).to_string(),
            });
        }
    }
}

fn push_table_items(items: &mut Vec<CompletionItem>, schema: &SchemaMetadata) {
    for t in &schema.tables {
        items.push(CompletionItem {
            label: t.name.clone(),
            kind: CompletionKind::Table,
            detail: Some(t.qualified_name.clone()),
            insert_text: t.name.clone(),
        });
    }
}

fn push_schema_items(items: &mut Vec<CompletionItem>, schema: &SchemaMetadata) {
    for s in &schema.schemas {
        items.push(CompletionItem {
            label: s.clone(),
            kind: CompletionKind::Schema,
            detail: None,
            insert_text: s.clone(),
        });
    }
}

fn push_columns_from_tables(
    items: &mut Vec<CompletionItem>,
    tables: &[String],
    aliases: &Aliases,
    schema: &SchemaMetadata,
) {
    for table in tables {
        let cols = get_columns_for_ref(table, aliases, schema);
        for col in cols {
            items.push(CompletionItem {
                label: col.clone(),
                kind: CompletionKind::Column,
                detail: Some(format!("from {}", table)),
                insert_text: col,
            });
        }
    }
}

fn push_alias_items(items: &mut Vec<CompletionItem>, aliases: &Aliases) {
    for (name, info) in aliases {
        items.push(CompletionItem {
            label: name.clone(),
            kind: CompletionKind::Alias,
            detail: Some(format!("→ {}", info.table_name)),
            insert_text: name.clone(),
        });
    }
}

// ---------------------------------------------------------------------------
// Build candidate list
// ---------------------------------------------------------------------------

fn build_candidates(
    context: &SqlContext,
    schema: &SchemaMetadata,
    aliases: &Aliases,
    from_tables: &[String],
    pre_tokens: &[SqlToken],
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    match context {
        SqlContext::SelectClause => {
            if from_tables.is_empty() {
                push_keywords(&mut items);
                push_functions(&mut items);
            } else {
                push_columns_from_tables(&mut items, from_tables, aliases, schema);
                items.push(CompletionItem {
                    label: "*".to_string(),
                    kind: CompletionKind::Column,
                    detail: Some("all columns".to_string()),
                    insert_text: "*".to_string(),
                });
                push_functions(&mut items);
                push_kw_if_missing(
                    &mut items,
                    &[
                        "DISTINCT", "AS", "CASE", "WHEN", "THEN", "ELSE", "END", "FROM", "WHERE",
                    ],
                );
            }
        }
        SqlContext::FromClause | SqlContext::DeleteFrom => {
            push_table_items(&mut items, schema);
            push_schema_items(&mut items, schema);
            push_kw_if_missing(
                &mut items,
                &[
                    "AS", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "ON",
                    "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "OFFSET", "UNION",
                ],
            );
        }
        SqlContext::InsertInto | SqlContext::UpdateTable => {
            push_table_items(&mut items, schema);
        }
        SqlContext::WhereClause | SqlContext::JoinOn => {
            push_columns_from_tables(&mut items, from_tables, aliases, schema);
            push_alias_items(&mut items, aliases);
            push_kw_if_missing(
                &mut items,
                &[
                    "AND", "OR", "NOT", "IN", "IS", "NULL", "LIKE", "BETWEEN", "EXISTS", "TRUE",
                    "FALSE", "CAST", "CASE", "WHEN", "THEN", "ELSE", "END", "ALL", "ANY", "SOME",
                    "ILIKE",
                ],
            );
            push_functions(&mut items);
        }
        SqlContext::OrderByClause | SqlContext::GroupByClause => {
            push_columns_from_tables(&mut items, from_tables, aliases, schema);
            push_kw_if_missing(
                &mut items,
                &[
                    "ASC", "DESC", "NULLS", "FIRST", "LAST", "HAVING", "LIMIT", "OFFSET",
                ],
            );
        }
        SqlContext::HavingClause => {
            push_columns_from_tables(&mut items, from_tables, aliases, schema);
            push_kw_if_missing(
                &mut items,
                &[
                    "AND", "OR", "NOT", "IN", "IS", "NULL", "LIKE", "BETWEEN", "EXISTS",
                ],
            );
            push_functions(&mut items);
        }
        SqlContext::UpdateSet => {
            if let Some(table_name) = find_update_table(pre_tokens) {
                let cols = get_columns_for_ref(&table_name, aliases, schema);
                for col in cols {
                    items.push(CompletionItem {
                        label: col.clone(),
                        kind: CompletionKind::Column,
                        detail: Some(format!("from {}", table_name)),
                        insert_text: col,
                    });
                }
            }
        }
        SqlContext::ValuesClause | SqlContext::DotAccess => {}
        SqlContext::Default => {
            push_keywords(&mut items);
            push_functions(&mut items);
        }
    }
    items
}

// ---------------------------------------------------------------------------
// Filter and rank
// ---------------------------------------------------------------------------

fn filter_and_rank(items: Vec<CompletionItem>, prefix: &str) -> Vec<CompletionItem> {
    if prefix.is_empty() {
        let mut sorted = items;
        sorted.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        sorted.dedup_by(|a, b| a.label.eq_ignore_ascii_case(&b.label));
        sorted.truncate(50);
        return sorted;
    }

    let prefix_lower = prefix.to_lowercase();
    let mut exact = Vec::new();
    let mut ci = Vec::new();

    for item in items {
        if item.label.starts_with(prefix) {
            exact.push(item);
        } else if item.label.to_lowercase().starts_with(&prefix_lower) {
            ci.push(item);
        }
    }

    exact.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    ci.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));

    exact.extend(ci);
    exact.dedup_by(|a, b| a.label.eq_ignore_ascii_case(&b.label));
    exact.truncate(50);
    exact
}

fn default_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    push_keywords(&mut items);
    push_functions(&mut items);
    filter_and_rank(items, prefix)
}

// ---------------------------------------------------------------------------
// Public entrypoint
// ---------------------------------------------------------------------------

/// Return a ranked list of completions for the given `sql` text at byte-offset
/// `cursor`, using the supplied `schema` metadata.
pub fn complete_sql(sql: &str, cursor: usize, schema: &SchemaMetadata) -> Vec<CompletionItem> {
    let cursor = cursor.min(sql.len());

    if sql.is_empty() || cursor == 0 {
        return default_keyword_completions("");
    }

    let text_before_cursor = &sql[..cursor];

    // Never complete inside strings or comments.
    if is_in_non_code_region(sql, cursor) {
        return Vec::new();
    }

    let (partial_token, _) = extract_current_token(text_before_cursor, cursor);

    // Tokenize full SQL for alias resolution and table collection.
    let full_tokens = tokenize_sql(sql);
    let aliases = parse_aliases(&full_tokens, schema);
    let from_tables = collect_from_tables(&full_tokens);

    // Tokenize pre-cursor text for context detection.
    let pre_tokens = tokenize_sql(text_before_cursor);

    // Dot completion takes priority.
    if let Some(dot) = try_dot_completion(text_before_cursor, cursor, &aliases, schema) {
        return dot;
    }

    let context = detect_context(&pre_tokens);
    let candidates = build_candidates(&context, schema, &aliases, &from_tables, &pre_tokens);
    filter_and_rank(candidates, &partial_token)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> SchemaMetadata {
        SchemaMetadata {
            tables: vec![
                TableMeta {
                    schema: Some("public".into()),
                    name: "users".into(),
                    qualified_name: "public.users".into(),
                    columns: vec![
                        "id".into(),
                        "name".into(),
                        "email".into(),
                        "age".into(),
                        "created_at".into(),
                    ],
                },
                TableMeta {
                    schema: Some("public".into()),
                    name: "orders".into(),
                    qualified_name: "public.orders".into(),
                    columns: vec![
                        "id".into(),
                        "user_id".into(),
                        "amount".into(),
                        "status".into(),
                        "created_at".into(),
                    ],
                },
                TableMeta {
                    schema: Some("analytics".into()),
                    name: "events".into(),
                    qualified_name: "analytics.events".into(),
                    columns: vec![
                        "id".into(),
                        "event_type".into(),
                        "payload".into(),
                        "timestamp".into(),
                    ],
                },
            ],
            schemas: vec!["public".into(), "analytics".into()],
        }
    }

    // ---- CompletionKind ----

    #[test]
    fn completion_kind_labels() {
        assert_eq!(CompletionKind::Table.label(), "T");
        assert_eq!(CompletionKind::View.label(), "V");
        assert_eq!(CompletionKind::Column.label(), "C");
        assert_eq!(CompletionKind::Keyword.label(), "K");
        assert_eq!(CompletionKind::Function.label(), "F");
        assert_eq!(CompletionKind::Schema.label(), "S");
        assert_eq!(CompletionKind::Alias.label(), "A");
    }

    // ---- String literal detection ----

    #[test]
    fn not_in_string() {
        assert!(!is_in_string_literal("SELECT * FROM users", 5));
    }

    #[test]
    fn inside_string() {
        assert!(is_in_string_literal("SELECT 'hello world' FROM", 13));
        assert!(is_in_string_literal("SELECT 'hello", 12));
    }

    #[test]
    fn escaped_quote_stays_in_string() {
        let sql = "SELECT 'it''s a test'";
        assert!(is_in_string_literal(sql, 14));
        assert!(!is_in_string_literal(sql, sql.len()));
    }

    // ---- Comment detection ----

    #[test]
    fn not_in_comment() {
        assert!(!is_in_line_comment("SELECT * FROM users", 10));
    }

    #[test]
    fn inside_line_comment() {
        assert!(is_in_line_comment("SELECT * -- comment", 17));
        assert!(is_in_line_comment("-- comment", 5));
    }

    #[test]
    fn dashes_inside_string_not_comment() {
        assert!(!is_in_line_comment("SELECT 'a--b' FROM t", 15));
    }

    #[test]
    fn block_comment_detection() {
        assert!(is_in_block_comment("SELECT /* hi */ FROM", 12));
        assert!(!is_in_block_comment("SELECT /* hi */ FROM", 16));
    }

    // ---- Token extraction ----

    #[test]
    fn extract_token_empty() {
        let (t, s) = extract_current_token("", 0);
        assert_eq!(t, "");
        assert_eq!(s, 0);
    }

    #[test]
    fn extract_token_middle_of_word() {
        let (t, s) = extract_current_token("SELECT SEL", 10);
        assert_eq!(t, "SEL");
        assert_eq!(s, 7);
    }

    #[test]
    fn extract_token_after_space() {
        let (t, s) = extract_current_token("SELECT ", 7);
        assert_eq!(t, "");
        assert_eq!(s, 7);
    }

    #[test]
    fn extract_token_with_dot() {
        let (t, _s) = extract_current_token("users.na", 9);
        assert_eq!(t, "users.na");
    }

    // ---- Tokenizer ----

    #[test]
    fn tokenize_simple() {
        let tokens = tokenize_sql("SELECT id FROM users");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].text, "SELECT");
        assert_eq!(tokens[1].text, "ID");
        assert_eq!(tokens[2].text, "FROM");
        assert_eq!(tokens[3].text, "USERS");
    }

    #[test]
    fn tokenize_skips_string() {
        let tokens = tokenize_sql("SELECT 'hello world' FROM users");
        // SELECT, FROM, USERS  (string content skipped)
        assert!(tokens.iter().any(|t| t.text == "SELECT"));
        assert!(tokens.iter().any(|t| t.text == "FROM"));
        assert!(tokens.iter().any(|t| t.text == "USERS"));
        assert!(!tokens.iter().any(|t| t.text.contains("hello")));
    }

    #[test]
    fn tokenize_skips_line_comment() {
        let tokens = tokenize_sql("SELECT -- comment\ncol FROM users");
        assert!(tokens.iter().any(|t| t.text == "COL"));
        assert!(!tokens.iter().any(|t| t.text.contains("comment")));
    }

    #[test]
    fn tokenize_dot_is_separate() {
        let tokens = tokenize_sql("SELECT * FROM public.users");
        assert_eq!(tokens[3].text, "PUBLIC");
        assert_eq!(tokens[4].text, ".");
        assert_eq!(tokens[5].text, "USERS");
    }

    // ---- Context detection ----

    #[test]
    fn ctx_select() {
        let tokens = tokenize_sql("SELECT ");
        assert_eq!(detect_context(&tokens), SqlContext::SelectClause);
    }

    #[test]
    fn ctx_from() {
        let tokens = tokenize_sql("SELECT * FROM ");
        assert_eq!(detect_context(&tokens), SqlContext::FromClause);
    }

    #[test]
    fn ctx_where() {
        let tokens = tokenize_sql("SELECT * FROM users WHERE ");
        assert_eq!(detect_context(&tokens), SqlContext::WhereClause);
    }

    #[test]
    fn ctx_after_and() {
        let tokens = tokenize_sql("SELECT * FROM users WHERE id = 1 AND ");
        assert_eq!(detect_context(&tokens), SqlContext::WhereClause);
    }

    #[test]
    fn ctx_order_by() {
        let tokens = tokenize_sql("SELECT * FROM users ORDER BY ");
        assert_eq!(detect_context(&tokens), SqlContext::OrderByClause);
    }

    #[test]
    fn ctx_group_by() {
        let tokens = tokenize_sql("SELECT * FROM users GROUP BY ");
        assert_eq!(detect_context(&tokens), SqlContext::GroupByClause);
    }

    #[test]
    fn ctx_insert_into() {
        let tokens = tokenize_sql("INSERT INTO ");
        assert_eq!(detect_context(&tokens), SqlContext::InsertInto);
    }

    #[test]
    fn ctx_update() {
        let tokens = tokenize_sql("UPDATE ");
        assert_eq!(detect_context(&tokens), SqlContext::UpdateTable);
    }

    #[test]
    fn ctx_update_set() {
        let tokens = tokenize_sql("UPDATE users SET ");
        assert_eq!(detect_context(&tokens), SqlContext::UpdateSet);
    }

    #[test]
    fn ctx_delete_from() {
        let tokens = tokenize_sql("DELETE FROM ");
        assert_eq!(detect_context(&tokens), SqlContext::DeleteFrom);
    }

    #[test]
    fn ctx_default_empty() {
        let tokens = tokenize_sql("");
        assert_eq!(detect_context(&tokens), SqlContext::Default);
    }

    #[test]
    fn ctx_select_before_from_with_from_later() {
        // Pre-cursor tokens only contain SELECT + partial
        let tokens = tokenize_sql("SELECT  ");
        assert_eq!(detect_context(&tokens), SqlContext::SelectClause);
    }

    // ---- Alias parsing ----

    #[test]
    fn alias_simple() {
        let schema = test_schema();
        let tokens = tokenize_sql("SELECT * FROM users u WHERE ");
        let aliases = parse_aliases(&tokens, &schema);
        assert!(aliases.contains_key("u"));
        assert_eq!(aliases["u"].table_name, "users");
    }

    #[test]
    fn alias_with_as() {
        let schema = test_schema();
        let tokens = tokenize_sql("SELECT * FROM users AS u WHERE ");
        let aliases = parse_aliases(&tokens, &schema);
        assert!(aliases.contains_key("u"));
        assert_eq!(aliases["u"].table_name, "users");
    }

    #[test]
    fn alias_qualified_table() {
        let schema = test_schema();
        let tokens = tokenize_sql("SELECT * FROM public.users AS u WHERE ");
        let aliases = parse_aliases(&tokens, &schema);
        assert!(aliases.contains_key("u"));
    }

    #[test]
    fn alias_multiple_joins() {
        let schema = test_schema();
        let tokens = tokenize_sql("SELECT * FROM users u JOIN orders o ON u.id = o.user_id");
        let aliases = parse_aliases(&tokens, &schema);
        assert!(aliases.contains_key("u"));
        assert!(aliases.contains_key("o"));
        assert_eq!(aliases["u"].table_name, "users");
        assert_eq!(aliases["o"].table_name, "orders");
    }

    // ---- Dot completion ----

    #[test]
    fn dot_columns_for_table() {
        let schema = test_schema();
        let results = complete_sql("SELECT users.", 13, &schema);
        assert!(results.iter().any(|r| r.label == "id"));
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(results.iter().any(|r| r.label == "email"));
    }

    #[test]
    fn dot_columns_for_alias() {
        let schema = test_schema();
        // Alias u is declared after the cursor position, but full SQL is parsed.
        let results = complete_sql("SELECT u.", 9, &schema);
        // u is not yet declared at this point in the full SQL —
        // but we parse the full SQL so the alias is available.
        // However, "SELECT u." at pos 9, full sql is "SELECT u." —
        // there's no FROM clause, so no alias. Let's adjust:
        let sql = "SELECT u. FROM users u";
        let results = complete_sql(sql, 9, &schema);
        assert!(results.iter().any(|r| r.label == "id"));
        assert!(results.iter().any(|r| r.label == "name"));
    }

    #[test]
    fn dot_partial_column() {
        let schema = test_schema();
        let results = complete_sql("SELECT users.na FROM users", 16, &schema);
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(!results.iter().any(|r| r.label == "id"));
    }

    #[test]
    fn dot_unknown_table_returns_empty() {
        let schema = test_schema();
        let results = complete_sql("SELECT foobar.", 14, &schema);
        assert!(results.is_empty());
    }

    // ---- Main function ----

    #[test]
    fn empty_sql_returns_keywords() {
        let schema = test_schema();
        let results = complete_sql("", 0, &schema);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.label == "SELECT"));
    }

    #[test]
    fn select_columns_from_table() {
        let schema = test_schema();
        let results = complete_sql("SELECT  FROM users", 7, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "id" && r.kind == CompletionKind::Column)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "name" && r.kind == CompletionKind::Column)
        );
    }

    #[test]
    fn from_clause_suggests_tables() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM ", 14, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "users" && r.kind == CompletionKind::Table)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "orders" && r.kind == CompletionKind::Table)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "public" && r.kind == CompletionKind::Schema)
        );
    }

    #[test]
    fn where_clause_suggests_columns_and_keywords() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users WHERE ", 26, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "id" && r.kind == CompletionKind::Column)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "AND" && r.kind == CompletionKind::Keyword)
        );
    }

    #[test]
    fn no_completion_inside_string() {
        let schema = test_schema();
        let results = complete_sql("SELECT 'hello world", 15, &schema);
        assert!(results.is_empty());
    }

    #[test]
    fn no_completion_inside_comment() {
        let schema = test_schema();
        let results = complete_sql("SELECT * -- some comment", 22, &schema);
        assert!(results.is_empty());
    }

    #[test]
    fn prefix_filtering_from_clause() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM us", 16, &schema);
        assert!(results.iter().any(|r| r.label == "users"));
        assert!(!results.iter().any(|r| r.label == "orders"));
    }

    #[test]
    fn case_insensitive_filtering() {
        let schema = test_schema();
        let results = complete_sql("select * from US", 16, &schema);
        assert!(results.iter().any(|r| r.label == "users"));
    }

    #[test]
    fn insert_into_tables() {
        let schema = test_schema();
        let results = complete_sql("INSERT INTO ", 12, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "users" && r.kind == CompletionKind::Table)
        );
    }

    #[test]
    fn update_set_columns() {
        let schema = test_schema();
        let results = complete_sql("UPDATE users SET ", 17, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "name" && r.kind == CompletionKind::Column)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "email" && r.kind == CompletionKind::Column)
        );
    }

    #[test]
    fn order_by_columns() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users ORDER BY ", 29, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "id" && r.kind == CompletionKind::Column)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "name" && r.kind == CompletionKind::Column)
        );
    }

    #[test]
    fn multiline_sql() {
        let schema = test_schema();
        let sql = "SELECT *\nFROM users\nWHERE ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "id" && r.kind == CompletionKind::Column)
        );
    }

    #[test]
    fn delete_from_tables() {
        let schema = test_schema();
        let results = complete_sql("DELETE FROM ", 12, &schema);
        assert!(results.iter().any(|r| r.label == "users"));
    }

    #[test]
    fn result_limit_50() {
        let schema = test_schema();
        let results = complete_sql("", 0, &schema);
        assert!(results.len() <= 50);
    }

    #[test]
    fn exact_prefix_ranked_first() {
        let schema = test_schema();
        let sql = "SELECT * FROM users WHERE na";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(!results.is_empty());
        assert_eq!(results[0].label, "name");
    }

    #[test]
    fn cursor_at_end() {
        let schema = test_schema();
        let sql = "SELECT * FROM users WHERE id = ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(!results.is_empty());
    }

    #[test]
    fn cursor_beyond_end_clamped() {
        let schema = test_schema();
        let results = complete_sql("SELECT", 1000, &schema);
        assert!(!results.is_empty());
    }

    #[test]
    fn block_comment_no_completions() {
        let schema = test_schema();
        let results = complete_sql("SELECT /* something */", 14, &schema);
        assert!(results.is_empty());
    }

    #[test]
    fn after_block_comment_completions() {
        let schema = test_schema();
        let sql = "SELECT /* c */ * FROM ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(results.iter().any(|r| r.label == "users"));
    }

    #[test]
    fn default_keyword_suggestion() {
        let schema = SchemaMetadata::default();
        let results = complete_sql("SEL", 3, &schema);
        assert!(results.iter().any(|r| r.label == "SELECT"));
    }

    #[test]
    fn functions_in_select() {
        let schema = test_schema();
        let results = complete_sql("SELECT COU FROM users", 10, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "COUNT" && r.kind == CompletionKind::Function)
        );
    }

    #[test]
    fn select_star_option() {
        let schema = test_schema();
        let results = complete_sql("SELECT  FROM users", 7, &schema);
        assert!(results.iter().any(|r| r.label == "*"));
    }

    #[test]
    fn alias_resolved_from_full_sql() {
        let schema = test_schema();
        // The alias is declared after the cursor in the full SQL text.
        let sql = "SELECT u. FROM users u";
        let results = complete_sql(sql, 9, &schema);
        assert!(results.iter().any(|r| r.label == "id"));
        assert!(results.iter().any(|r| r.label == "name"));
    }

    #[test]
    fn join_on_suggests_columns() {
        let schema = test_schema();
        let sql = "SELECT * FROM users u JOIN orders o ON ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "id" && r.kind == CompletionKind::Column)
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "u" && r.kind == CompletionKind::Alias)
        );
    }

    #[test]
    fn having_clause_suggests_aggregates() {
        let schema = test_schema();
        let sql = "SELECT COUNT(*) FROM users GROUP BY name HAVING ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "COUNT" && r.kind == CompletionKind::Function)
        );
    }

    #[test]
    fn deduplication_by_label() {
        let schema = test_schema();
        let results = complete_sql("", 0, &schema);
        let labels: Vec<&str> = results.iter().map(|r| r.label.as_str()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len(), "duplicate labels found");
    }

    #[test]
    fn comma_separated_from_tables() {
        let schema = test_schema();
        let sql = "SELECT * FROM users, orders WHERE ";
        let results = complete_sql(sql, sql.len(), &schema);
        // Columns from both tables should appear
        let has_id = results.iter().any(|r| r.label == "id");
        assert!(has_id, "should have 'id' columns from both tables");
    }

    #[test]
    fn qualified_table_in_from() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM public.users WHERE ", 31, &schema);
        assert!(
            results
                .iter()
                .any(|r| r.label == "id" && r.kind == CompletionKind::Column)
        );
    }

    #[test]
    fn left_join_suggests_tables() {
        let schema = test_schema();
        let results = complete_sql(
            "SELECT * FROM users LEFT JOIN ",
            "SELECT * FROM users LEFT JOIN ".len(),
            &schema,
        );
        assert!(
            results
                .iter()
                .any(|r| r.label == "orders" && r.kind == CompletionKind::Table)
        );
    }

    #[test]
    fn select_with_table_prefix_filter() {
        let schema = test_schema();
        let results = complete_sql("SELECT na FROM users", 10, &schema);
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(!results.iter().any(|r| r.label == "id"));
    }

    #[test]
    fn cursor_at_position_zero() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users", 0, &schema);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.label == "SELECT"));
    }
}
