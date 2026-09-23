//! Lexical shielding. Offsets always refer to UTF-8 bytes in the original source.
use super::{OutlineLexicalPlan, OutlineStringPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutlineCodeMask {
    code: Vec<bool>,
    literal_starts: Vec<usize>,
}

impl OutlineCodeMask {
    pub(super) fn new(text: &str, plan: &OutlineLexicalPlan) -> Self {
        let mut mask = Self {
            code: vec![true; text.len()],
            literal_starts: Vec::new(),
        };
        let mut cursor = 0;
        while cursor < text.len() {
            let tail = &text[cursor..];
            let mut literal = false;
            let end = if let Some(open) = plan
                .line_comments
                .iter()
                .filter(|open| !open.is_empty() && tail.starts_with(open.as_str()))
                .max_by_key(|open| open.len())
            {
                Some(line_end(text, cursor + open.len()))
            } else if let Some(comment) = plan
                .block_comments
                .iter()
                .filter(|comment| {
                    !comment.open.is_empty()
                        && !comment.close.is_empty()
                        && tail.starts_with(&comment.open)
                })
                .max_by_key(|comment| comment.open.len())
            {
                Some(block_comment_end(text, cursor, comment))
            } else if let Some(end) = raw_string_end(text, cursor, plan) {
                literal = true;
                Some(end)
            } else if let Some((_, end)) = plan
                .strings
                .iter()
                .filter(|string| {
                    !string.open.is_empty()
                        && !string.close.is_empty()
                        && tail.starts_with(&string.open)
                        && (!string.single_quote_literals
                            || single_quoted_literal_starts(text, cursor, string))
                })
                .filter_map(|string| {
                    let end = quoted_string_end(text, cursor, string);
                    (!string.requires_closing_on_line || end.is_some()).then_some((string, end))
                })
                .max_by_key(|(string, _)| string.open.len())
            {
                literal = true;
                Some(end.unwrap_or(text.len()))
            } else {
                None
            };

            if let Some(end) = end {
                if literal {
                    mask.literal_starts.push(cursor);
                }
                mask.code[cursor..end].fill(false);
                cursor = end;
            } else {
                cursor += char_len(text, cursor);
            }
        }
        mask
    }

    pub(super) fn is_code(&self, offset: usize) -> bool {
        self.code.get(offset).copied().unwrap_or(false)
    }

    pub(super) fn is_code_range(&self, start: usize, end: usize) -> bool {
        start < end
            && self
                .code
                .get(start..end)
                .is_some_and(|code| code.iter().all(|code| *code))
    }

    pub(super) fn is_literal_start(&self, offset: usize) -> bool {
        self.literal_starts.binary_search(&offset).is_ok()
    }
}

fn block_comment_end(text: &str, start: usize, comment: &super::OutlineBlockCommentPlan) -> usize {
    let mut cursor = start + comment.open.len();
    let mut depth = 1;
    while cursor < text.len() {
        if text[cursor..].starts_with(&comment.close) {
            cursor += comment.close.len();
            depth -= 1;
            if depth == 0 {
                return cursor;
            }
        } else if comment.nested && text[cursor..].starts_with(&comment.open) {
            depth += 1;
            cursor += comment.open.len();
        } else {
            cursor += char_len(text, cursor);
        }
    }
    text.len()
}

fn quoted_string_end(text: &str, start: usize, string: &OutlineStringPlan) -> Option<usize> {
    let mut cursor = start + string.open.len();
    let end = if string.requires_closing_on_line {
        line_end(text, cursor)
    } else {
        text.len()
    };
    while cursor < end {
        if let Some(escape) = string
            .escape
            .as_deref()
            .filter(|escape| !escape.is_empty() && text[cursor..end].starts_with(escape))
        {
            cursor += escape.len();
            if cursor < end {
                cursor += char_len(text, cursor);
            }
        } else if text[cursor..end].starts_with(&string.close) {
            return Some(cursor + string.close.len());
        } else {
            cursor += char_len(text, cursor);
        }
    }
    None
}

fn raw_string_end(text: &str, start: usize, plan: &OutlineLexicalPlan) -> Option<usize> {
    if text[..start].chars().next_back().is_some_and(|ch| {
        plan.word_character_extra.contains(ch)
            || if plan.unicode_word_characters {
                ch.is_alphanumeric()
            } else {
                ch.is_ascii_alphanumeric()
            }
    }) {
        return None;
    }
    for raw in &plan.raw_strings {
        let Some(prefix) = raw
            .prefixes
            .iter()
            .filter(|prefix| !prefix.is_empty() && text[start..].starts_with(prefix.as_str()))
            .max_by_key(|prefix| prefix.len())
        else {
            continue;
        };
        if raw.open.is_empty() || raw.close.is_empty() {
            continue;
        }
        let delimiter_start = start + prefix.len();
        let mut cursor = delimiter_start;
        if let Some(repeat) = raw.repeat.as_deref().filter(|repeat| !repeat.is_empty()) {
            while text[cursor..].starts_with(repeat) {
                cursor += repeat.len();
            }
        } else {
            while cursor < text.len() && !text[cursor..].starts_with(&raw.open) {
                let ch = text[cursor..].chars().next()?;
                if ch.is_whitespace()
                    || raw.forbidden_delimiter_characters.contains(ch)
                    || raw
                        .max_delimiter_length
                        .is_some_and(|limit| cursor - delimiter_start >= limit)
                {
                    break;
                }
                cursor += ch.len_utf8();
            }
        }
        if !text[cursor..].starts_with(&raw.open) {
            continue;
        }
        let delimiter = &text[delimiter_start..cursor];
        if raw
            .max_delimiter_length
            .is_some_and(|limit| delimiter.len() > limit)
        {
            continue;
        }
        let body = cursor + raw.open.len();
        let close = format!("{}{}{}", raw.close, delimiter, raw.suffix);
        return Some(
            text[body..]
                .find(&close)
                .map_or(text.len(), |offset| body + offset + close.len()),
        );
    }
    None
}

fn single_quoted_literal_starts(text: &str, index: usize, string: &OutlineStringPlan) -> bool {
    let start = index + string.open.len();
    if let Some(escape) = string
        .escape
        .as_deref()
        .filter(|escape| !escape.is_empty() && text[start..].starts_with(escape))
    {
        return text[start + escape.len()..line_end(text, start)].contains(&string.close);
    }
    text.get(start + char_len(text, start)..)
        .is_some_and(|tail| tail.starts_with(&string.close))
}

fn line_end(text: &str, start: usize) -> usize {
    text[start..]
        .find(['\r', '\n'])
        .map_or(text.len(), |offset| start + offset)
}

fn char_len(text: &str, start: usize) -> usize {
    text[start..].chars().next().map_or(1, char::len_utf8)
}
