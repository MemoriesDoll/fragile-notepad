use std::sync::Arc;

use crate::core::{Document, DocumentId};

use super::super::buffer::EditorBuffer;
use super::{
    FunctionEntry, OutlineDiagnostic, OutlineEngine, OutlineParseRequest, OutlineParseResult,
    OutlineRegistry, OutlineTree, outline_for_syntax_with_registry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSnapshotMetadata {
    pub document_id: DocumentId,
    pub revision: u64,
    pub syntax_token: String,
    pub registry_hash: u64,
}

impl OutlineSnapshotMetadata {
    pub fn new(
        document_id: DocumentId,
        revision: u64,
        syntax_token: impl Into<String>,
        registry_hash: u64,
    ) -> Self {
        Self {
            document_id,
            revision,
            syntax_token: syntax_token.into(),
            registry_hash,
        }
    }

    pub fn from_document(document: &Document, registry_hash: u64) -> Self {
        Self::new(
            document.id,
            document.revision(),
            document.syntax_token.clone(),
            registry_hash,
        )
    }

    pub fn from_request(request: &OutlineParseRequest) -> Self {
        Self::new(
            request.document_id,
            request.revision,
            request.syntax_token.clone(),
            request.registry_hash,
        )
    }

    pub fn from_result(result: &OutlineParseResult) -> Self {
        Self::new(
            result.document_id,
            result.revision,
            result.syntax_token.clone(),
            result.registry_hash,
        )
    }

    pub fn matches_document(&self, document: &Document, registry_hash: u64) -> bool {
        self.document_id == document.id
            && self.revision == document.revision()
            && self.syntax_token == document.syntax_token
            && self.registry_hash == registry_hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineStatus {
    Pending,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineState {
    pub metadata: OutlineSnapshotMetadata,
    pub status: OutlineStatus,
    pub tree: OutlineTree,
    pub functions: Vec<FunctionEntry>,
    pub diagnostics: Vec<OutlineDiagnostic>,
}

impl OutlineState {
    pub fn pending(request: &OutlineParseRequest) -> Self {
        Self {
            metadata: OutlineSnapshotMetadata::from_request(request),
            status: OutlineStatus::Pending,
            tree: OutlineTree::default(),
            functions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn pending_metadata(metadata: OutlineSnapshotMetadata) -> Self {
        Self {
            metadata,
            status: OutlineStatus::Pending,
            tree: OutlineTree::default(),
            functions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn ready(result: OutlineParseResult) -> Self {
        Self {
            metadata: OutlineSnapshotMetadata::from_result(&result),
            status: OutlineStatus::Ready,
            tree: result.tree,
            functions: result.functions,
            diagnostics: result.diagnostics,
        }
    }

    pub fn ready_from_functions(
        metadata: OutlineSnapshotMetadata,
        functions: Vec<FunctionEntry>,
    ) -> Self {
        Self {
            metadata,
            status: OutlineStatus::Ready,
            tree: OutlineTree::default(),
            functions,
            diagnostics: Vec::new(),
        }
    }

    pub fn matches_metadata(&self, metadata: &OutlineSnapshotMetadata) -> bool {
        self.metadata == *metadata
    }

    pub fn current_functions(
        &self,
        metadata: &OutlineSnapshotMetadata,
    ) -> Option<&[FunctionEntry]> {
        (self.status == OutlineStatus::Ready && self.matches_metadata(metadata))
            .then_some(self.functions.as_slice())
    }
}

pub fn outline_registry_hash() -> u64 {
    OutlineRegistry::shared().registry_hash()
}

pub fn outline_request_for_document(
    document: &Document,
    registry_hash: u64,
) -> OutlineParseRequest {
    OutlineParseRequest::new(
        document.id,
        Arc::new(document.buffer.text()),
        document.syntax_token.clone(),
        document.revision(),
        registry_hash,
    )
}

pub async fn parse_outline_request(request: OutlineParseRequest) -> OutlineParseResult {
    parse_outline_snapshot(request)
}

pub fn parse_outline_snapshot(request: OutlineParseRequest) -> OutlineParseResult {
    let registry = OutlineRegistry::shared();

    let Some(plan) = registry.plan_for_syntax(&request.syntax_token) else {
        return OutlineParseResult::new(
            request.document_id,
            request.revision,
            request.syntax_token,
            request.registry_hash,
            OutlineTree::default(),
            Vec::new(),
            registry.diagnostics().to_vec(),
        );
    };

    let mut result = OutlineEngine::new(plan, registry.registry_hash()).parse(request.clone());
    if result.functions.is_empty() {
        let buffer = EditorBuffer::from_text(request.text.as_ref().clone());
        let fallback = outline_for_syntax_with_registry(&buffer, &request.syntax_token, registry);

        if !fallback.is_empty() {
            result.tree = OutlineTree::default();
            result.functions = fallback;
        }
    }

    result
}
