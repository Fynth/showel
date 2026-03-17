use models::{ExplorerNode, ExplorerNodeKind, QueryOutput, QueryTabState, TablePreviewSource};
use std::collections::{BTreeSet, HashMap, HashSet};

use super::selection::{EditorSelection, clean_token, current_token_range, normalize_identifier};

const SQL_KEYWORDS: [&str; 58] = [
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "LEFT JOIN",
    "RIGHT JOIN",
    "INNER JOIN",
    "OUTER JOIN",
    "GROUP BY",
    "ORDER BY",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "INSERT INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE TABLE",
    "ALTER TABLE",
    "DROP TABLE",
    "WITH",
    "AS",
    "DISTINCT",
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "AND",
    "OR",
    "NOT",
    "IN",
    "EXISTS",
    "IS NULL",
    "IS NOT NULL",
    "LIKE",
    "ILIKE",
    "BETWEEN",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "UNION",
    "ALL",
    "EXCEPT",
    "INTERSECT",
    "DESC",
    "DESCRIBE",
    "SHOW TABLES",
    "SHOW DATABASES",
    "EXPLAIN",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "TRUNCATE",
    "RETURNING",
];

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SqlInlineCompletion {
    pub(super) replacement: String,
    pub(super) suffix: String,
    pub(super) cursor_position: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct AutocompleteRelation {
    pub(super) schema: Option<String>,
    pub(super) name: String,
    pub(super) qualified_name: String,
    pub(super) kind_label: &'static str,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct AutocompleteCatalog {
    pub(super) schemas: Vec<String>,
    pub(super) relations: Vec<AutocompleteRelation>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct CompletionContext {
    pub(super) raw_token: String,
    pub(super) qualifier: Option<String>,
    pub(super) qualifier_normalized: Option<String>,
    pub(super) prefix: String,
    pub(super) prefix_normalized: String,
    pub(super) previous_word_normalized: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct RelationBinding {
    pub(super) alias: String,
    pub(super) relation: AutocompleteRelation,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SqlAutocompleteSuggestion {
    pub(super) label: String,
    pub(super) replacement: String,
    pub(super) kind_label: &'static str,
    pub(super) detail: String,
    pub(super) score: u8,
}

pub(super) fn flatten_catalog(nodes: &[ExplorerNode]) -> AutocompleteCatalog {
    let mut schemas = BTreeSet::new();
    let mut relations = Vec::new();
    flatten_catalog_inner(nodes, &mut schemas, &mut relations);

    AutocompleteCatalog {
        schemas: schemas.into_iter().collect(),
        relations,
    }
}

fn flatten_catalog_inner(
    nodes: &[ExplorerNode],
    schemas: &mut BTreeSet<String>,
    relations: &mut Vec<AutocompleteRelation>,
) {
    for node in nodes {
        match node.kind {
            ExplorerNodeKind::Schema => {
                schemas.insert(node.name.clone());
                flatten_catalog_inner(&node.children, schemas, relations);
            }
            ExplorerNodeKind::Table | ExplorerNodeKind::View => {
                relations.push(AutocompleteRelation {
                    schema: node.schema.clone(),
                    name: node.name.clone(),
                    qualified_name: node.qualified_name.clone(),
                    kind_label: if node.kind == ExplorerNodeKind::View {
                        "View"
                    } else {
                        "Table"
                    },
                });
            }
        }
    }
}

pub(super) fn completion_context(sql: &str, selection: EditorSelection) -> CompletionContext {
    let token_range = current_token_range(sql, selection);
    let raw_token = sql[token_range.start..token_range.end].to_string();
    let raw_token_parts = raw_token.clone();
    let previous_word_normalized = previous_word_normalized(sql, token_range.start);

    if let Some((qualifier, prefix)) = raw_token_parts.rsplit_once('.') {
        CompletionContext {
            raw_token,
            qualifier: Some(qualifier.to_string()),
            qualifier_normalized: Some(normalize_identifier(qualifier)),
            prefix: prefix.to_string(),
            prefix_normalized: normalize_identifier(prefix),
            previous_word_normalized,
        }
    } else {
        CompletionContext {
            prefix_normalized: normalize_identifier(&raw_token),
            prefix: raw_token.clone(),
            raw_token,
            previous_word_normalized,
            ..CompletionContext::default()
        }
    }
}

fn previous_word_normalized(sql: &str, token_start: usize) -> Option<String> {
    sql[..token_start]
        .split_whitespace()
        .last()
        .map(clean_token)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
}

pub(super) fn should_open_autocomplete(context: &CompletionContext) -> bool {
    context.qualifier.is_some()
        || !context.prefix.trim().is_empty()
        || context
            .previous_word_normalized
            .as_deref()
            .is_some_and(is_relation_keyword)
}

pub(super) fn extract_relation_bindings(
    sql: &str,
    catalog: &AutocompleteCatalog,
) -> Vec<RelationBinding> {
    let tokens = sql
        .split_whitespace()
        .map(clean_token)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();

    for index in 0..tokens.len() {
        let token = tokens[index].to_ascii_lowercase();
        if !matches!(token.as_str(), "from" | "join" | "update" | "into") {
            continue;
        }

        let Some(relation_token) = tokens.get(index + 1) else {
            continue;
        };
        if relation_token.eq_ignore_ascii_case("select") {
            continue;
        }

        let Some(relation) = find_relation(catalog, relation_token) else {
            continue;
        };

        let alias_index = if tokens
            .get(index + 2)
            .is_some_and(|next| next.eq_ignore_ascii_case("as"))
        {
            index + 3
        } else {
            index + 2
        };

        let Some(alias) = tokens.get(alias_index) else {
            continue;
        };
        let alias_normalized = normalize_identifier(alias);
        if alias_normalized.is_empty() || is_clause_boundary(&alias_normalized) {
            continue;
        }

        bindings.push(RelationBinding {
            alias: alias.to_string(),
            relation,
        });
    }

    bindings
}

pub(super) fn relations_to_prefetch(
    active_tab: Option<&QueryTabState>,
    catalog: &AutocompleteCatalog,
    relation_bindings: &[RelationBinding],
    context: &CompletionContext,
) -> Vec<AutocompleteRelation> {
    let mut relations = Vec::new();

    if let Some(tab) = active_tab
        && let Some(source) = tab.preview_source.as_ref()
        && let Some(relation) = relation_for_source(catalog, source)
    {
        relations.push(relation);
    }

    relations.extend(
        relation_bindings
            .iter()
            .map(|binding| binding.relation.clone()),
    );

    if let Some(qualifier) = context.qualifier_normalized.as_deref() {
        if let Some(binding) = relation_bindings
            .iter()
            .find(|binding| normalize_identifier(&binding.alias) == qualifier)
        {
            relations.push(binding.relation.clone());
        }

        if let Some(relation) = catalog
            .relations
            .iter()
            .find(|relation| relation_matches_qualifier(relation, qualifier))
            .cloned()
        {
            relations.push(relation);
        }
    }

    let mut seen = HashSet::new();
    relations
        .into_iter()
        .filter(|relation| seen.insert(relation.qualified_name.clone()))
        .collect()
}

pub(super) fn build_suggestions(
    sql: &str,
    selection: EditorSelection,
    active_tab: Option<&QueryTabState>,
    catalog: &AutocompleteCatalog,
    relation_bindings: &[RelationBinding],
    column_cache: &HashMap<String, Vec<String>>,
    force_open: bool,
) -> Vec<SqlAutocompleteSuggestion> {
    let context = completion_context(sql, selection);
    if !force_open && !should_open_autocomplete(&context) {
        return Vec::new();
    }

    let mut suggestions = Vec::new();
    let mut seen = HashSet::new();

    if let Some(qualifier) = context.qualifier.as_deref() {
        let qualifier_normalized = context.qualifier_normalized.as_deref().unwrap_or_default();

        for schema in catalog
            .schemas
            .iter()
            .filter(|schema| normalize_identifier(schema) == qualifier_normalized)
        {
            for relation in catalog.relations.iter().filter(|relation| {
                relation.schema.as_deref() == Some(schema.as_str())
                    && prefix_matches(&relation.name, &context.prefix_normalized)
            }) {
                push_suggestion(
                    &mut suggestions,
                    &mut seen,
                    SqlAutocompleteSuggestion {
                        label: relation.name.clone(),
                        replacement: format!("{}.{}", qualifier, relation_insert_segment(relation)),
                        kind_label: relation.kind_label,
                        detail: schema.clone(),
                        score: suggestion_score(&relation.name, &context.prefix_normalized),
                    },
                );
            }
        }

        for binding in relation_bindings
            .iter()
            .filter(|binding| normalize_identifier(&binding.alias) == qualifier_normalized)
        {
            push_column_suggestions(
                &mut suggestions,
                &mut seen,
                qualifier,
                &binding.relation,
                column_cache,
                &context.prefix_normalized,
            );
        }

        if let Some(relation) = catalog
            .relations
            .iter()
            .find(|relation| relation_matches_qualifier(relation, qualifier_normalized))
        {
            push_column_suggestions(
                &mut suggestions,
                &mut seen,
                qualifier,
                relation,
                column_cache,
                &context.prefix_normalized,
            );
        }
    } else {
        let relations_only = context
            .previous_word_normalized
            .as_deref()
            .is_some_and(is_relation_keyword);
        let columns_allowed = !relations_only;

        if !relations_only {
            for keyword in SQL_KEYWORDS {
                if prefix_matches(keyword, &context.prefix_normalized) {
                    push_suggestion(
                        &mut suggestions,
                        &mut seen,
                        SqlAutocompleteSuggestion {
                            label: keyword.to_string(),
                            replacement: keyword.to_string(),
                            kind_label: "Keyword",
                            detail: "SQL".to_string(),
                            score: suggestion_score(keyword, &context.prefix_normalized),
                        },
                    );
                }
            }
        }

        for schema in catalog
            .schemas
            .iter()
            .filter(|schema| prefix_matches(schema, &context.prefix_normalized))
        {
            push_suggestion(
                &mut suggestions,
                &mut seen,
                SqlAutocompleteSuggestion {
                    label: schema.clone(),
                    replacement: schema.clone(),
                    kind_label: "Schema",
                    detail: "Database schema".to_string(),
                    score: suggestion_score(schema, &context.prefix_normalized),
                },
            );
        }

        for relation in catalog
            .relations
            .iter()
            .filter(|relation| prefix_matches(&relation.name, &context.prefix_normalized))
        {
            push_suggestion(
                &mut suggestions,
                &mut seen,
                SqlAutocompleteSuggestion {
                    label: relation.name.clone(),
                    replacement: relation.qualified_name.clone(),
                    kind_label: relation.kind_label,
                    detail: relation
                        .schema
                        .clone()
                        .unwrap_or_else(|| "default schema".to_string()),
                    score: suggestion_score(&relation.name, &context.prefix_normalized),
                },
            );
        }

        if columns_allowed {
            if let Some(tab) = active_tab {
                push_active_result_columns(
                    &mut suggestions,
                    &mut seen,
                    tab,
                    &context.prefix_normalized,
                );
            }

            for binding in relation_bindings {
                push_alias_prefixed_columns(
                    &mut suggestions,
                    &mut seen,
                    binding,
                    column_cache,
                    &context.prefix_normalized,
                );
            }
        }
    }

    suggestions.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| left.kind_label.cmp(right.kind_label))
            .then_with(|| left.label.cmp(&right.label))
    });
    suggestions
}

fn push_active_result_columns(
    suggestions: &mut Vec<SqlAutocompleteSuggestion>,
    seen: &mut HashSet<String>,
    tab: &QueryTabState,
    prefix: &str,
) {
    let Some(QueryOutput::Table(page)) = tab.result.as_ref() else {
        return;
    };

    for column in page
        .columns
        .iter()
        .filter(|column| prefix_matches(column, prefix))
    {
        push_suggestion(
            suggestions,
            seen,
            SqlAutocompleteSuggestion {
                label: column.clone(),
                replacement: column.clone(),
                kind_label: "Column",
                detail: "Current result".to_string(),
                score: suggestion_score(column, prefix),
            },
        );
    }
}

fn push_alias_prefixed_columns(
    suggestions: &mut Vec<SqlAutocompleteSuggestion>,
    seen: &mut HashSet<String>,
    binding: &RelationBinding,
    column_cache: &HashMap<String, Vec<String>>,
    prefix: &str,
) {
    let Some(columns) = column_cache.get(&binding.relation.qualified_name) else {
        return;
    };

    for column in columns
        .iter()
        .filter(|column| prefix_matches(column, prefix))
    {
        push_suggestion(
            suggestions,
            seen,
            SqlAutocompleteSuggestion {
                label: format!("{}.{}", binding.alias, column),
                replacement: format!("{}.{}", binding.alias, column),
                kind_label: "Column",
                detail: binding.relation.name.clone(),
                score: suggestion_score(column, prefix),
            },
        );
    }
}

fn push_column_suggestions(
    suggestions: &mut Vec<SqlAutocompleteSuggestion>,
    seen: &mut HashSet<String>,
    qualifier: &str,
    relation: &AutocompleteRelation,
    column_cache: &HashMap<String, Vec<String>>,
    prefix: &str,
) {
    let Some(columns) = column_cache.get(&relation.qualified_name) else {
        return;
    };

    for column in columns
        .iter()
        .filter(|column| prefix_matches(column, prefix))
    {
        push_suggestion(
            suggestions,
            seen,
            SqlAutocompleteSuggestion {
                label: format!("{}.{}", qualifier, column),
                replacement: format!("{}.{}", qualifier, column),
                kind_label: "Column",
                detail: relation.name.clone(),
                score: suggestion_score(column, prefix),
            },
        );
    }
}

fn push_suggestion(
    suggestions: &mut Vec<SqlAutocompleteSuggestion>,
    seen: &mut HashSet<String>,
    suggestion: SqlAutocompleteSuggestion,
) {
    let key = format!("{}::{}", suggestion.kind_label, suggestion.replacement);
    if seen.insert(key) {
        suggestions.push(suggestion);
    }
}

fn relation_for_source(
    catalog: &AutocompleteCatalog,
    source: &TablePreviewSource,
) -> Option<AutocompleteRelation> {
    catalog
        .relations
        .iter()
        .find(|relation| relation.qualified_name == source.qualified_name)
        .cloned()
        .or_else(|| {
            Some(AutocompleteRelation {
                schema: source.schema.clone(),
                name: source.table_name.clone(),
                qualified_name: source.qualified_name.clone(),
                kind_label: "Table",
            })
        })
}

fn find_relation(catalog: &AutocompleteCatalog, token: &str) -> Option<AutocompleteRelation> {
    let normalized = normalize_identifier(token);
    catalog
        .relations
        .iter()
        .find(|relation| relation_matches_qualifier(relation, &normalized))
        .cloned()
}

fn relation_matches_qualifier(relation: &AutocompleteRelation, qualifier_normalized: &str) -> bool {
    if qualifier_normalized.is_empty() {
        return false;
    }

    normalize_identifier(&relation.name) == qualifier_normalized
        || normalize_identifier(&relation.qualified_name) == qualifier_normalized
        || relation
            .schema
            .as_ref()
            .map(|schema| normalize_identifier(&format!("{schema}.{}", relation.name)))
            .is_some_and(|value| value == qualifier_normalized)
}

fn relation_insert_segment(relation: &AutocompleteRelation) -> String {
    relation
        .qualified_name
        .rsplit_once('.')
        .map(|(_, tail)| tail.to_string())
        .unwrap_or_else(|| relation.qualified_name.clone())
}

fn is_relation_keyword(token: &str) -> bool {
    matches!(
        token,
        "from" | "join" | "update" | "into" | "table" | "desc" | "describe"
    )
}

fn is_clause_boundary(token: &str) -> bool {
    matches!(
        token,
        "on" | "where"
            | "group"
            | "order"
            | "limit"
            | "offset"
            | "join"
            | "left"
            | "right"
            | "inner"
            | "outer"
            | "set"
            | "values"
            | "returning"
            | "and"
            | "or"
    )
}

fn suggestion_score(candidate: &str, prefix: &str) -> u8 {
    if prefix.is_empty() {
        return 3;
    }

    let normalized = normalize_identifier(candidate);
    if normalized == prefix {
        0
    } else if normalized.starts_with(prefix) {
        1
    } else if normalized.contains(prefix) {
        2
    } else {
        3
    }
}

fn prefix_matches(candidate: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }

    let normalized = normalize_identifier(candidate);
    normalized.starts_with(prefix) || normalized.contains(prefix)
}

