//! Active-document outline scheduling and stale-result rejection.

use std::collections::HashMap;

use super::App;
use crate::editor::EditorSelection;
use crate::message::Message;

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
}

impl App {
    pub(super) fn active_outline_state(&self) -> Option<&OutlineState> {
        self.workspace
            .active_document()
            .and_then(|document| self.outline_parsing.state_for(document))
    }

    pub(super) fn complete_outline_parse(&mut self, result: OutlineParseResult) -> Task<Message> {
        self.outline_parsing
            .complete(self.workspace.document(result.document_id), result);
        Task::none()
    }

    pub(super) fn toggle_function_list(&mut self) -> Task<Message> {
        self.menu.close();

        self.is_function_list_visible = !self.is_function_list_visible;
        self.chrome_animation
            .function_list
            .set_visible(self.is_function_list_visible);

        Task::none()
    }

    pub(super) fn select_function_list_entry(
        &mut self,
        position: crate::editor::EditorPosition,
    ) -> Task<Message> {
        self.menu.close();

        let Some(document) = self.workspace.active_document_mut() else {
            return Task::none();
        };

        let position = document.buffer.clamp_position(position);
        document.set_main_selection(EditorSelection::new(position, position));
        document.preferred_vertical_column = None;
        document.reveal_position(position);

        Task::none()
    }
}

impl OutlineParsing {
    pub(super) fn observe(
        &mut self,
        event: super::events::Event,
        active: DocumentId,
        work: &mut super::events::PendingWork,
    ) {
        use super::events::{Event, Work, WorkspaceEvent as W};
        if let Event::Workspace(W::DocumentClosed(id)) = event {
            self.remove(id);
        }
        let needed = match event {
            Event::Started | Event::SettingsChanged => true,
            Event::Workspace(W::ActiveDocumentChanged(_) | W::DocumentOpened(_)) => true,
            Event::Workspace(W::ContentChanged(id) | W::LoadStateChanged(id)) => id == active,
            _ => false,
        };
        if needed {
            work.request(Work::Outline);
        }
    }
}
