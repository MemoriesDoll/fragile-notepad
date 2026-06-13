use crate::core::DocumentId;
use crate::editor::position::EditorRange;

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    Function,
    Method,
    Declaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionEntry {
    pub name: String,
    pub kind: FunctionKind,
    pub range: EditorRange,
    pub body_range: Option<EditorRange>,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineNodeKind {
    Module,
    Namespace,
    Class,
    Interface,
    Trait,
    Impl,
    Method,
    Constructor,
    Function,
    Declaration,
    Tag,
    Section,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineNode {
    pub name: String,
    pub kind: OutlineNodeKind,
    pub range: EditorRange,
    pub body_range: Option<EditorRange>,
    pub depth: usize,
    pub children: Vec<OutlineNode>,
}

impl OutlineNode {
    pub fn new(
        name: impl Into<String>,
        kind: OutlineNodeKind,
        range: EditorRange,
        body_range: Option<EditorRange>,
        depth: usize,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            range,
            body_range,
            depth,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutlineTree {
    pub roots: Vec<OutlineNode>,
}

impl OutlineTree {
    pub fn new(roots: Vec<OutlineNode>) -> Self {
        Self { roots }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineDiagnostic {
    pub severity: OutlineDiagnosticSeverity,
    pub message: String,
    pub range: Option<EditorRange>,
}

impl OutlineDiagnostic {
    pub fn new(
        severity: OutlineDiagnosticSeverity,
        message: impl Into<String>,
        range: Option<EditorRange>,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            range,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutlineParseRequest {
    pub document_id: DocumentId,
    pub text: Arc<String>,
    pub syntax_token: String,
    pub revision: u64,
    pub registry_hash: u64,
}

impl OutlineParseRequest {
    pub fn new(
        document_id: DocumentId,
        text: Arc<String>,
        syntax_token: impl Into<String>,
        revision: u64,
        registry_hash: u64,
    ) -> Self {
        Self {
            document_id,
            text,
            syntax_token: syntax_token.into(),
            revision,
            registry_hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineParseResult {
    pub document_id: DocumentId,
    pub revision: u64,
    pub syntax_token: String,
    pub registry_hash: u64,
    pub tree: OutlineTree,
    pub functions: Vec<FunctionEntry>,
    pub diagnostics: Vec<OutlineDiagnostic>,
}

impl OutlineParseResult {
    pub fn new(
        document_id: DocumentId,
        revision: u64,
        syntax_token: impl Into<String>,
        registry_hash: u64,
        tree: OutlineTree,
        functions: Vec<FunctionEntry>,
        diagnostics: Vec<OutlineDiagnostic>,
    ) -> Self {
        Self {
            document_id,
            revision,
            syntax_token: syntax_token.into(),
            registry_hash,
            tree,
            functions,
            diagnostics,
        }
    }
}