pub(super) fn build_inline_completion(
    sql: &str,
    selection: EditorSelection,
    context: &CompletionContext,
    suggestion: Option<&SqlAutocompleteSuggestion>,
) -> Option<SqlInlineCompletion> {
    let suggestion = suggestion?;
    let selection = selection.clamped(sql);
    if selection.start != selection.end {
        return None;
    }

    let token_range = current_token_range(sql, selection);
    if token_range.end != selection.end {
        return None;
    }

    let typed = &sql[token_range.start..token_range.end];
    if typed.trim().is_empty() {
        return None;
    }

    let replacement = resolved_replacement(suggestion, context);
    if replacement == typed {
        return None;
    }

    let typed_chars = typed.chars().count();
    let replacement_chars = replacement.chars().collect::<Vec<_>>();
    if replacement_chars.len() <= typed_chars {
        return None;
    }

    if !normalize_identifier(&replacement).starts_with(&normalize_identifier(typed)) {
        return None;
    }

    let literal_prefix = replacement_chars
        .iter()
        .take(typed_chars)
        .collect::<String>();
    if literal_prefix != typed {
        return None;
    }

    let suffix = replacement_chars
        .iter()
        .skip(typed_chars)
        .collect::<String>();
    if suffix.is_empty() {
        return None;
    }
    let cursor_position = token_range.start + literal_prefix.len() + suffix.len();

    Some(SqlInlineCompletion {
        replacement,
        suffix,
        cursor_position,
    })
}

pub(super) fn resolved_replacement(
    suggestion: &SqlAutocompleteSuggestion,
    context: &CompletionContext,
) -> String {
    if suggestion.kind_label != "Keyword" {
        let raw_normalized = normalize_identifier(&context.raw_token);
        let replacement_normalized = normalize_identifier(&suggestion.replacement);
        let label_normalized = normalize_identifier(&suggestion.label);

        if context.qualifier.is_none()
            && !raw_normalized.is_empty()
            && !replacement_normalized.starts_with(&raw_normalized)
            && label_normalized.starts_with(&raw_normalized)
        {
            return suggestion.label.clone();
        }

        return suggestion.replacement.clone();
    }

    if context.raw_token.chars().any(|ch| ch.is_ascii_alphabetic())
        && context
            .raw_token
            .chars()
            .all(|ch| !ch.is_ascii_alphabetic() || ch.is_ascii_lowercase())
    {
        suggestion.replacement.to_ascii_lowercase()
    } else {
        suggestion.replacement.clone()
    }
}
