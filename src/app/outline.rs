//! Active-document outline scheduling and stale-result rejection.

use std::collections::HashMap;

use iced::Task;

use crate::core::{Document, DocumentId};
use crate::editor::{
    FunctionEntry, OutlineParseResult, OutlineSnapshotMetadata, OutlineState,
    outline_registry_hash, outline_request_for_document, parse_outline_request,
};

#[derive(Debug)]
pub(super) struct OutlineParsing {
    states: HashMap<DocumentId, OutlineState>,
    handles: HashMap<DocumentId, iced::task::Handle>,
    registry_hash: u64,
}

impl OutlineParsing {
    pub(super) fn new() -> Self {
        Self {
            states: HashMap::new(),
            handles: HashMap::new(),
            registry_hash: outline_registry_hash(),
        }
    }

    pub(super) fn state_for(&self, document: &Document) -> Option<&OutlineState> {
        let metadata = OutlineSnapshotMetadata::from_document(document, self.registry_hash);
        self.states
            .get(&document.id)
            .filter(|state| state.matches_metadata(&metadata))
    }

    pub(super) fn functions_for(&self, document: &Document) -> Option<&[FunctionEntry]> {
        let metadata = OutlineSnapshotMetadata::from_document(document, self.registry_hash);
        self.states
            .get(&document.id)
            .and_then(|state| state.current_functions(&metadata))
    }

    pub(super) fn schedule(&mut self, document: &Document) -> Task<OutlineParseResult> {
        let document_id = document.id;
        let inactive = self
            .handles
            .keys()
            .copied()
            .filter(|id| *id != document_id)
            .collect::<Vec<_>>();
        for id in inactive {
            self.remove(id);
        }

        let metadata = OutlineSnapshotMetadata::from_document(document, self.registry_hash);
        if !document.can_run_full_document_analysis() {
            self.states
                .entry(document_id)
                .and_modify(|state| {
                    if !state.matches_metadata(&metadata) {
                        *state = OutlineState::pending_metadata(metadata.clone());
                    }
                })
                .or_insert_with(|| OutlineState::pending_metadata(metadata));
            return Task::none();
        }
        if self.state_for(document).is_some() {
            return Task::none();
        }

        let request = outline_request_for_document(document, self.registry_hash);
        self.states
            .insert(document_id, OutlineState::pending(&request));
        let (task, handle) =
            Task::perform(parse_outline_request(request), std::convert::identity).abortable();
        if let Some(previous) = self.handles.insert(document_id, handle) {
            previous.abort();
        }
        task
    }

    pub(super) fn complete(&mut self, document: Option<&Document>, result: OutlineParseResult) {
        let metadata = OutlineSnapshotMetadata::from_result(&result);
        let Some(document) = document else {
            self.remove(metadata.document_id);
            return;
        };
        if !document.can_run_full_document_analysis()
            || !metadata.matches_document(document, self.registry_hash)
            || !self
                .states
                .get(&metadata.document_id)
                .is_some_and(|state| state.matches_metadata(&metadata))
        {
            return;
        }

        self.states
            .insert(metadata.document_id, OutlineState::ready(result));
        self.handles.remove(&metadata.document_id);
    }

    pub(super) fn remove(&mut self, document: DocumentId) {
        self.states.remove(&document);
        if let Some(handle) = self.handles.remove(&document) {
            handle.abort();
        }
    }

    pub(super) fn clear(&mut self) {
        for (_, handle) in self.handles.drain() {
            handle.abort();
        }
        self.states.clear();
    }
}
