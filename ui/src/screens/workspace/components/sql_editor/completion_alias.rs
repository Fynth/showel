use super::{
    completion_context::is_clause_keyword, completion_tokenizer::SqlToken, SchemaMetadata,
};

#[derive(Debug, Clone)]
pub struct AliasInfo {
    pub table_name: String,
    pub qualified_name: String,
}

pub type Aliases = std::collections::HashMap<String, AliasInfo>;

pub fn parse_aliases(tokens: &[SqlToken], schema: &SchemaMetadata) -> Aliases {
    let mut aliases = Aliases::new();
    let mut i = 0;

    while i < tokens.len() {
        let tok = &tokens[i];
        let is_from_trigger = tok.is_keyword && tok.text == "FROM";
        let is_join_trigger = tok.is_keyword && tok.text == "JOIN";
        let is_comma_trigger = tok.text == ",";

        if is_from_trigger || is_join_trigger || is_comma_trigger {
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

            if j >= tokens.len() {
                i += 1;
                continue;
            }

            let table_ref = super::completion_context::read_table_ref(tokens, &mut j);
            if table_ref.is_empty() {
                i += 1;
                continue;
            }

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

pub fn find_qualified_name(table_ref: &str, schema: &SchemaMetadata) -> String {
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
