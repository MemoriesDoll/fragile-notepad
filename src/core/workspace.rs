use crate::core::document::{Document, DocumentId};
use crate::core::encoding::DecodedText;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub documents: Vec<Document>,
    pub active_document_id: DocumentId,
    next_document_id: u64,
}

impl Workspace {
    pub fn new() -> Self {
        let first_id = DocumentId::new(1);

        Self {
            documents: vec![Document::untitled(first_id)],
            active_document_id: first_id,
            next_document_id: 2,
        }
    }

    pub fn documents(&self) -> &[Document] {
        &self.documents
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
        self.documents.push(Document::untitled(id));
        self.active_document_id = id;
        id
    }

    pub fn insert_loaded_file(&mut self, path: impl Into<PathBuf>, text: &str) -> DocumentId {
        let id = self.generate_document_id();
        self.documents.push(Document::from_path(id, path, text));
        self.active_document_id = id;
        id
    }

    pub fn insert_decoded_file(
        &mut self,
        path: impl Into<PathBuf>,
        decoded: DecodedText,
    ) -> DocumentId {
        let id = self.generate_document_id();
        self.documents
            .push(Document::from_decoded(id, path, decoded));
        self.active_document_id = id;
        id
    }

    pub fn active_document(&self) -> Option<&Document> {
        self.document(self.active_document_id)
    }

    pub fn active_document_mut(&mut self) -> Option<&mut Document> {
        self.document_mut(self.active_document_id)
    }

    pub fn document(&self, id: DocumentId) -> Option<&Document> {
        self.documents.iter().find(|document| document.id == id)
    }

    pub fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.documents.iter_mut().find(|document| document.id == id)
    }

    pub fn select(&mut self, id: DocumentId) -> bool {
        if self.document(id).is_some() {
            self.active_document_id = id;
            true
        } else {
            false
        }
    }

    pub fn close(&mut self, id: DocumentId) -> Option<Document> {
        let index = self.index_of(id)?;
        let removed = self.documents.remove(index);

        if self.documents.is_empty() {
            let replacement_id = self.generate_document_id();
            self.documents.push(Document::untitled(replacement_id));
            self.active_document_id = replacement_id;
            return Some(removed);
        }

        if self.active_document_id == id {
            let next_index = index.saturating_sub(1).min(self.documents.len() - 1);
            self.active_document_id = self.documents[next_index].id;
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
        let insert_index = if from_index < to_index {
            to_index
        } else {
            to_index
        };

        self.documents.insert(insert_index, document);
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

    fn index_of(&self, id: DocumentId) -> Option<usize> {
        self.documents.iter().position(|document| document.id == id)
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
