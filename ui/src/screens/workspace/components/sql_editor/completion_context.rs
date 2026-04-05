use super::completion_tokenizer::SqlToken;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqlContext {
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

pub fn detect_context(tokens: &[SqlToken]) -> SqlContext {
    if tokens.is_empty() {
        return SqlContext::Default;
    }

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

pub fn is_clause_keyword(text: &str) -> bool {
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

pub fn collect_from_tables(tokens: &[SqlToken]) -> Vec<String> {
    let mut tables = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_from = tokens[i].is_keyword && tokens[i].text == "FROM";
        let is_join = tokens[i].is_keyword && tokens[i].text == "JOIN";
        if is_from || is_join {
            let mut j = i + 1;
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

pub fn read_table_ref(tokens: &[SqlToken], j: &mut usize) -> String {
    if *j >= tokens.len() || tokens[*j].text == "." {
        return String::new();
    }
    if tokens[*j].is_keyword {
        let is_qualified = *j + 1 < tokens.len() && tokens[*j + 1].text == ".";
        if !is_qualified {
            return String::new();
        }
    }
    let mut parts = vec![tokens[*j].original.clone()];
    *j += 1;
    while *j + 1 < tokens.len() && tokens[*j].text == "." {
        *j += 1;
        parts.push(tokens[*j].original.clone());
        *j += 1;
    }
    parts.join(".")
}
