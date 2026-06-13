use super::{OutlineLexicalPlan, OutlineStringPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutlineCodeMask {
    code: Vec<bool>,
}

impl OutlineCodeMask {
    pub(super) fn new(text: &str, plan: &OutlineLexicalPlan) -> Self {
        let mut mask = Self {
            code: vec![false; text.len()],
        };
        let mut syntax = LexicalState::Code;
        let mut index = 0;

        while index < text.len() {
            match syntax.clone() {
                LexicalState::Code => {
                    if let Some(comment) = plan
                        .line_comments
                        .iter()
                        .find(|comment| text[index..].starts_with(comment.as_str()))
                    {
                        index = line_comment_end(text, index + comment.len());
                        continue;
                    }

                    if let Some(comment) = plan
                        .block_comments
                        .iter()
                        .find(|comment| text[index..].starts_with(&comment.open))
                    {
                        mark_range(&mut mask.code, index, index + comment.open.len(), false);
                        syntax = LexicalState::BlockComment {
                            close: comment.close.clone(),
                        };
                        index += comment.open.len();
                        continue;
                    }

                    if plan.raw_strings.iter().any(|kind| kind == "rust") {
                        if let Some((hashes, raw_string_len)) = rust_raw_string_start(text, index) {
                            mark_range(&mut mask.code, index, index + raw_string_len, false);
                            syntax = LexicalState::RustRawString { hashes };
                            index += raw_string_len;
                            continue;
                        }
                    }

                    if let Some(delimiter) = quoted_string_delimiter(text, index, plan) {
                        mark_range(&mut mask.code, index, index + delimiter.open.len(), false);
                        syntax = LexicalState::QuotedString {
                            close: delimiter.close.clone(),
                            escape: delimiter.escape,
                            escaped: false,
                        };
                        index += delimiter.open.len();
                        continue;
                    }

                    let len = next_char_len(text, index);
                    mark_range(&mut mask.code, index, index + len, true);
                    index += len;
                }
                LexicalState::BlockComment { close } => {
                    let len = if text[index..].starts_with(&close) {
                        syntax = LexicalState::Code;
                        close.len()
                    } else {
                        next_char_len(text, index)
                    };
                    mark_range(&mut mask.code, index, index + len, false);
                    index += len;
                }
                LexicalState::QuotedString {
                    close,
                    escape,
                    escaped,
                } => {
                    let len = next_char_len(text, index);
                    if escaped {
                        syntax = LexicalState::QuotedString {
                            close,
                            escape,
                            escaped: false,
                        };
                    } else if escape
                        .as_deref()
                        .is_some_and(|escape| text[index..].starts_with(escape))
                    {
                        syntax = LexicalState::QuotedString {
                            close,
                            escape,
                            escaped: true,
                        };
                    } else if text[index..].starts_with(&close) {
                        syntax = LexicalState::Code;
                    }

                    mark_range(&mut mask.code, index, index + len, false);
                    index += len;
                }
                LexicalState::RustRawString { hashes } => {
                    if let Some(end) = rust_raw_string_end(text, index, hashes) {
                        mark_range(&mut mask.code, index, end, false);
                        syntax = LexicalState::Code;
                        index = end;
                    } else {
                        let len = next_char_len(text, index);
                        mark_range(&mut mask.code, index, index + len, false);
                        index += len;
                    }
                }
            }
        }

        mask
    }

    pub(super) fn is_code(&self, offset: usize) -> bool {
        self.code.get(offset).copied().unwrap_or(false)
    }

    pub(super) fn is_code_range(&self, start: usize, end: usize) -> bool {
        start < end
            && end <= self.code.len()
            && self.code[start..end].iter().all(|is_code| *is_code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LexicalState {
    Code,
    BlockComment {
        close: String,
    },
    QuotedString {
        close: String,
        escape: Option<String>,
        escaped: bool,
    },
    RustRawString {
        hashes: usize,
    },
}

fn quoted_string_delimiter(
    text: &str,
    index: usize,
    plan: &OutlineLexicalPlan,
) -> Option<OutlineStringPlan> {
    plan.strings
        .iter()
        .find(|hint| {
            text[index..].starts_with(&hint.open)
                && (!hint.single_quote_literals || single_quoted_literal_starts(text, index))
                && (!hint.requires_closing_on_line
                    || line_tail(text, index + hint.open.len()).contains(&hint.close))
        })
        .cloned()
}

fn single_quoted_literal_starts(text: &str, index: usize) -> bool {
    let Some(after_quote) = index.checked_add(1) else {
        return false;
    };
    let Some(first) = text.get(after_quote..).and_then(|tail| tail.chars().next()) else {
        return false;
    };

    let close = if first == '\\' {
        let mut cursor = after_quote + first.len_utf8();
        while cursor < text.len() {
            let Some(ch) = text[cursor..].chars().next() else {
                return false;
            };
            cursor += ch.len_utf8();
            if ch == '\'' {
                return true;
            }
            if ch == '\r' || ch == '\n' {
                return false;
            }
        }
        return false;
    } else {
        after_quote + first.len_utf8()
    };

    text.get(close..).is_some_and(|tail| tail.starts_with('\''))
}

fn rust_raw_string_start(text: &str, index: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(index) != Some(&b'r') {
        return None;
    }

    if index > 0
        && bytes
            .get(index - 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        return None;
    }

    let mut cursor = index + 1;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }

    (bytes.get(cursor) == Some(&b'"')).then_some((cursor - index - 1, cursor - index + 1))
}

fn rust_raw_string_end(text: &str, index: usize, hashes: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(index) != Some(&b'"') {
        return None;
    }

    let end = index + hashes + 1;
    (end <= text.len() && bytes.get(index + 1..end)?.iter().all(|byte| *byte == b'#'))
        .then_some(end)
}

fn line_comment_end(text: &str, index: usize) -> usize {
    let mut cursor = index;
    while cursor < text.len() {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        if ch == '\r' || ch == '\n' {
            break;
        }
        cursor += ch.len_utf8();
    }

    cursor
}

fn line_tail(text: &str, index: usize) -> &str {
    let end = line_comment_end(text, index);
    text.get(index..end).unwrap_or("")
}

fn mark_range(mask: &mut [bool], start: usize, end: usize, is_code: bool) {
    for entry in mask.iter_mut().take(end).skip(start) {
        *entry = is_code;
    }
}

fn next_char_len(text: &str, index: usize) -> usize {
    text[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}
