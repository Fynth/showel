mod completion_alias;
mod completion_candidates;
mod completion_context;
mod completion_keywords;
mod completion_tokenizer;
mod completion_types;

pub use completion_types::{CompletionItem, CompletionKind, SchemaMetadata, TableMeta};

pub use completion_alias::{parse_aliases, AliasInfo, Aliases};
pub use completion_candidates::{
    build_candidates, default_keyword_completions, filter_and_rank, try_dot_completion,
};
pub use completion_context::{collect_from_tables, detect_context, SqlContext};
pub use completion_keywords::{build_function_set, build_keyword_set, SQL_FUNCTIONS, SQL_KEYWORDS};
pub use completion_tokenizer::{
    extract_current_token, is_boundary_char, is_in_block_comment, is_in_line_comment,
    is_in_non_code_region, is_in_string_literal, tokenize_sql, SqlToken,
};

pub fn complete_sql(sql: &str, cursor: usize, schema: &SchemaMetadata) -> Vec<CompletionItem> {
    let cursor = cursor.min(sql.len());

    if sql.is_empty() || cursor == 0 {
        return default_keyword_completions("");
    }

    let text_before_cursor = &sql[..cursor];

    if is_in_non_code_region(sql, cursor) {
        return Vec::new();
    }

    let (partial_token, _) = extract_current_token(text_before_cursor, cursor);

    let full_tokens = tokenize_sql(sql);
    let aliases = parse_aliases(&full_tokens, schema);
    let from_tables = collect_from_tables(&full_tokens);

    let pre_tokens = tokenize_sql(text_before_cursor);

    if let Some(dot) = try_dot_completion(text_before_cursor, cursor, &aliases, schema) {
        return dot;
    }

    let context = detect_context(&pre_tokens);
    let candidates = build_candidates(&context, schema, &aliases, &from_tables, &pre_tokens);
    filter_and_rank(candidates, &partial_token)
}

