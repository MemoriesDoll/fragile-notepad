use super::OutlineRulePlan;
use super::adapters::{OutlineLanguageAdapter, is_rust_modifier_token};
use super::fsm::ByteRange;
use super::lexical::OutlineCodeMask;

pub(super) fn matching_code_brace(
    text: &str,
    mask: &OutlineCodeMask,
    open: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (relative_offset, ch) in text.get(open..)?.char_indices() {
        let offset = open + relative_offset;
        if !mask.is_code(offset) {
            continue;
        }

        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

pub(super) fn matching_end_keyword(
    text: &str,
    mask: &OutlineCodeMask,
    adapter: OutlineLanguageAdapter,
    start: usize,
) -> Option<usize> {
    let mut cursor = start;
    let mut depth = 1usize;

    while cursor < text.len() {
        let Some(token) = next_code_token(text, mask, cursor) else {
            break;
        };
        cursor = token.end;

        let token_text = token.text(text);
        if adapter.opens_end_keyword_block(token_text) {
            depth += 1;
        } else if token_text == "end" {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(token.end);
            }
        }
    }

    None
}

pub(super) fn signature_start(
    text: &str,
    mask: &OutlineCodeMask,
    adapter: OutlineLanguageAdapter,
    rule: &OutlineRulePlan,
    keyword_offset: usize,
) -> usize {
    if !adapter.is_rust() {
        return keyword_offset;
    }

    let mut start = keyword_offset;
    let mut cursor = keyword_offset;

    loop {
        let Some(token) = previous_code_token(text, mask, cursor) else {
            break;
        };

        if is_rust_modifier_token(token.text(text)) {
            start = token.start;
            cursor = token.start;
            continue;
        }

        if token.text(text) == ")" {
            let Some(open) = matching_code_paren_before(text, mask, token.start) else {
                break;
            };
            let Some(previous) = previous_code_token(text, mask, open) else {
                break;
            };
            if is_rust_modifier_token(previous.text(text)) {
                start = previous.start;
                cursor = previous.start;
                continue;
            }
        }

        break;
    }

    if rule.keyword.first().is_some_and(|keyword| keyword == "fn") {
        start
    } else {
        keyword_offset
    }
}

pub(super) fn matching_code_paren_before(
    text: &str,
    mask: &OutlineCodeMask,
    close: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text.get(..=close)?.char_indices().rev() {
        if !mask.is_code(offset) {
            continue;
        }

        match ch {
            ')' => depth += 1,
            '(' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

pub(super) fn matching_code_angle_before(
    text: &str,
    mask: &OutlineCodeMask,
    close: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text.get(..=close)?.char_indices().rev() {
        if !mask.is_code(offset) {
            continue;
        }

        match ch {
            '>' => depth += 1,
            '<' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

pub(super) fn matching_code_paren_after(
    text: &str,
    mask: &OutlineCodeMask,
    open: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (relative_offset, ch) in text.get(open..)?.char_indices() {
        let offset = open + relative_offset;
        if !mask.is_code(offset) {
            continue;
        }

        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

pub(super) fn find_next_code_char(
    text: &str,
    mask: &OutlineCodeMask,
    mut cursor: usize,
    target: char,
) -> Option<usize> {
    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        if mask.is_code(cursor) && ch == target {
            return Some(cursor);
        }
        cursor += ch.len_utf8();
    }

    None
}

pub(super) fn find_keyword_sequence(
    text: &str,
    mask: &OutlineCodeMask,
    cursor: usize,
    keywords: &[String],
) -> Option<usize> {
    find_keyword_sequence_in_range(text, mask, cursor, text.len(), keywords)
}

pub(super) fn find_keyword_sequence_in_range(
    text: &str,
    mask: &OutlineCodeMask,
    mut cursor: usize,
    end: usize,
    keywords: &[String],
) -> Option<usize> {
    let first = keywords.first()?;
    let end = end.min(text.len());

    while let Some(offset) = find_keyword_in_range(text, mask, cursor, end, first) {
        if keyword_sequence_end(text, mask, offset, keywords).is_some_and(|next| next <= end) {
            return Some(offset);
        }

        cursor = offset + first.len();
    }

    None
}

pub(super) fn keyword_sequence_end(
    text: &str,
    mask: &OutlineCodeMask,
    offset: usize,
    keywords: &[String],
) -> Option<usize> {
    let mut cursor = offset;

    for (index, keyword) in keywords.iter().enumerate() {
        if index > 0 {
            cursor = skip_non_code_whitespace(text, mask, cursor);
        }

        let end = cursor + keyword.len();
        if text.get(cursor..end) != Some(keyword.as_str())
            || !mask.is_code_range(cursor, end)
            || has_identifier_before(text, cursor)
            || has_identifier_after(text, end)
        {
            return None;
        }
        cursor = end;
    }

    Some(cursor)
}

pub(super) fn find_keyword_in_range(
    text: &str,
    mask: &OutlineCodeMask,
    mut cursor: usize,
    end: usize,
    keyword: &str,
) -> Option<usize> {
    let end = end.min(text.len());

    while cursor < end {
        let relative = text.get(cursor..end)?.find(keyword)?;
        let offset = cursor + relative;
        let keyword_end = offset + keyword.len();

        if mask.is_code_range(offset, keyword_end)
            && !has_identifier_before(text, offset)
            && !has_identifier_after(text, keyword_end)
        {
            return Some(offset);
        }

        cursor = keyword_end;
    }

    None
}

pub(super) fn previous_code_token(
    text: &str,
    mask: &OutlineCodeMask,
    before: usize,
) -> Option<CodeToken> {
    let mut cursor = previous_code_char(text, mask, before)?;
    let ch = text[cursor..].chars().next()?;

    if is_identifier_char(ch) || ch == '@' || ch == ')' {
        let end = cursor + ch.len_utf8();
        while let Some(previous) = previous_code_char(text, mask, cursor) {
            let Some(previous_ch) = text[previous..].chars().next() else {
                break;
            };
            if !(is_identifier_char(previous_ch)
                || previous_ch == '@'
                || matches!(previous_ch, '(' | ')' | ':'))
            {
                break;
            }
            cursor = previous;
        }
        return Some(CodeToken { start: cursor, end });
    }

    Some(CodeToken {
        start: cursor,
        end: cursor + ch.len_utf8(),
    })
}

pub(super) fn next_code_token(
    text: &str,
    mask: &OutlineCodeMask,
    mut cursor: usize,
) -> Option<CodeToken> {
    while cursor < text.len() {
        if mask.is_code(cursor) {
            let ch = text[cursor..].chars().next()?;
            if !ch.is_whitespace() {
                break;
            }
        }
        cursor += next_char_len(text, cursor);
    }

    let ch = text[cursor..].chars().next()?;
    if is_identifier_char(ch) || ch == '@' {
        let start = cursor;
        cursor += ch.len_utf8();
        while let Some(next) = text.get(cursor..).and_then(|tail| tail.chars().next()) {
            if !mask.is_code(cursor) || !(is_identifier_char(next) || next == '@') {
                break;
            }
            cursor += next.len_utf8();
        }
        return Some(CodeToken { start, end: cursor });
    }

    Some(CodeToken {
        start: cursor,
        end: cursor + ch.len_utf8(),
    })
}

pub(super) fn previous_contiguous_code_token(
    text: &str,
    mask: &OutlineCodeMask,
    before: usize,
) -> Option<CodeToken> {
    let mut cursor = skip_code_whitespace_before(text, mask, before);
    if cursor == 0 {
        return None;
    }
    let end = cursor;

    while let Some(offset) = previous_code_char_including_whitespace(text, mask, cursor) {
        let Some(ch) = text[offset..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            break;
        }
        cursor = offset;
    }

    (cursor < end).then_some(CodeToken { start: cursor, end })
}

pub(super) fn previous_code_char(
    text: &str,
    mask: &OutlineCodeMask,
    before: usize,
) -> Option<usize> {
    text.get(..before)?
        .char_indices()
        .rev()
        .find(|(offset, ch)| mask.is_code(*offset) && !ch.is_whitespace())
        .map(|(offset, _)| offset)
}

pub(super) fn skip_non_code_whitespace(
    text: &str,
    mask: &OutlineCodeMask,
    mut cursor: usize,
) -> usize {
    while cursor < text.len() {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        if mask.is_code(cursor) && !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }

    cursor
}

pub(super) fn skip_code_whitespace_before(
    text: &str,
    mask: &OutlineCodeMask,
    before: usize,
) -> usize {
    let mut cursor = before;

    while let Some(offset) = previous_code_char_including_whitespace(text, mask, cursor) {
        let Some(ch) = text[offset..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cursor = offset;
    }

    cursor
}

pub(super) fn previous_code_char_including_whitespace(
    text: &str,
    mask: &OutlineCodeMask,
    before: usize,
) -> Option<usize> {
    text.get(..before)?
        .char_indices()
        .rev()
        .find(|(offset, _)| mask.is_code(*offset))
        .map(|(offset, _)| offset)
}

pub(super) fn parse_identifier_range(text: &str, start: usize) -> Option<ByteRange> {
    let mut cursor = start;
    let mut name_start = start;

    if text[start..].starts_with("r#") {
        cursor += 2;
        name_start = cursor;
    }

    let first = text[cursor..].chars().next()?;
    if !is_identifier_start(first) {
        return None;
    }
    cursor += first.len_utf8();

    while let Some(ch) = text.get(cursor..).and_then(|tail| tail.chars().next()) {
        if !is_identifier_char(ch) {
            break;
        }
        cursor += ch.len_utf8();
    }

    Some(ByteRange::new(name_start, cursor))
}

pub(super) fn has_identifier_before(text: &str, offset: usize) -> bool {
    if offset == 0 || offset == text.len() {
        return false;
    }

    if !text.is_char_boundary(offset) {
        return true;
    }

    text[..offset]
        .chars()
        .next_back()
        .is_some_and(is_identifier_char)
}

pub(super) fn has_identifier_after(text: &str, offset: usize) -> bool {
    if offset == text.len() {
        return false;
    }

    if !text.is_char_boundary(offset) {
        return true;
    }

    text[offset..]
        .chars()
        .next()
        .is_some_and(is_identifier_char)
}

pub(super) fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

pub(super) fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

pub(super) fn line_start_offset(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .and_then(|prefix| prefix.rfind(['\n', '\r']))
        .map(|index| {
            if text[index..].starts_with("\r\n") || text[index..].starts_with("\n\r") {
                index + 2
            } else {
                index + 1
            }
        })
        .unwrap_or(0)
}

pub(super) fn next_line_start_offset(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }

    let mut cursor = offset;
    while cursor < text.len() {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        cursor += ch.len_utf8();
        if ch == '\r' || ch == '\n' {
            if cursor < text.len() {
                let next = text[cursor..].chars().next();
                if matches!((ch, next), ('\r', Some('\n')) | ('\n', Some('\r'))) {
                    cursor += 1;
                }
            }
            return cursor;
        }
    }

    text.len()
}

pub(super) fn line_end_offset(text: &str, offset: usize) -> usize {
    text.get(offset..)
        .and_then(|tail| {
            tail.char_indices()
                .find(|(_, ch)| *ch == '\r' || *ch == '\n')
                .map(|(index, _)| offset + index)
        })
        .unwrap_or(text.len())
}

pub(super) fn line_ending_len_before(text: &str, offset: usize) -> usize {
    if offset >= 2 {
        let pair = &text[offset - 2..offset];
        if pair == "\r\n" || pair == "\n\r" {
            return 2;
        }
    }
    if offset >= 1
        && text
            .as_bytes()
            .get(offset - 1)
            .is_some_and(|byte| *byte == b'\r' || *byte == b'\n')
    {
        return 1;
    }
    0
}

pub(super) fn indentation_before(text: &str, offset: usize) -> usize {
    let line_start = line_start_offset(text, offset);
    text.get(line_start..offset)
        .unwrap_or("")
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum()
}

pub(super) fn leading_whitespace_len(line: &str) -> usize {
    line.chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum()
}

pub(super) fn next_char_len(text: &str, index: usize) -> usize {
    text[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CodeToken {
    pub start: usize,
    pub end: usize,
}

impl CodeToken {
    pub(super) fn text<'a>(self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }
}
