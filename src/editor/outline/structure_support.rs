use super::fsm::{ByteRange, DeclarationEvent, StructuralEvent, StructuralEventKind};
use super::scan::{
    indentation_before, leading_whitespace_len, line_ending_len_before, line_start_offset,
};
use crate::editor::position_for_byte_offset;

pub(super) fn containing_container<'a>(
    containers: &'a [StructuralEvent],
    offset: usize,
    method_containers: &[super::OutlineNodeKind],
) -> Option<&'a StructuralEvent> {
    containers
        .iter()
        .filter(|event| {
            let StructuralEventKind::Body { owner_kind, .. } = event.kind;
            method_containers.contains(&owner_kind)
                && event
                    .body_range
                    .is_some_and(|range| range.start <= offset && offset < range.end)
        })
        .max_by_key(|event| event.body_range.map(|range| range.start).unwrap_or(0))
}

pub(super) fn container_depth(containers: &[StructuralEvent], offset: usize) -> usize {
    containers
        .iter()
        .filter(|event| {
            event
                .body_range
                .is_some_and(|range| range.start <= offset && offset < range.end)
        })
        .count()
}

pub(super) fn declaration_depth(declarations: &[DeclarationEvent], offset: usize) -> usize {
    declarations
        .iter()
        .filter(|event| {
            event
                .body_range
                .is_some_and(|range| range.start <= offset && offset < range.end)
                && event.signature_range.start != offset
        })
        .count()
}

pub(super) fn indent_depth_before(text: &str, offset: usize) -> usize {
    let current_indent = indentation_before(text, offset);
    let current_line_start = line_start_offset(text, offset);
    let mut cursor = current_line_start;
    let mut depths = Vec::new();

    while cursor > 0 {
        let previous_line_end = cursor.saturating_sub(line_ending_len_before(text, cursor));
        let previous_line_start = line_start_offset(text, previous_line_end);
        let line = text
            .get(previous_line_start..previous_line_end)
            .unwrap_or("");

        if !line.trim().is_empty() {
            let indent =
                indentation_before(text, previous_line_start + leading_whitespace_len(line));
            if indent < current_indent && !depths.contains(&indent) {
                depths.push(indent);
            }
        }

        cursor = previous_line_start;
    }

    depths.len()
}

pub(super) fn editor_range(text: &str, range: ByteRange) -> Option<super::EditorRange> {
    Some(super::EditorRange::new(
        position_for_byte_offset(text, range.start)?,
        position_for_byte_offset(text, range.end)?,
    ))
}
