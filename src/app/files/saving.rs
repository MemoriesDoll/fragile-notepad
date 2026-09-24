//! Saving snapshots and handling completion or failure.

use crate::app::{App, CloseGoal};
use crate::core::{DocumentId, DocumentLoadState};
use crate::message::{Message, SaveRequest};
use crate::services;
use crate::services::types::{FileError, FileSaveResult};
use iced::{Task, window};
use std::sync::Arc;

impl App {
    pub(in crate::app) fn auto_save_before_switch(
        &mut self,
        target: Option<DocumentId>,
    ) -> Task<Message> {
        let active = self.workspace.active_document_id();
        if target == Some(active) {
            return Task::none();
        }

        self.queue_auto_save(active)
    }

    pub(in crate::app) fn queue_auto_save(&mut self, document_id: DocumentId) -> Task<Message> {
        if !self.settings.auto_save
            || self.close_prompt.is_closing()
            || self.files.pending_auto_saves.contains(&document_id)
        {
            return Task::none();
        }

        let eligible = self
            .workspace
            .document(document_id)
            .is_some_and(|document| {
                document.is_dirty && document.path.is_some() && document.has_complete_text_index()
            });
        if !eligible {
            return Task::none();
        }

        self.files.pending_auto_saves.push_back(document_id);
        self.continue_auto_save()
    }

    fn continue_auto_save(&mut self) -> Task<Message> {
        if !self.settings.auto_save || self.should_exit() {
            self.files.pending_auto_saves.clear();
            return Task::none();
        }
        if self.files.pending_save.is_some() || !self.files.pending_save_all.is_empty() {
            return Task::none();
        }

        while let Some(document_id) = self.files.pending_auto_saves.pop_front() {
            let eligible = self
                .workspace
                .document(document_id)
                .is_some_and(|document| {
                    document.is_dirty
                        && document.path.is_some()
                        && document.has_complete_text_index()
                });
            if !eligible {
                continue;
            }

            let task = self.save_one(document_id, false);
            if self.files.pending_save.is_some() {
                return task;
            }
        }

        Task::none()
    }

    pub(super) fn save_active(&mut self, force_save_as: bool) -> Task<Message> {
        self.files.pending_save_all.clear();
        self.save_one(self.workspace.active_document_id(), force_save_as)
    }

    pub(super) fn save_copy_active(&mut self) -> Task<Message> {
        let id = self.workspace.active_document_id();
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
        if self.files.pending_save.is_some() {
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
        self.files.pending_save = Some(request.clone());
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
        if self.files.pending_save.is_some() {
            return Task::none();
        }

        self.files.pending_save_all = self
            .workspace
            .documents()
            .iter()
            .filter(|document| document.is_dirty)
            .map(|document| document.id)
            .collect();

        self.continue_save_all()
    }

    pub(super) fn continue_save_all(&mut self) -> Task<Message> {
        while let Some(document_id) = self.files.pending_save_all.front().copied() {
            if self
                .workspace
                .document(document_id)
                .is_some_and(|document| document.is_dirty)
            {
                return self.save_one(document_id, false);
            }

            self.files.pending_save_all.pop_front();
        }

        self.continue_auto_save()
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
        if self.files.pending_save.is_some() {
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
        self.files.pending_save = Some(request.clone());

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
        self.files.pending_save = None;
        let save_succeeded = result.is_ok();
        let mut tasks = Vec::new();

        match result {
            Ok(path) => {
                self.file_status = None;
                let saved_path = path.clone();
                if let Some(document) = self.workspace.document_mut(request.document_id) {
                    document.set_path(path);
                    let saved_snapshot_is_current = document
                        .bytes_for_save()
                        .is_ok_and(|bytes| bytes == request.snapshot.as_ref().as_slice());

                    if saved_snapshot_is_current {
                        document.mark_clean();
                    } else {
                        document.invalidate_clean_checkpoint();
                    }
                }
                tasks.push(self.record_open_history(saved_path));
            }
            Err(error) => {
                self.file_status = Some(format!("Save failed: {}", error.summary()));
            }
        }

        if self.files.pending_save_all.front() == Some(&request.document_id) {
            if save_succeeded {
                self.files.pending_save_all.pop_front();
                tasks.push(self.continue_save_all());
                return Task::batch(tasks);
            }

            self.files.pending_save_all.clear();
        }

        if self.files.pending_close_after_save == Some(request.document_id) {
            self.files.pending_close_after_save = None;

            if save_succeeded
                && self
                    .workspace
                    .document(request.document_id)
                    .is_some_and(|document| !document.is_dirty)
            {
                tasks.push(self.close_now(request.document_id));

                if !self.files.pending_close_documents.is_empty() {
                    tasks.push(self.continue_close());
                    return Task::batch(tasks);
                }

                if self.should_exit() {
                    self.files.close_goal = CloseGoal::KeepOpen;
                    tasks.push(self.exit_after_settings());
                }
            } else {
                self.clear_close();
                self.files.close_goal = CloseGoal::KeepOpen;
            }
        }

        tasks.push(self.continue_auto_save());
        Task::batch(tasks)
    }

    pub(super) fn save_copy_done(
        &mut self,
        _request: SaveRequest,
        result: FileSaveResult,
    ) -> Task<Message> {
        self.files.pending_save = None;

        match result {
            Ok(path) => {
                self.file_status = Some(format!("Saved copy: {}", path.display()));
            }
            Err(error) => {
                self.file_status = Some(format!("Save copy failed: {}", error.summary()));
            }
        }

        self.continue_auto_save()
    }

    pub(super) fn save_failed(&mut self, document_id: DocumentId, error: FileError) {
        self.files.pending_save = None;
        self.file_status = Some(format!("Save failed: {}", error.summary()));

        if self.files.pending_save_all.front() == Some(&document_id) {
            self.files.pending_save_all.clear();
        }

        if self.files.pending_close_after_save == Some(document_id) {
            self.files.pending_close_after_save = None;
            self.clear_close();
            self.files.close_goal = CloseGoal::KeepOpen;
        }
    }
}
