//! XML-configured, delimiter-aware member lists inside structural containers.

use regex::Regex;

use super::compiler::{OutlineBodyKind, OutlineMemberPlan};
use super::fsm::{ByteRange, StructuralEvent, StructuralEventKind};
use super::scan::parse_identifier_range;
use super::source::{OutlineSource, SyntaxSymbol};

struct MemberPatterns {
    prefix: Option<Regex>,
    name: Option<Regex>,
    line_skip: Option<Regex>,
    generic_open: Option<Regex>,
    generic_suffix: Option<Regex>,
}

impl MemberPatterns {
    fn new(rule: &OutlineMemberPlan) -> Self {
        let compile = |pattern: &Option<String>| {
            pattern
                .as_ref()
                .and_then(|pattern| Regex::new(&format!("\\A(?:{pattern})")).ok())
        };
        Self {
            prefix: compile(&rule.prefix_pattern),
            name: compile(&rule.name_pattern),
            line_skip: compile(&rule.line_skip_pattern),
            generic_open: compile(&rule.generic_open_pattern),
            generic_suffix: compile(&rule.generic_suffix_pattern),
        }
    }

    fn skip_line(
        &self,
        text: &str,
        source: &OutlineSource,
        cursor: usize,
        close: usize,
    ) -> Option<usize> {
        let pattern = self.line_skip.as_ref()?;
        let found = pattern
            .find(&text[cursor..close])
            .filter(|found| found.end() > 0)?;
        let line_start = text[..cursor]
            .rfind(['\r', '\n'])
            .map_or(0, |index| index + 1);
        if text[line_start..cursor].char_indices().any(|(offset, ch)| {
            !ch.is_whitespace()
                && (source.is_code(line_start + offset)
                    || source.is_literal_start(line_start + offset))
        }) {
            return None;
        }
        Some(cursor + found.end())
    }
}

pub(super) fn discover_members(
    text: &str,
    source: &OutlineSource,
    containers: &[StructuralEvent],
) -> Vec<StructuralEvent> {
    let mut members = Vec::new();
    for rule in &source.plan.members {
        if !containers.iter().any(|container| {
            let StructuralEventKind::Body { owner_kind, .. } = container.kind;
            owner_kind == rule.within
        }) {
            continue;
        }
        let patterns = MemberPatterns::new(rule);
        for container in containers {
            let StructuralEventKind::Body { owner_kind, .. } = container.kind;
            if owner_kind != rule.within {
                continue;
            }
            let Some(body) = container.body_range else {
                continue;
            };
            if !source.is_body_open(text, body.start) {
                continue;
            }
            let Some(close) = source.matching_delimiter(body.start) else {
                continue;
            };
            let Some(open) = source.next_token(body.start) else {
                continue;
            };
            let mut cursor = open.end;
            // Unlike the code-token index, this also stops at literal starts:
            // XML may allow a quoted name here. Comments remain invisible.
            while let Some(start) = next_member_start(text, source, cursor, close) {
                cursor = start;
                if let Some(end) = patterns.skip_line(text, source, cursor, close) {
                    cursor = end;
                    continue;
                }
                if at_terminator(text, source, cursor, rule) {
                    break;
                }
                if at_marker(text, source, cursor, &rule.separator) {
                    cursor += rule.separator.len();
                    continue;
                }
                // A configured prefix consumes the annotation name/marker, then
                // its optional balanced argument group. Repeated attributes work
                // without treating identifiers inside attributes as members.
                if let Some(found) = patterns
                    .prefix
                    .as_ref()
                    .and_then(|prefix| prefix.find(&text[cursor..close]))
                    .filter(|found| found.start() == 0 && found.end() > 0)
                {
                    cursor += found.end();
                    if let Some(group) = source
                        .next_token(cursor)
                        .filter(|group| group.start < close)
                        && source.is_delimiter_open(text, group.start)
                        && let Some(end) = source
                            .matching_delimiter(group.start)
                            .filter(|end| *end < close)
                    {
                        cursor = source.next_token(end).unwrap().end;
                    }
                    continue;
                }
                let Some((name, name_end)) = member_name(text, source, cursor, close, &patterns)
                else {
                    // Recover at the next list boundary instead of losing every
                    // subsequent member after an incomplete/unsupported name.
                    cursor = member_end(text, source, cursor, close, rule, &patterns);
                    continue;
                };
                if name.end > close {
                    break;
                }
                let boundary = member_end(text, source, name_end, close, rule, &patterns);
                let end = name_end + text[name_end..boundary].trim_end().len();
                members.push(StructuralEvent {
                    kind: StructuralEventKind::Body {
                        owner_kind: rule.node_kind,
                        body_kind: OutlineBodyKind::None,
                    },
                    keyword_range: ByteRange::new(name.start, name.start),
                    name_range: name,
                    signature_range: ByteRange::new(name.start, end),
                    // Payloads may contain declarations (e.g. constant-specific
                    // methods); retain ownership without adding callable entries.
                    body_range: (end > name_end).then_some(ByteRange::new(name_end, end)),
                });
                cursor = boundary;
            }
        }
    }
    members
}

