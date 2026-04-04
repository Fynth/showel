use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct EditorSelection {
    pub(super) start: usize,
    pub(super) end: usize,
}

impl EditorSelection {
    pub(super) fn collapsed(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn clamped(self, sql: &str) -> Self {
        let len = sql.len();
        Self {
            start: clamp_to_char_boundary(sql, self.start.min(len)),
            end: clamp_to_char_boundary(sql, self.end.min(len)),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn clamp_to_char_boundary(sql: &str, index: usize) -> usize {
    let mut index = index.min(sql.len());
    while index > 0 && !sql.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn current_token_range(sql: &str, selection: EditorSelection) -> std::ops::Range<usize> {
    let selection = selection.clamped(sql);
    let start = selection.start.min(selection.end);
    let end = selection.start.max(selection.end);

    if start != end {
        return start..end;
    }

    let cursor = end;
    let mut range_start = cursor;
    for (index, ch) in sql[..cursor].char_indices().rev() {
        if is_token_boundary(ch) {
            break;
        }
        range_start = index;
    }

    let mut range_end = cursor;
    for (offset, ch) in sql[cursor..].char_indices() {
        if is_token_boundary(ch) {
            break;
        }
        range_end = cursor + offset + ch.len_utf8();
    }

    range_start..range_end
}

#[cfg_attr(not(test), allow(dead_code))]
fn is_token_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            ',' | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '+'
                | '-'
                | '*'
                | '/'
                | '='
                | '<'
                | '>'
                | ':'
        )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn clean_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| {
            ch.is_whitespace() || matches!(ch, ',' | ';' | '(' | ')' | '\n' | '\r')
        })
        .to_string()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '"' | '\'' | '`'))
        .collect::<String>()
        .trim()
        .to_ascii_lowercase()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn apply_suggestion(
    sql: &str,
    selection: EditorSelection,
    replacement: &str,
) -> (String, usize) {
    let range = current_token_range(sql, selection);
    let mut next_sql = String::with_capacity(sql.len() + replacement.len());
    next_sql.push_str(&sql[..range.start]);
    next_sql.push_str(replacement);
    next_sql.push_str(&sql[range.end..]);
    (next_sql, range.start + replacement.len())
}

pub(super) fn sync_editor_selection(
    mut editor_selection: Signal<EditorSelection>,
    editor_id: &'static str,
) {
    spawn(async move {
        let Ok((start, end)) = document::eval(&selection_query_script(editor_id))
            .join::<(usize, usize)>()
            .await
        else {
            return;
        };

        editor_selection.set(EditorSelection { start, end });
    });
}

fn selection_query_script(editor_id: &str) -> String {
    format!(
        r#"
        (() => {{
            const editor = document.getElementById({editor_id:?});
            if (!editor) {{
                return [0, 0];
            }}
            const toByteIndex = (value, utf16Offset) =>
                new TextEncoder().encode(value.slice(0, utf16Offset)).length;
            const value = editor.value ?? "";
            const start = editor.selectionStart ?? value.length ?? 0;
            const end = editor.selectionEnd ?? start;
            return [
                toByteIndex(value, start),
                toByteIndex(value, end)
            ];
        }})()
        "#
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn set_editor_selection_script(editor_id: &str, position: usize) -> String {
    format!(
        r#"
        (() => {{
            const editor = document.getElementById({editor_id:?});
            if (!editor) {{
                return false;
            }}
            const encoder = new TextEncoder();
            const value = editor.value ?? "";
            let utf16Position = 0;
            let byteOffset = 0;
            for (const ch of value) {{
                const nextByteOffset = byteOffset + encoder.encode(ch).length;
                if (nextByteOffset > {position}) {{
                    break;
                }}
                byteOffset = nextByteOffset;
                utf16Position += ch.length;
            }}
            editor.focus();
            editor.setSelectionRange(utf16Position, utf16Position);
            return true;
        }})()
        "#
    )
}

#[cfg(test)]
mod tests {
    use super::{EditorSelection, current_token_range};

    #[test]
    fn selection_clamps_invalid_utf8_offsets_to_char_boundaries() {
        let sql = "select * from пользователи";
        let selection = EditorSelection { start: 15, end: 15 }.clamped(sql);
        assert!(sql.is_char_boundary(selection.start));
        assert_eq!(selection.start, selection.end);
    }

    #[test]
    fn current_token_range_handles_multibyte_identifiers() {
        let sql = "select * from пользователи";
        let cursor = sql.find("ль").unwrap() + 1;
        let range = current_token_range(
            sql,
            EditorSelection {
                start: cursor,
                end: cursor,
            },
        );
        assert_eq!(&sql[range], "пользователи");
    }
}
