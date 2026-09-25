use super::position::{EditorPosition, EditorRange};

mod callable_statements;
mod cascade;
mod compiler;
mod diagnostics;
mod engine;
mod fsm;
mod lexical;
mod members;
mod navigation;
mod projection;
mod registry;
mod scan;
mod schema;
mod service;
mod source;
mod structure;
mod structure_support;
mod types;

pub use compiler::{
    OutlineBlockCommentPlan, OutlineBodyKind, OutlineBodyPlan, OutlineCallablePlan,
    OutlineLexicalPlan, OutlineMemberPlan, OutlineNameCapture, OutlinePlan, OutlineRulePlan,
    OutlineScanMode, OutlineStringPlan, OutlineStructurePlan,
};
pub use engine::OutlineEngine;
pub use navigation::{
    containing_function, next_function_after, outline_for_syntax, previous_function_before,
};
pub use registry::OutlineRegistry;
pub use schema::{
    RawBlockComment, RawBody, RawDelimiter, RawFamily, RawLanguage, RawLexical, RawMemberRule,
    RawOutlineSchema, RawRawString, RawRule, RawString, RawSyntaxToken, RawUseFamily,
    parse_outline_schema,
};
pub use service::{
    OutlineSnapshotMetadata, OutlineState, OutlineStatus, outline_registry_hash,
    outline_request_for_document, parse_outline_request, parse_outline_snapshot,
};
pub use types::{
    FunctionEntry, FunctionKind, OutlineDiagnostic, OutlineDiagnosticSeverity, OutlineNode,
    OutlineNodeKind, OutlineParseRequest, OutlineParseResult, OutlineTree,
};