fn next_member_start(
    text: &str,
    source: &OutlineSource,
    cursor: usize,
    close: usize,
) -> Option<usize> {
    text[cursor..close].char_indices().find_map(|(offset, ch)| {
        let offset = cursor + offset;
        (!ch.is_whitespace() && (source.is_code(offset) || source.is_literal_start(offset)))
            .then_some(offset)
    })
}

fn member_name(
    text: &str,
    source: &OutlineSource,
    cursor: usize,
    close: usize,
    patterns: &MemberPatterns,
) -> Option<(ByteRange, usize)> {
    if let Some(pattern) = patterns.name.as_ref()
        && let Some(captures) = pattern.captures(&text[cursor..close])
    {
        let name = pattern
            .capture_names()
            .flatten()
            .filter(|name| *name == "name" || name.starts_with("name_"))
            .find_map(|name| captures.name(name))?;
        return Some((
            ByteRange::new(cursor + name.start(), cursor + name.end()),
            cursor + captures.get(0)?.end(),
        ));
    }
    source
        .is_code(cursor)
        .then(|| parse_identifier_range(text, source, cursor))
        .flatten()
        .map(|name| (name, name.end))
}

fn member_end(
    text: &str,
    source: &OutlineSource,
    mut cursor: usize,
    close: usize,
    rule: &OutlineMemberPlan,
    patterns: &MemberPatterns,
) -> usize {
    while let Some(token) = source
        .next_token(cursor)
        .filter(|token| token.start < close)
    {
        if let Some(end) = patterns.skip_line(text, source, token.start, close) {
            cursor = end;
            continue;
        }
        if at_marker(text, source, token.start, &rule.separator)
            || at_terminator(text, source, token.start, rule)
        {
            return token.start;
        }
        if let Some(end) = generic_arguments_end(text, source, token.start, close, rule, patterns) {
            cursor = end;
        } else if source.is_delimiter_open(text, token.start) {
            let Some(end) = source
                .matching_delimiter(token.start)
                .filter(|end| *end < close)
            else {
                return close;
            };
            cursor = source.next_token(end).unwrap().end;
        } else {
            cursor = token.end;
        }
    }
    close
}

// Generic delimiters are contextual: treating every comparison/shift token as
// a bracket would swallow later members. XML identifies an opening expression
// and the syntax allowed after its balanced argument list.
fn generic_arguments_end(
    text: &str,
    source: &OutlineSource,
    start: usize,
    close: usize,
    rule: &OutlineMemberPlan,
    patterns: &MemberPatterns,
) -> Option<usize> {
    let found = patterns.generic_open.as_ref()?.find(&text[start..close])?;
    let mut cursor = start + found.end();
    let open = source.symbol_char(SyntaxSymbol::GenericsOpen)?;
    if !text[start..cursor].ends_with(open) {
        return None;
    }
    let mut depth = 1usize;
    while let Some(token) = source
        .next_token(cursor)
        .filter(|token| token.start < close)
    {
        if at_terminator(text, source, token.start, rule) {
            return None;
        }
        if source.is_delimiter_open(text, token.start) {
            let end = source
                .matching_delimiter(token.start)
                .filter(|end| *end < close)?;
            cursor = source.next_token(end)?.end;
            continue;
        }
        match source.symbol_text(token.text(text)) {
            SyntaxSymbol::GenericsOpen => depth += 1,
            SyntaxSymbol::GenericsClose => {
                depth -= 1;
                if depth == 0 {
                    let suffix = source
                        .next_token(token.end)
                        .filter(|next| next.start <= close)?;
                    return patterns
                        .generic_suffix
                        .as_ref()?
                        .is_match(&text[suffix.start..])
                        .then_some(token.end);
                }
            }
            // An assignment at this level belongs to a later member, not to a
            // generic argument. Parenthesized constant expressions were skipped.
            SyntaxSymbol::Assignment => return None,
            _ => {}
        }
        cursor = token.end;
    }
    None
}

fn at_marker(text: &str, source: &OutlineSource, offset: usize, marker: &str) -> bool {
    !marker.is_empty()
        && text[offset..].starts_with(marker)
        && source.is_code_range(offset, offset + marker.len())
}

fn at_terminator(
    text: &str,
    source: &OutlineSource,
    offset: usize,
    rule: &OutlineMemberPlan,
) -> bool {
    rule.terminator
        .as_deref()
        .is_some_and(|marker| at_marker(text, source, offset, marker))
}
