pub mod changes;
use crate::core::document::{Document, DocumentId, DocumentLoadGeneration};
use crate::core::encoding::DecodedText;
use changes::{ChangeJournal, WorkspaceEvent};

use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Workspace {
    documents: Vec<Document>,
    changes: ChangeJournal,
    indices: HashMap<DocumentId, usize>,
    active_document_id: DocumentId,
    next_document_id: u64,
}

impl Workspace {
    pub fn new() -> Self {
        let first_id = DocumentId::new(1);

        Self {
            documents: vec![Document::untitled(first_id)],
            active_document_id: first_id,
            next_document_id: 2,
            changes: ChangeJournal::default(),
            indices: HashMap::from([(first_id, 0)]),
        }
    }

    pub fn active_document_id(&self) -> DocumentId {
        self.active_document_id
    }

    pub fn documents(&self) -> &[Document] {
        &self.documents
    }

    /// Apply a bulk edit while preserving tab order and recording touched documents.
    /// As with `document_mut`, edits must preserve each document's ID.
    pub fn edit_documents(&mut self, mut edit: impl FnMut(&mut Document)) {
        for document in &mut self.documents {
            self.changes.touch(document);
            edit(document);
        }
    }

    pub fn push_document(&mut self, document: Document) {
        assert!(
            !self.indices.contains_key(&document.id),
            "document IDs must be unique"
        );
        self.next_document_id = self.next_document_id.max(document.id.get() + 1);
        self.indices.insert(document.id, self.documents.len());
        self.changes
            .push(WorkspaceEvent::DocumentOpened(document.id));
        self.documents.push(document);
    }

    pub fn clear_documents(&mut self) {
        self.indices.clear();
        for document in self.documents.drain(..) {
            self.changes
                .push(WorkspaceEvent::DocumentClosed(document.id));
        }
    }

    /// Drain structural facts and coalesced document invalidations at an update boundary.
    pub fn publish_changes(&mut self, emit: impl FnMut(WorkspaceEvent)) {
        self.changes.publish(&self.documents, &self.indices, emit);
    }

    pub fn next_document_id(&self) -> DocumentId {
        DocumentId::new(self.next_document_id)
    }

    pub fn generate_document_id(&mut self) -> DocumentId {
        let id = DocumentId::new(self.next_document_id);
        self.next_document_id += 1;
        id
    }

    pub fn create_untitled(&mut self) -> DocumentId {
        let id = self.generate_document_id();
        self.push_document(Document::untitled(id));
        self.select(id);
        id
    }

    pub fn insert_loaded_file(&mut self, path: impl Into<PathBuf>, text: &str) -> DocumentId {
        let id = self.generate_document_id();
        self.push_document(Document::from_path(id, path, text));
        self.select(id);
        id
    }

    pub fn insert_decoded_file(
        &mut self,
        path: impl Into<PathBuf>,
        decoded: DecodedText,
    ) -> DocumentId {
        let id = self.generate_document_id();
        self.push_document(Document::from_decoded(id, path, decoded));
        self.select(id);
        id
    }

    pub fn insert_loading_file(
        &mut self,
        path: impl Into<PathBuf>,
    ) -> (DocumentId, DocumentLoadGeneration) {
        let id = self.generate_document_id();
        let generation = DocumentLoadGeneration::next();
        self.push_document(Document::loading(id, path, generation));
        self.select(id);
        (id, generation)
    }

    pub fn active_document(&self) -> Option<&Document> {
        self.document(self.active_document_id)
    }

    pub fn active_document_mut(&mut self) -> Option<&mut Document> {
        self.document_mut(self.active_document_id)
    }

    pub fn document(&self, id: DocumentId) -> Option<&Document> {
        self.indices
            .get(&id)
            .and_then(|index| self.documents.get(*index))
    }

    pub fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        let index = self.index_of(id)?;
        let document = &mut self.documents[index];
        self.changes.touch(document);
        Some(document)
    }

    pub fn select(&mut self, id: DocumentId) -> bool {
        if self.document(id).is_some() {
            if self.active_document_id != id {
                self.active_document_id = id;
                self.changes.push(WorkspaceEvent::ActiveDocumentChanged(id));
            }
            true
        } else {
            false
        }
    }

    pub fn close(&mut self, id: DocumentId) -> Option<Document> {
        let index = self.index_of(id)?;
        let removed = self.documents.remove(index);
        self.reindex();
        self.changes.push(WorkspaceEvent::DocumentClosed(id));

        if self.documents.is_empty() {
            let replacement_id = self.generate_document_id();
            self.push_document(Document::untitled(replacement_id));
            self.select(replacement_id);
            return Some(removed);
        }

        if self.active_document_id == id {
            let next_index = index.saturating_sub(1).min(self.documents.len() - 1);
            self.select(self.documents[next_index].id);
        }

        Some(removed)
    }

    pub fn document_ids(&self) -> Vec<DocumentId> {
        self.document_ids_matching(|_| true)
    }

    pub fn document_ids_except(&self, excluded_id: DocumentId) -> Vec<DocumentId> {
        self.document_ids_matching(|document| document.id != excluded_id)
    }

    pub fn document_ids_unpinned(&self) -> Vec<DocumentId> {
        self.document_ids_matching(|document| !document.is_pinned)
    }

    pub fn document_ids_clean(&self) -> Vec<DocumentId> {
        self.document_ids_matching(|document| !document.is_dirty)
    }

    pub fn document_ids_to_left_of(&self, id: DocumentId) -> Vec<DocumentId> {
        let Some(index) = self.index_of(id) else {
            return Vec::new();
        };

        self.documents[..index]
            .iter()
            .map(|document| document.id)
            .collect()
    }

    pub fn document_ids_to_right_of(&self, id: DocumentId) -> Vec<DocumentId> {
        let Some(index) = self.index_of(id) else {
            return Vec::new();
        };

        self.documents[index.saturating_add(1)..]
            .iter()
            .map(|document| document.id)
            .collect()
    }

    pub fn toggle_pin(&mut self, id: DocumentId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };

        let mut document = self.documents.remove(index);
        document.is_pinned = !document.is_pinned;

        let insert_index = if document.is_pinned {
            self.pinned_count()
        } else {
            self.documents.len()
        };

        self.documents.insert(insert_index, document);
        self.reindex();
        self.changes.push(WorkspaceEvent::OrderChanged);
        true
    }

    pub fn reorder(&mut self, moved_id: DocumentId, target_id: DocumentId) -> bool {
        if moved_id == target_id {
            return false;
        }

        let Some(from_index) = self.index_of(moved_id) else {
            return false;
        };
        let Some(to_index) = self.index_of(target_id) else {
            return false;
        };

        if self.documents[from_index].is_pinned != self.documents[to_index].is_pinned {
            return false;
        }

        let document = self.documents.remove(from_index);
        self.documents.insert(to_index, document);
        self.reindex();
        self.changes.push(WorkspaceEvent::OrderChanged);
        true
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn pinned_count(&self) -> usize {
        self.documents
            .iter()
            .take_while(|document| document.is_pinned)
            .count()
    }

    fn reindex(&mut self) {
        self.indices.clear();
        self.indices.extend(
            self.documents
                .iter()
                .enumerate()
                .map(|(index, document)| (document.id, index)),
        );
    }

    fn index_of(&self, id: DocumentId) -> Option<usize> {
        self.indices.get(&id).copied()
    }

    fn document_ids_matching(&self, predicate: impl Fn(&Document) -> bool) -> Vec<DocumentId> {
        self.documents
            .iter()
            .filter(|document| predicate(document))
            .map(|document| document.id)
            .collect()
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}
