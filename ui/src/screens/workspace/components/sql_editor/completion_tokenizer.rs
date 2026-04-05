use super::completion_keywords::{build_function_set, build_keyword_set};

const BOUNDARY_CHARS: &[char] = &[
    ' ', '\t', '\n', '\r', '(', ')', ',', ';', '+', '-', '*', '/', '=', '<', '>', ':', '[', ']',
    '{', '}', '!', '&', '|', '^', '~', '%', '#', '@', '`', '?',
];

#[inline]
pub fn is_boundary_char(ch: char) -> bool {
    BOUNDARY_CHARS.contains(&ch)
}

pub fn is_in_string_literal(text: &str, pos: usize) -> bool {
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < pos {
        if bytes[i] == b'\'' {
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

pub fn is_in_line_comment(text: &str, pos: usize) -> bool {
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

pub fn is_in_block_comment(text: &str, pos: usize) -> bool {
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

pub fn is_in_non_code_region(text: &str, pos: usize) -> bool {
    is_in_string_literal(text, pos)
        || is_in_line_comment(text, pos)
        || is_in_block_comment(text, pos)
}

pub fn extract_current_token(text: &str, pos: usize) -> (String, usize) {
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

#[derive(Debug, Clone)]
pub struct SqlToken {
    pub text: String,
    pub original: String,
    pub is_keyword: bool,
}

pub fn tokenize_sql(text: &str) -> Vec<SqlToken> {
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

        if in_string {
            if ch == '\'' {
                if i + 1 < len && chars[i + 1] == '\'' {
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }

        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if in_block_comment {
            if ch == '*' && i + 1 < len && chars[i + 1] == '/' {
                in_block_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if ch == '\'' {
            flush!();
            in_string = true;
            i += 1;
            continue;
        }

        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            flush!();
            in_line_comment = true;
            i += 2;
            continue;
        }

        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            flush!();
            in_block_comment = true;
            i += 2;
            continue;
        }

        if ch == '.' {
            flush!();
            tokens.push(SqlToken {
                text: ".".to_string(),
                original: ".".to_string(),
                is_keyword: false,
            });
            i += 1;
            continue;
        }

        if is_boundary_char(ch) {
            flush!();
            if ch == ',' {
                tokens.push(SqlToken {
                    text: ",".to_string(),
                    original: ",".to_string(),
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
