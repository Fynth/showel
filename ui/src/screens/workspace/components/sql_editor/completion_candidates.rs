use super::{
    completion_alias::Aliases,
    completion_context::SqlContext,
    completion_keywords::{SQL_FUNCTIONS, SQL_KEYWORDS},
    completion_tokenizer::extract_current_token,
    CompletionItem, CompletionKind, SchemaMetadata,
};

pub fn get_columns_for_ref(
    ref_name: &str,
    aliases: &Aliases,
    schema: &SchemaMetadata,
) -> Vec<String> {
    if let Some(info) = aliases.get(&ref_name.to_lowercase()) {
        return get_columns_for_qualified(&info.qualified_name, schema);
    }
    let mut cols = get_columns_for_qualified(ref_name, schema);
    if cols.is_empty() {
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

pub fn find_update_table(tokens: &[super::completion_tokenizer::SqlToken]) -> Option<String> {
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

pub fn try_dot_completion(
    text_before_cursor: &str,
    cursor: usize,
    aliases: &Aliases,
    schema: &SchemaMetadata,
) -> Option<Vec<CompletionItem>> {
    let (partial, _) = extract_current_token(text_before_cursor, cursor);

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

pub fn build_candidates(
    context: &SqlContext,
    schema: &SchemaMetadata,
    aliases: &Aliases,
    from_tables: &[String],
    pre_tokens: &[super::completion_tokenizer::SqlToken],
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

#[derive(Clone, Debug)]
struct ScoredItem {
    item: CompletionItem,
    score: i32,
    match_start: usize,
}

fn kind_priority(kind: &CompletionKind) -> u8 {
    match kind {
        CompletionKind::Column | CompletionKind::Alias => 0,
        CompletionKind::Table | CompletionKind::View | CompletionKind::Schema => 1,
        CompletionKind::Function => 2,
        CompletionKind::Keyword => 3,
    }
}

fn match_score(label: &str, prefix: &str) -> Option<(i32, usize)> {
    let label_lower = label.to_lowercase();
    let prefix_lower = prefix.to_lowercase();

    if label_lower == prefix_lower {
        return Some((1000, 0));
    }

    if label_lower.starts_with(&prefix_lower) {
        return Some((900 - prefix_lower.len() as i32, 0));
    }

    if let Some(pos) = label_lower.find(&prefix_lower) {
        let base_score = 500 - pos as i32 - (prefix_lower.len() as i32 / 2);
        return Some((base_score, pos));
    }

    let mut score = 0i32;
    let mut match_positions = Vec::new();
    let mut prefix_chars = prefix_lower.chars().peekable();

    for (i, ch) in label_lower.char_indices() {
        if Some(&ch) == prefix_chars.peek() {
            match_positions.push(i);
            score += 10;
            prefix_chars.next();
            if prefix_chars.peek().is_none() {
                break;
            }
        }
    }

    if match_positions.len() == prefix_lower.len() {
        let avg_pos: usize = match_positions.iter().sum::<usize>() / match_positions.len();
        return Some((score - avg_pos as i32 / 2, avg_pos));
    }

    None
}

pub fn filter_and_rank(items: Vec<CompletionItem>, prefix: &str) -> Vec<CompletionItem> {
    const MAX_RESULTS: usize = 100;

    if prefix.is_empty() {
        let mut sorted = items;
        sorted.sort_by(|a, b| {
            kind_priority(&a.kind)
                .cmp(&kind_priority(&b.kind))
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
        });
        sorted.dedup_by(|a, b| a.label.eq_ignore_ascii_case(&b.label));
        sorted.truncate(MAX_RESULTS);
        return sorted;
    }

    let mut scored: Vec<ScoredItem> = Vec::new();

    for item in items {
        if let Some((score, match_start)) = match_score(&item.label, prefix) {
            let kind_bonus = -(kind_priority(&item.kind) as i32 * 20);
            scored.push(ScoredItem {
                item,
                score: score + kind_bonus,
                match_start,
            });
        }
    }

    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.match_start.cmp(&b.match_start))
            .then_with(|| {
                a.item
                    .label
                    .to_lowercase()
                    .cmp(&b.item.label.to_lowercase())
            })
    });

    let mut result: Vec<CompletionItem> = scored.into_iter().map(|s| s.item).collect();
    result.dedup_by(|a, b| a.label.eq_ignore_ascii_case(&b.label));
    result.truncate(MAX_RESULTS);
    result
}

pub fn default_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    push_keywords(&mut items);
    push_functions(&mut items);
    filter_and_rank(items, prefix)
}