#[cfg(test)]
mod tests {
    use super::completion_types::{CompletionKind, SchemaMetadata, TableMeta};
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
        let (t, _s) = extract_current_token("users.na", 8);
        assert_eq!(t, "users.na");
    }

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
        assert_eq!(tokens[2].text, "PUBLIC");
        assert_eq!(tokens[3].text, ".");
        assert_eq!(tokens[4].text, "USERS");
    }

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
        let tokens = tokenize_sql("SELECT  ");
        assert_eq!(detect_context(&tokens), SqlContext::SelectClause);
    }

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
        let sql = "SELECT u. FROM users u";
        let results = complete_sql(sql, 9, &schema);
        assert!(results.iter().any(|r| r.label == "id"));
        assert!(results.iter().any(|r| r.label == "name"));
    }

    #[test]
    fn dot_partial_column() {
        let schema = test_schema();
        let results = complete_sql("SELECT users.na FROM users", 14, &schema);
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(!results.iter().any(|r| r.label == "id"));
    }

    #[test]
    fn dot_unknown_table_returns_empty() {
        let schema = test_schema();
        let results = complete_sql("SELECT foobar.", 14, &schema);
        assert!(results.is_empty());
    }

    #[test]
    fn empty_sql_returns_keywords() {
        let schema = test_schema();
        let results = complete_sql("", 0, &schema);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.kind == CompletionKind::Keyword));
    }

    #[test]
    fn select_columns_from_table() {
        let schema = test_schema();
        let results = complete_sql("SELECT  FROM users", 7, &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "id" && r.kind == CompletionKind::Column));
        assert!(results
            .iter()
            .any(|r| r.label == "name" && r.kind == CompletionKind::Column));
    }

    #[test]
    fn from_clause_suggests_tables() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM ", 14, &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "users" && r.kind == CompletionKind::Table));
        assert!(results
            .iter()
            .any(|r| r.label == "orders" && r.kind == CompletionKind::Table));
        assert!(results
            .iter()
            .any(|r| r.label == "public" && r.kind == CompletionKind::Schema));
    }

    #[test]
    fn where_clause_suggests_columns_and_keywords() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users WHERE ", 26, &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "id" && r.kind == CompletionKind::Column));
        assert!(results
            .iter()
            .any(|r| r.label == "AND" && r.kind == CompletionKind::Keyword));
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
        assert!(results
            .iter()
            .any(|r| r.label == "users" && r.kind == CompletionKind::Table));
    }

    #[test]
    fn update_set_columns() {
        let schema = test_schema();
        let results = complete_sql("UPDATE users SET ", 17, &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "name" && r.kind == CompletionKind::Column));
        assert!(results
            .iter()
            .any(|r| r.label == "email" && r.kind == CompletionKind::Column));
    }

    #[test]
    fn order_by_columns() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users ORDER BY ", 29, &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "id" && r.kind == CompletionKind::Column));
        assert!(results
            .iter()
            .any(|r| r.label == "name" && r.kind == CompletionKind::Column));
    }

    #[test]
    fn multiline_sql() {
        let schema = test_schema();
        let sql = "SELECT *\nFROM users\nWHERE ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "id" && r.kind == CompletionKind::Column));
    }

    #[test]
    fn delete_from_tables() {
        let schema = test_schema();
        let results = complete_sql("DELETE FROM ", 12, &schema);
        assert!(results.iter().any(|r| r.label == "users"));
    }

    #[test]
    fn result_limit_100() {
        let schema = test_schema();
        let results = complete_sql("", 0, &schema);
        assert!(results.len() <= 100);
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
        assert!(results
            .iter()
            .any(|r| r.label == "COUNT" && r.kind == CompletionKind::Function));
    }

    #[test]
    fn substring_match_in_where_clause() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users WHERE na", 26, &schema);
        assert!(
            results.iter().any(|r| r.label == "name"),
            "should match name"
        );
        assert!(
            results.iter().any(|r| r.label == "created_at"),
            "should match created_at"
        );
    }

    #[test]
    fn substring_match_finds_created_at() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users WHERE created", 29, &schema);
        assert!(
            results.iter().any(|r| r.label == "created_at"),
            "should match created_at"
        );
    }

    #[test]
    fn alias_resolved_from_full_sql() {
        let schema = test_schema();
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
        assert!(results
            .iter()
            .any(|r| r.label == "id" && r.kind == CompletionKind::Column));
        assert!(results
            .iter()
            .any(|r| r.label == "u" && r.kind == CompletionKind::Alias));
    }

    #[test]
    fn having_clause_suggests_aggregates() {
        let schema = test_schema();
        let sql = "SELECT COUNT(*) FROM users GROUP BY name HAVING ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "COUNT" && r.kind == CompletionKind::Function));
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
        let has_id = results.iter().any(|r| r.label == "id");
        assert!(has_id, "should have 'id' columns from both tables");
    }

    #[test]
    fn qualified_table_in_from() {
        let schema = test_schema();
        let sql = "SELECT * FROM public.users WHERE ";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(results
            .iter()
            .any(|r| r.label == "id" && r.kind == CompletionKind::Column));
    }

    #[test]
    fn left_join_suggests_tables() {
        let schema = test_schema();
        let results = complete_sql(
            "SELECT * FROM users LEFT JOIN ",
            "SELECT * FROM users LEFT JOIN ".len(),
            &schema,
        );
        assert!(results
            .iter()
            .any(|r| r.label == "orders" && r.kind == CompletionKind::Table));
    }

    #[test]
    fn select_with_table_prefix_filter() {
        let schema = test_schema();
        let results = complete_sql("SELECT na FROM users", 9, &schema);
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(!results.iter().any(|r| r.label == "id"));
    }

    #[test]
    fn cursor_at_position_zero() {
        let schema = test_schema();
        let results = complete_sql("SELECT * FROM users", 0, &schema);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.kind == CompletionKind::Keyword));
    }

    #[test]
    fn empty_metadata_returns_keywords_for_empty_sql() {
        let schema = SchemaMetadata::default();
        assert!(schema.tables.is_empty());
        assert!(schema.schemas.is_empty());

        let results = complete_sql("", 0, &schema);
        assert!(!results.is_empty(), "should still get keyword completions");
        assert!(
            results
                .iter()
                .all(|r| matches!(r.kind, CompletionKind::Keyword | CompletionKind::Function)),
            "all items should be keywords or functions when no metadata"
        );
    }

    #[test]
    fn empty_metadata_from_clause_returns_no_tables() {
        let schema = SchemaMetadata::default();
        let results = complete_sql("SELECT * FROM ", 14, &schema);
        let has_tables = results.iter().any(|r| r.kind == CompletionKind::Table);
        assert!(!has_tables, "no table completions when schema is empty");
        assert!(
            results
                .iter()
                .any(|r| r.label == "WHERE" || r.label == "JOIN"),
            "keywords should still appear in FROM context"
        );
    }

    #[test]
    fn empty_metadata_select_clause_returns_keywords_and_functions() {
        let schema = SchemaMetadata::default();
        let results = complete_sql("SELECT ", 7, &schema);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.kind == CompletionKind::Keyword));
        assert!(results.iter().any(|r| r.kind == CompletionKind::Function));
    }

    #[test]
    fn empty_metadata_dot_access_returns_empty() {
        let schema = SchemaMetadata::default();
        let results = complete_sql("SELECT foo.", 11, &schema);
        assert!(
            results.is_empty(),
            "dot on unknown table with no metadata should be empty"
        );
    }

    #[test]
    fn qualified_schema_dot_table_dot_column() {
        let schema = test_schema();
        let sql = "SELECT public.users.";
        let results = complete_sql(sql, sql.len(), &schema);
        assert!(results.iter().any(|r| r.label == "id"));
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(results.iter().any(|r| r.label == "email"));
    }

    #[test]
    fn qualified_schema_dot_table_partial_column() {
        let schema = test_schema();
        let results = complete_sql("SELECT public.users.em", 21, &schema);
        assert!(results.iter().any(|r| r.label == "email"));
        assert!(!results.iter().any(|r| r.label == "id"));
    }

    #[test]
    fn table_with_empty_columns_still_completes_no_columns() {
        let schema = SchemaMetadata {
            tables: vec![TableMeta {
                schema: Some("main".into()),
                name: "empty_table".into(),
                qualified_name: "main.empty_table".into(),
                columns: vec![],
            }],
            schemas: vec!["main".into()],
        };
        let results = complete_sql("SELECT empty_table.", 19, &schema);
        assert!(results.is_empty());
    }

    #[test]
    fn single_char_keyword_prefix() {
        let schema = SchemaMetadata::default();
        let results = complete_sql("S", 1, &schema);
        assert!(results.iter().any(|r| r.label == "SELECT"));
        assert!(results.iter().any(|r| r.label == "SET"));
        assert!(!results.iter().any(|r| r.label == "FROM"));
    }

    #[test]
    fn alias_dot_column_with_prefix_filter() {
        let schema = test_schema();
        let sql = "SELECT u.ag FROM users u";
        let results = complete_sql(sql, 10, &schema);
        assert!(results.iter().any(|r| r.label == "age"));
        assert!(!results.iter().any(|r| r.label == "id"));
    }

    #[test]
    fn multi_table_from_columns_all_available_in_select() {
        let schema = test_schema();
        let results = complete_sql("SELECT  FROM users, orders", 7, &schema);
        assert!(results.iter().any(|r| r.label == "name"));
        assert!(results.iter().any(|r| r.label == "amount"));
    }

    #[test]
    fn update_set_with_no_matching_table_returns_empty() {
        let schema = SchemaMetadata {
            tables: vec![TableMeta {
                schema: Some("public".into()),
                name: "products".into(),
                qualified_name: "public.products".into(),
                columns: vec!["id".into(), "price".into()],
            }],
            schemas: vec!["public".into()],
        };
        let results = complete_sql("UPDATE unknown_table SET ", 24, &schema);
        assert!(results.is_empty());
    }
}
