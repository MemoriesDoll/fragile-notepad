use std::sync::Arc;

use crate::core::DocumentId;

use super::super::buffer::EditorBuffer;
use super::cascade::cascade;
use super::lexical::OutlineCodeMask;
use super::projection::project_functions;
use super::structure::discover_structure;
use super::{OutlineParseRequest, OutlineParseResult, OutlinePlan, OutlineTree};

#[derive(Debug, Clone)]
pub struct OutlineEngine<'a> {
    plan: &'a OutlinePlan,
    registry_hash: u64,
}

impl<'a> OutlineEngine<'a> {
    pub fn new(plan: &'a OutlinePlan, registry_hash: u64) -> Self {
        Self {
            plan,
            registry_hash,
        }
    }

    pub fn parse_buffer(
        &self,
        buffer: &EditorBuffer,
        syntax_token: impl Into<String>,
    ) -> OutlineParseResult {
        self.parse(OutlineParseRequest::new(
            DocumentId::new(0),
            Arc::new(buffer.text().to_owned()),
            syntax_token,
            0,
            self.registry_hash,
        ))
    }

    pub fn parse(&self, request: OutlineParseRequest) -> OutlineParseResult {
        let mask = OutlineCodeMask::new(&request.text, &self.plan.lexical);
        let structure = discover_structure(&request.text, &mask, self.plan);
        let cascade = cascade(&request.text, &structure.containers, structure.declarations);
        let functions = project_functions(cascade.functions);

        OutlineParseResult::new(
            request.document_id,
            request.revision,
            request.syntax_token,
            request.registry_hash,
            if functions.is_empty() {
                OutlineTree::default()
            } else {
                cascade.tree
            },
            functions,
            self.plan.diagnostics.clone(),
        )
    }
}
