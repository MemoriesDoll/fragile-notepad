use super::buffer::EditorBuffer;
use super::position::{EditorPosition, EditorRange};

mod adapters;
mod callable_statements;
mod cascade;
mod compiler;
mod diagnostics;
mod engine;
mod fsm;
mod lexical;
mod projection;
mod registry;
mod scan;
mod schema;
mod service;
mod structure;
mod structure_support;
mod types;

pub use compiler::{
    OutlineBlockCommentPlan, OutlineBodyKind, OutlineBodyPlan, OutlineCallablePlan,
    OutlineLexicalPlan, OutlineNameCapture, OutlinePlan, OutlineRulePlan, OutlineScanMode,
    OutlineStringPlan, OutlineStructurePlan,
};
pub use engine::OutlineEngine;
pub use registry::OutlineRegistry;
pub use schema::{
    RawBlockComment, RawBody, RawFamily, RawLanguage, RawLexical, RawOutlineSchema, RawRule,
    RawString, RawUseFamily, parse_outline_schema,
};
pub use service::{
    OutlineSnapshotMetadata, OutlineState, OutlineStatus, outline_registry_hash,
    outline_request_for_document, parse_outline_request, parse_outline_snapshot,
};
pub use types::{
    FunctionEntry, FunctionKind, OutlineDiagnostic, OutlineDiagnosticSeverity, OutlineNode,
    OutlineNodeKind, OutlineParseRequest, OutlineParseResult, OutlineTree,
};

pub fn outline_for_syntax(buffer: &EditorBuffer, syntax_token: &str) -> Vec<FunctionEntry> {
    outline_for_syntax_with_registry(buffer, syntax_token, OutlineRegistry::shared())
}

pub(super) fn outline_for_syntax_with_registry(
    buffer: &EditorBuffer,
    syntax_token: &str,
    registry: &OutlineRegistry,
) -> Vec<FunctionEntry> {
    if let Some(plan) = registry.plan_for_syntax(syntax_token) {
        let result =
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
