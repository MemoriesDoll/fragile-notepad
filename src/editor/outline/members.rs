//! XML-configured, delimiter-aware member lists inside structural containers.

use regex::Regex;

use super::compiler::{OutlineBodyKind, OutlineMemberPlan};
use super::fsm::{ByteRange, StructuralEvent, StructuralEventKind};
use super::scan::parse_identifier_range;
use super::source::OutlineSource;

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
        let prefix = rule
            .prefix_pattern
            .as_ref()
            .and_then(|pattern| Regex::new(&format!("^(?:{pattern})")).ok());
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
            while let Some(token) = source
                .next_token(cursor)
                .filter(|token| token.start < close)
            {
                cursor = token.start;
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
                if let Some(found) = prefix
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
                let Some(name) = parse_identifier_range(text, source, cursor) else {
                    break;
                };
                if name.end > close {
                    break;
                }
                let boundary = member_end(text, source, name.end, close, rule);
                let end = name.end + text[name.end..boundary].trim_end().len();
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
                    body_range: (end > name.end).then_some(ByteRange::new(name.end, end)),
                });
                cursor = boundary;
            }
        }
    }
    members
}

fn member_end(
    text: &str,
    source: &OutlineSource,
    mut cursor: usize,
    close: usize,
    rule: &OutlineMemberPlan,
) -> usize {
    while let Some(token) = source
        .next_token(cursor)
        .filter(|token| token.start < close)
    {
        if at_marker(text, source, token.start, &rule.separator)
            || at_terminator(text, source, token.start, rule)
        {
            return token.start;
        }
        if source.is_delimiter_open(text, token.start) {
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
