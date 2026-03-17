use models::TablePreviewSource;

#[derive(Clone)]
pub(super) struct EditableSelectPlan {
    pub(super) source: TablePreviewSource,
    pub(super) select_list: String,
    pub(super) tail: String,
}

pub(super) fn editable_select_plan(sql: &str) -> Option<EditableSelectPlan> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("select ") || lower.starts_with("select distinct ") {
        return None;
    }

    let from_idx = find_top_level_keyword(&lower, " from ")?;
    let select_list = trimmed[6..from_idx].trim().to_string();
    if !is_simple_projection(&select_list) {
        return None;
    }

    let after_from = trimmed[from_idx + " from ".len()..].trim();
    let (table_ref, tail) = split_table_ref(after_from)?;
    let tail = strip_limit_offset(tail.trim());
    let tail_lower = tail.to_lowercase();
    if tail_lower.contains(" join ")
        || tail_lower.contains(" union ")
        || tail_lower.contains(" intersect ")
        || tail_lower.contains(" except ")
        || tail_lower.contains(" group by ")
        || tail_lower.contains(" having ")
    {
        return None;
    }

    let (schema, table_name) = split_qualified_name(&table_ref);
    Some(EditableSelectPlan {
        source: TablePreviewSource {
            schema,
            table_name,
            qualified_name: table_ref,
        },
        select_list,
        tail,
    })
}

fn find_top_level_keyword(sql: &str, needle: &str) -> Option<usize> {
    let bytes = sql.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut depth = 0i32;
    let mut idx = 0usize;

    while idx + needle_bytes.len() <= bytes.len() {
        let ch = bytes[idx] as char;
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => depth += 1,
            ')' if !in_single && !in_double && depth > 0 => depth -= 1,
            _ => {}
        }

        if !in_single
            && !in_double
            && depth == 0
            && &bytes[idx..idx + needle_bytes.len()] == needle_bytes
        {
            return Some(idx);
        }
        idx += 1;
    }

    None
}

fn split_table_ref(after_from: &str) -> Option<(String, String)> {
    let mut in_double = false;
    let mut depth = 0i32;

    for (idx, ch) in after_from.char_indices() {
        match ch {
            '"' => in_double = !in_double,
            '(' if !in_double => depth += 1,
            ')' if !in_double && depth > 0 => depth -= 1,
            ' ' | '\n' | '\t' if !in_double && depth == 0 => {
                let table = after_from[..idx].trim().to_string();
                let tail = after_from[idx..].trim().to_string();
                return Some((table, tail));
            }
            _ => {}
        }
    }

    if after_from.is_empty() {
        None
    } else {
        Some((after_from.trim().to_string(), String::new()))
    }
}

fn split_qualified_name(table_ref: &str) -> (Option<String>, String) {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_double = false;

    for (idx, ch) in table_ref.char_indices() {
        match ch {
            '"' => in_double = !in_double,
            '.' if !in_double => {
                parts.push(table_ref[start..idx].trim().to_string());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(table_ref[start..].trim().to_string());

    match parts.as_slice() {
        [table] => (None, unquote_identifier(table)),
        [schema, table] => (Some(unquote_identifier(schema)), unquote_identifier(table)),
        _ => (None, table_ref.to_string()),
    }
}

fn unquote_identifier(identifier: &str) -> String {
    let trimmed = identifier.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].replace("\"\"", "\"")
    } else {
        trimmed.to_string()
    }
}

fn is_simple_projection(select_list: &str) -> bool {
    let trimmed = select_list.trim();
    if trimmed == "*" || trimmed.ends_with(".*") {
        return true;
    }

    split_projection_items(trimmed)
        .into_iter()
        .all(|item| is_simple_column_ref(item.trim()))
}

fn split_projection_items(select_list: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut depth = 0i32;

    for (idx, ch) in select_list.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => depth += 1,
            ')' if !in_single && !in_double && depth > 0 => depth -= 1,
            ',' if !in_single && !in_double && depth == 0 => {
                parts.push(&select_list[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&select_list[start..]);
    parts
}

fn is_simple_column_ref(item: &str) -> bool {
    let lowered = item.to_lowercase();
    if lowered.contains(" as ")
        || item.contains('(')
        || item.contains(')')
        || item.contains('+')
        || item.contains('-')
        || item.contains('*')
        || item.contains('/')
    {
        return false;
    }

    item.split('.').all(|part| {
        let part = part.trim();
        if part.is_empty() {
            return false;
        }
        if part.starts_with('"') && part.ends_with('"') {
            return true;
        }
        part.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn strip_limit_offset(tail: &str) -> String {
    let lower = tail.to_lowercase();
    let limit_pos = find_top_level_keyword(&lower, " limit ");
    let offset_pos = find_top_level_keyword(&lower, " offset ");

    match (limit_pos, offset_pos) {
        (Some(limit), Some(offset)) => tail[..limit.min(offset)].trim().to_string(),
        (Some(limit), None) => tail[..limit].trim().to_string(),
        (None, Some(offset)) => tail[..offset].trim().to_string(),
        (None, None) => tail.trim().to_string(),
    }
}
