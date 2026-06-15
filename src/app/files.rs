use iced::{Task, window};

use crate::core::DocumentId;
use crate::message::{
    DirtyCloseDecision, FileError, FileLoadChunk, FileLoadFailure, FileLoadFinished,
    FileLoadProgress, FileLoadRequest, FileOpenResult, FileResult, FileSaveResult, Message,
    SaveRequest,
};
use crate::services;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use super::{App, CloseGoal};

impl App {
    pub(super) fn update_file(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(document_id) => {
                self.active_menu = None;
                if self.workspace.select(document_id) {
                    self.refresh_find_matches();
                    self.prewarm_active_syntax_cache();
                    return self.schedule_outline_parse(document_id);
                }

                Task::none()
            }
            Message::TabClosed(document_id) => {
                self.active_menu = None;
                self.dragged_tab = None;
                self.hovered_drop_tab = None;
                self.close_request(document_id)
            }
            Message::TabPinToggled(document_id) => {
                self.active_menu = None;
                self.dragged_tab = None;
                self.hovered_drop_tab = None;
                self.workspace.toggle_pin(document_id);
                Task::none()
            }
            Message::TabDragStarted(document_id) => {
                self.active_menu = None;
                self.dragged_tab = Some(document_id);
                self.hovered_drop_tab = Some(document_id);
                if self.workspace.select(document_id) {
                    self.refresh_find_matches();
                }
                Task::none()
            }
            Message::TabDragHovered(document_id) => {
                if self.dragged_tab.is_some() {
                    self.hovered_drop_tab = Some(document_id);
                }

                Task::none()
            }
            Message::TabDragLeft(document_id) => {
                if self.hovered_drop_tab == Some(document_id) {
                    self.hovered_drop_tab = None;
                }

                Task::none()
            }
            Message::TabDragReleased(document_id) => {
                self.active_menu = None;

                if let Some(moved_id) = self.dragged_tab.take() {
                    self.workspace.reorder(moved_id, document_id);
                }

                self.hovered_drop_tab = None;
                Task::none()
            }
            Message::NewFile => {
                self.active_menu = None;
                let document_id = self.workspace.create_untitled();
                if let Some(document) = self.workspace.document_mut(document_id) {
                    document.set_decoration_settings(self.settings.decoration_settings());
                }
                self.refresh_find_matches();
                self.schedule_outline_parse(document_id)
            }
            Message::OpenFile => {
                self.active_menu = None;
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;
                    self.file_status = None;

                    window::oldest()
                        .and_then(|id| window::run(id, services::pick_file))
                        .then(Task::future)
                        .map(Message::FilePicked)
                }
            }
            Message::FileDropped(window_id, path) => self.open_dropped_file(window_id, path),
            Message::FilePicked(result) => self.file_picked(result),
            Message::FileOpened(result) => self.open_done(result),
            Message::FileLoadProgress(progress) => self.load_progress(progress),
            Message::FileLoadChunk(chunk) => self.load_chunk(chunk),
            Message::FileLoadFinished(result) => self.load_finished(result),
            Message::SaveFile => {
                self.active_menu = None;
                self.file_status = None;
                self.save_active(false)
            }
            Message::SaveAllFiles => {
                self.active_menu = None;
                self.file_status = None;
                self.save_all_documents()
            }
            Message::SaveFileAs => {
                self.active_menu = None;
                self.file_status = None;
                self.save_active(true)
            }
            Message::FileSaved(request, result) => self.save_done(request, result),
            Message::EncodingSelected(encoding) => {
                self.active_menu = None;
                if let Some(document) = self.workspace.active_document_mut() {
                    document.set_encoding(encoding);
                }
                Task::none()
            }
            Message::CloseFile => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_request(self.workspace.active_document_id)
            }
            Message::CloseAllFiles => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_documents(self.workspace.document_ids())
            }
            Message::CloseAllButActiveFile => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_documents(
                    self.workspace
                        .document_ids_except(self.workspace.active_document_id),
                )
            }
            Message::CloseAllButPinnedFiles => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_documents(self.workspace.document_ids_unpinned())
            }
            Message::CloseAllToLeft => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_documents(
                    self.workspace
                        .document_ids_to_left_of(self.workspace.active_document_id),
                )
            }
            Message::CloseAllToRight => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_documents(
                    self.workspace
                        .document_ids_to_right_of(self.workspace.active_document_id),
                )
            }
            Message::CloseAllUnchanged => {
                self.active_menu = None;
                self.close_goal = CloseGoal::KeepOpen;
                self.close_documents(self.workspace.document_ids_clean())
            }
            Message::DirtyCloseResolved(document_id, decision) => {
                self.resolve_close(document_id, decision)
            }
            _ => unreachable!("file handler received non-file message"),
        }
    }

    fn open_done(&mut self, result: FileOpenResult) -> Task<Message> {
        let task = match result {
            Ok(opened) => {
                self.file_status = None;
                let document_id =
                    if let Some(document_id) = self.loading_document_id_for_path(&opened.path) {
                        if let Some(document) = self.workspace.document_mut(document_id)
                            && let Some(generation) = document.load_generation()
                        {
                            document.complete_loading(generation, opened.contents.as_ref().clone());
                        }
                        self.workspace.select(document_id);
                        document_id
                    } else {
                        self.workspace
                            .insert_decoded_file(opened.path, opened.contents.as_ref().clone())
                    };
                if let Some(document) = self.workspace.document_mut(document_id) {
                    document.set_decoration_settings(self.settings.decoration_settings());
                }
                self.refresh_find_matches();
                self.prewarm_active_syntax_cache();
                self.schedule_outline_parse(document_id)
            }
            Err(error) => {
                self.file_status = Some(format!("Open failed: {}", error.summary()));
                Task::none()
            }
        };

        self.refresh_file_loading_state();
        task
    }

    fn file_picked(&mut self, result: FileResult<PathBuf>) -> Task<Message> {
        match result {
            Ok(path) => self.start_loading_file(path),
            Err(error) => {
                self.file_status = Some(format!("Open failed: {}", error.summary()));
                self.refresh_file_loading_state();
                Task::none()
            }
        }
    }

    fn open_dropped_file(&mut self, window_id: window::Id, path: PathBuf) -> Task<Message> {
        if self.main_window_id != Some(window_id) {
            return Task::none();
        }

        self.active_menu = None;

        self.start_loading_file(path)
    }

    fn start_loading_file(&mut self, path: PathBuf) -> Task<Message> {
        self.is_loading = true;
        self.file_status = None;

        let (document_id, generation) = self.workspace.insert_loading_file(path.clone());
        if let Some(document) = self.workspace.document_mut(document_id) {
            document.set_decoration_settings(self.settings.decoration_settings());
        }
        self.refresh_find_matches();
        self.prewarm_active_syntax_cache();

        services::load_file_request(FileLoadRequest {
            document_id,
            generation,
            path,
            chunk_size: services::DEFAULT_CHUNK_SIZE,
        })
    }

    fn load_progress(&mut self, progress: FileLoadProgress) -> Task<Message> {
        let Some(document) = self.workspace.document_mut(progress.document_id) else {
            return Task::none();
        };

        document.update_load_progress(
            progress.generation,
            progress.bytes_read,
            progress.total_bytes,
        );
        Task::none()
    }

    fn load_chunk(&mut self, chunk: FileLoadChunk) -> Task<Message> {
        let Some(document) = self.workspace.document_mut(chunk.document_id) else {
            return Task::none();
        };

        if document.replace_loading_preview(
            chunk.generation,
            chunk.text.as_ref(),
            chunk.bytes_read,
            chunk.total_bytes,
        ) {
            if self.workspace.active_document_id == chunk.document_id {
                self.refresh_find_matches();
                self.prewarm_active_syntax_cache();
            }
        }

        Task::none()
    }

    fn load_finished(
        &mut self,
        result: Result<FileLoadFinished, FileLoadFailure>,
    ) -> Task<Message> {
        let task = match result {
            Ok(finished) => {
                let Some(document) = self.workspace.document_mut(finished.document_id) else {
                    self.refresh_file_loading_state();
                    return Task::none();
                };

                let completed = if let Some(contents) = finished.fallback_contents {
                    document.complete_loading(finished.generation, contents.as_ref().clone())
                } else {
                    document.complete_streaming_load(finished.generation, finished.encoding)
                };

                if !completed {
                    self.refresh_file_loading_state();
                    return Task::none();
                }

                self.file_status = None;
                self.refresh_find_matches();
                self.prewarm_active_syntax_cache();
                self.schedule_outline_parse(finished.document_id)
            }
            Err(failure) => self.load_failed(failure),
        };

        self.refresh_file_loading_state();
        task
    }

    fn load_failed(&mut self, failure: FileLoadFailure) -> Task<Message> {
        let Some(document) = self.workspace.document_mut(failure.document_id) else {
            return Task::none();
        };

        if document.fail_loading(failure.generation) {
            self.file_status = Some(format!("Open failed: {}", failure.error.summary()));
        }

        Task::none()
    }

    fn save_active(&mut self, force_save_as: bool) -> Task<Message> {
        self.pending_save_all.clear();
        self.save_one(self.workspace.active_document_id, force_save_as)
    }

    fn save_all_documents(&mut self) -> Task<Message> {
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

    fn continue_save_all(&mut self) -> Task<Message> {
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

    fn save_one(&mut self, document_id: DocumentId, force_save_as: bool) -> Task<Message> {
        if self.pending_save.is_some() {
            return Task::none();
        }

        let Some(document) = self.workspace.document(document_id) else {
            return Task::none();
        };
        if document.is_loading_or_indexing() {
            self.file_status = Some(String::from("Finish loading before saving."));
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

    fn save_done(&mut self, request: SaveRequest, result: FileSaveResult) -> Task<Message> {
        self.pending_save = None;
        let save_succeeded = result.is_ok();
        let mut tasks = Vec::new();

        match result {
            Ok(path) => {
                self.file_status = None;
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
                    }
                }
                if syntax_changed {
                    tasks.push(self.schedule_outline_parse(request.document_id));
                }
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
                    tasks.push(iced::exit());
                }
            } else {
                self.clear_close();
                self.close_goal = CloseGoal::KeepOpen;
            }
        }

        Task::batch(tasks)
    }

    fn close_request(&mut self, document_id: DocumentId) -> Task<Message> {
        let Some(document) = self.workspace.document(document_id) else {
            return Task::none();
        };

        if !document.is_dirty {
            return self.close_now(document_id);
        }

        self.pending_dirty_close = Some(document_id);
        Task::none()
    }

    fn resolve_close(
        &mut self,
        document_id: DocumentId,
        decision: DirtyCloseDecision,
    ) -> Task<Message> {
        if self.pending_dirty_close == Some(document_id) {
            self.pending_dirty_close = None;
        }

        match decision {
            DirtyCloseDecision::Save => {
                if self.pending_save.is_some() {
                    self.pending_close_after_save = None;
                    self.clear_close();
                    self.pending_save_all.clear();
                    return Task::none();
                }

                self.pending_save_all.clear();
                self.pending_close_after_save = Some(document_id);
                self.save_one(document_id, false)
            }
            DirtyCloseDecision::Discard => {
                let close_task = self.close_now(document_id);

                if !self.pending_close_documents.is_empty() {
                    Task::batch([close_task, self.continue_close()])
                } else if self.should_exit() {
                    self.close_goal = CloseGoal::KeepOpen;
                    Task::batch([close_task, iced::exit()])
                } else {
                    close_task
                }
            }
            DirtyCloseDecision::Cancel => {
                self.pending_close_after_save = None;
                self.clear_close();
                self.pending_save_all.clear();
                self.close_goal = CloseGoal::KeepOpen;
                Task::none()
            }
        }
    }

    fn close_now(&mut self, document_id: DocumentId) -> Task<Message> {
        if self.pending_dirty_close == Some(document_id) {
            self.pending_dirty_close = None;
        }

        self.workspace.close(document_id);
        self.refresh_file_loading_state();
        self.outline_states.remove(&document_id);

        let active_document_id = self.workspace.active_document_id;
        self.refresh_find_matches();
        self.schedule_outline_parse(active_document_id)
    }

    fn close_documents(&mut self, document_ids: Vec<DocumentId>) -> Task<Message> {
        self.pending_close_documents = VecDeque::from(document_ids);
        self.continue_close()
    }

    fn continue_close(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();

        while let Some(document_id) = self.pending_close_documents.pop_front() {
            let Some(document) = self.workspace.document(document_id) else {
                continue;
            };

            if document.is_dirty {
                tasks.push(self.close_request(document_id));
                return Task::batch(tasks);
            }

            tasks.push(self.close_now(document_id));
        }

        self.clear_close();
        Task::batch(tasks)
    }

    pub(super) fn exit_request(&mut self) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();

        if self.pending_save.is_some() {
            self.file_status = Some(String::from("Finish the current save before closing."));
            return Task::none();
        }

        let dirty_documents = self
            .workspace
            .documents()
            .iter()
            .filter(|document| document.is_dirty)
            .map(|document| document.id)
            .collect::<Vec<_>>();

        if dirty_documents.is_empty() {
            return iced::exit();
        }

        self.close_goal = CloseGoal::ExitApp;
        self.close_documents(dirty_documents)
    }

    fn save_failed(&mut self, document_id: DocumentId, error: FileError) {
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

    fn clear_close(&mut self) {
        self.pending_close_documents.clear();
    }

    fn should_exit(&self) -> bool {
        self.close_goal == CloseGoal::ExitApp
    }

    fn refresh_file_loading_state(&mut self) {
        self.is_loading = self
            .workspace
            .documents()
            .iter()
            .any(|document| document.is_loading());
    }

    fn loading_document_id_for_path(&self, path: &std::path::Path) -> Option<DocumentId> {
        self.workspace
            .documents()
            .iter()
            .find(|document| document.is_loading() && document.path.as_deref() == Some(path))
            .map(|document| document.id)
    }
}
