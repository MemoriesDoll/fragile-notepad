//! Saving snapshots and handling completion or failure.

use crate::app::{App, CloseGoal};
use crate::core::{DocumentId, DocumentLoadState};
use crate::message::{Message, SaveRequest};
use crate::services;
use crate::services::types::{FileError, FileSaveResult};
use iced::{Task, window};
use std::sync::Arc;

impl App {
    pub(super) fn save_active(&mut self, force_save_as: bool) -> Task<Message> {
        self.pending_save_all.clear();
        self.save_one(self.workspace.active_document_id, force_save_as)
    }

    pub(super) fn save_copy_active(&mut self) -> Task<Message> {
        let id = self.workspace.active_document_id;
        if self
            .workspace
            .document(id)
            .is_some_and(|doc| matches!(doc.load_state, DocumentLoadState::Deferred { .. }))
        {
            let load = self.activate_document(id);
            if self
                .workspace
                .document(id)
                .is_some_and(|doc| !doc.has_complete_text_index())
            {
                return load;
            }
        }
        if self.pending_save.is_some() {
            return Task::none();
        }

        let Some(document) = self.workspace.active_document() else {
            return Task::none();
        };
        if document.is_loading_or_indexing() {
            self.file_status = Some(String::from("Finish loading before saving."));
            return Task::none();
        }
        if matches!(document.load_state, DocumentLoadState::Failed { .. }) {
            self.file_status = Some(String::from("Reload the file successfully before saving."));
            return Task::none();
        }
        let snapshot = match document.bytes_for_save() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.file_status = Some(format!(
                    "Save copy failed: {}",
                    FileError::Encoding(error).summary()
                ));
                return Task::none();
            }
        };

        let request = SaveRequest {
            document_id: document.id,
            revision: document.revision(),
            snapshot: Arc::new(snapshot),
        };
        self.pending_save = Some(request.clone());
        let contents = request.snapshot.as_ref().clone();

        window::oldest()
            .and_then(move |id| {
                let contents = contents.clone();

                window::run(id, move |window| {
                    services::file_system::save_file_copy_as(window, contents)
                })
            })
            .then(Task::future)
            .map(move |result| Message::FileCopySaved(request.clone(), result))
    }

    pub(super) fn save_all_documents(&mut self) -> Task<Message> {
        if self.pending_save.is_some() {
            return Task::none();
        }

        self.pending_save_all = self
            .workspace
            .documents()
            .iter()
            .filter(|document| document.is_dirty)
            .map(|document| document.id)
            .collect();

        self.continue_save_all()
    }

    pub(super) fn continue_save_all(&mut self) -> Task<Message> {
        while let Some(document_id) = self.pending_save_all.front().copied() {
            if self
                .workspace
                .document(document_id)
                .is_some_and(|document| document.is_dirty)
            {
                return self.save_one(document_id, false);
            }

            self.pending_save_all.pop_front();
        }

        Task::none()
    }

    pub(super) fn save_one(
        &mut self,
        document_id: DocumentId,
        force_save_as: bool,
    ) -> Task<Message> {
        if self
            .workspace
            .document(document_id)
            .is_some_and(|doc| matches!(doc.load_state, DocumentLoadState::Deferred { .. }))
        {
            let load = self.activate_document(document_id);
            if self
                .workspace
                .document(document_id)
                .is_some_and(|doc| !doc.has_complete_text_index())
            {
                return load;
            }
        }
        if self.pending_save.is_some() {
            return Task::none();
        }

        let Some(document) = self.workspace.document_mut(document_id) else {
            return Task::none();
        };
        if document.is_loading_or_indexing() {
            self.file_status = Some(String::from("Finish loading before saving."));
            return Task::none();
        }
        if matches!(document.load_state, DocumentLoadState::Failed { .. }) {
            self.file_status = Some(String::from("Reload the file successfully before saving."));
            return Task::none();
        }
        let snapshot = match document.bytes_for_save() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.save_failed(document_id, FileError::Encoding(error));
                return Task::none();
            }
        };

        let request = SaveRequest {
            document_id: document.id,
            revision: document.revision(),
            snapshot: Arc::new(snapshot),
        };
        document.history.break_group();
        self.pending_save = Some(request.clone());

        if !force_save_as {
            if let Some(path) = document.path.clone() {
                let contents = request.snapshot.as_ref().clone();

                return Task::perform(services::save_file(path, contents), move |result| {
                    Message::FileSaved(request, result)
                });
            }
        }

        let contents = request.snapshot.as_ref().clone();

        window::oldest()
            .and_then(move |id| {
                let contents = contents.clone();

                window::run(id, move |window| services::save_file_as(window, contents))
            })
            .then(Task::future)
            .map(move |result| Message::FileSaved(request.clone(), result))
    }

    pub(super) fn save_done(
        &mut self,
        request: SaveRequest,
        result: FileSaveResult,
    ) -> Task<Message> {
        self.pending_save = None;
        let save_succeeded = result.is_ok();
        let mut tasks = Vec::new();

        match result {
            Ok(path) => {
                self.file_status = None;
                let saved_path = path.clone();
                let mut syntax_changed = false;
                if let Some(document) = self.workspace.document_mut(request.document_id) {
                    let before_revision = document.revision();
                    document.set_path(path);
                    syntax_changed = document.revision() != before_revision;
                    let saved_snapshot_is_current = document
                        .bytes_for_save()
                        .is_ok_and(|bytes| bytes == request.snapshot.as_ref().as_slice());

                    if saved_snapshot_is_current {
                        document.mark_clean();
                    } else {
                        document.invalidate_clean_checkpoint();
                    }
                }
                if syntax_changed {
                    tasks.push(self.schedule_outline_parse(request.document_id));
                }
                tasks.push(self.record_open_history(saved_path));
            }
            Err(error) => {
                self.file_status = Some(format!("Save failed: {}", error.summary()));
            }
        }

        if self.pending_save_all.front() == Some(&request.document_id) {
            if save_succeeded {
                self.pending_save_all.pop_front();
                tasks.push(self.continue_save_all());
                return Task::batch(tasks);
            }

            self.pending_save_all.clear();
        }

        if self.pending_close_after_save == Some(request.document_id) {
            self.pending_close_after_save = None;

            if save_succeeded
                && self
                    .workspace
                    .document(request.document_id)
                    .is_some_and(|document| !document.is_dirty)
            {
                tasks.push(self.close_now(request.document_id));

                if !self.pending_close_documents.is_empty() {
                    tasks.push(self.continue_close());
                    return Task::batch(tasks);
                }

                if self.should_exit() {
                    self.close_goal = CloseGoal::KeepOpen;
                    tasks.push(self.exit_after_settings());
                }
            } else {
                self.clear_close();
                self.close_goal = CloseGoal::KeepOpen;
            }
        }

        Task::batch(tasks)
    }

    pub(super) fn save_copy_done(
        &mut self,
        _request: SaveRequest,
        result: FileSaveResult,
    ) -> Task<Message> {
        self.pending_save = None;

        match result {
            Ok(path) => {
                self.file_status = Some(format!("Saved copy: {}", path.display()));
            }
            Err(error) => {
                self.file_status = Some(format!("Save copy failed: {}", error.summary()));
            }
        }

        Task::none()
    }

    pub(super) fn save_failed(&mut self, document_id: DocumentId, error: FileError) {
        self.pending_save = None;
        self.file_status = Some(format!("Save failed: {}", error.summary()));

        if self.pending_save_all.front() == Some(&document_id) {
            self.pending_save_all.clear();
        }

        if self.pending_close_after_save == Some(document_id) {
            self.pending_close_after_save = None;
            self.clear_close();
            self.close_goal = CloseGoal::KeepOpen;
        }
    }
}
