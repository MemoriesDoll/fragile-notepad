//! Function-list navigation over parsed outline entries.

use super::engine::OutlineEngine;
use super::registry::OutlineRegistry;
use super::types::{FunctionEntry, OutlineParseResult};
use crate::editor::buffer::EditorBuffer;
use crate::editor::position::{EditorPosition, EditorRange};

pub fn outline_for_syntax(buffer: &EditorBuffer, syntax_token: &str) -> Vec<FunctionEntry> {
    outline_for_syntax_with_registry(buffer, syntax_token, OutlineRegistry::shared())
}

fn outline_for_syntax_with_registry(
    buffer: &EditorBuffer,
    syntax_token: &str,
    registry: &OutlineRegistry,
) -> Vec<FunctionEntry> {
    if let Some(plan) = registry.plan_for_syntax(syntax_token) {
        let result: OutlineParseResult =
            OutlineEngine::new(plan, registry.registry_hash()).parse_buffer(buffer, syntax_token);
        return result.functions;
    }

    Vec::new()
}

pub fn containing_function(
    entries: &[FunctionEntry],
    position: EditorPosition,
) -> Option<&FunctionEntry> {
    entries
        .iter()
        .filter(|entry| range_contains_position(entry.range, position))
        .max_by_key(|entry| (entry.depth, entry.range.start))
}

pub fn next_function_after(
    entries: &[FunctionEntry],
    position: EditorPosition,
) -> Option<&FunctionEntry> {
    entries
        .iter()
        .filter(|entry| entry.range.start > position)
        .min_by_key(|entry| entry.range.start)
}

pub fn previous_function_before(
    entries: &[FunctionEntry],
    position: EditorPosition,
) -> Option<&FunctionEntry> {
    entries
        .iter()
        .filter(|entry| entry.range.start < position)
        .max_by_key(|entry| entry.range.start)
}

fn range_contains_position(range: EditorRange, position: EditorPosition) -> bool {
    range.start <= position && position < range.end
}
