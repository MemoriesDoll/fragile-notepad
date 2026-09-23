#[cfg(test)]
use super::fsm::{ByteRange, DeclarationEvent};
#[cfg(test)]
use super::fsm::{StructuralEvent, StructuralEventKind};
#[cfg(test)]
use crate::editor::position_for_byte_offset;

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
pub(super) fn editor_range(text: &str, range: ByteRange) -> Option<super::EditorRange> {
    Some(super::EditorRange::new(
        position_for_byte_offset(text, range.start)?,
        position_for_byte_offset(text, range.end)?,
    ))
}
