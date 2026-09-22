//! Document close queues and application exit decisions.

use crate::app::{App, CloseGoal};
use crate::core::{DirtyCloseDecision, DocumentId, DocumentLoadState};
use crate::message::Message;
use iced::Task;
use std::collections::VecDeque;

impl App {
    pub(super) fn close_request(&mut self, document_id: DocumentId) -> Task<Message> {
        if self.close_prompt.is_closing() {
            return Task::none();
        }
        if self.workspace.document(document_id).is_some_and(|doc| {
            matches!(doc.load_state, DocumentLoadState::Deferred { .. }) && doc.is_dirty
        }) {
            return self
                .activate_document(document_id)
                .chain(Task::done(Message::TabClosed(document_id)));
        }
        let Some(document) = self.workspace.document(document_id) else {
            return Task::none();
        };

        if !document.is_dirty {
            return self.close_now(document_id);
        }

        self.go_to_line_prompt = None;
        self.close_prompt.show(document_id);
        Task::none()
    }

    pub(super) fn resolve_close(
        &mut self,
        document_id: DocumentId,
        decision: DirtyCloseDecision,
    ) -> Task<Message> {
        self.close_prompt.dismiss(document_id);

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
                    Task::batch([close_task, self.exit_after_settings()])
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

    pub(super) fn close_now(&mut self, document_id: DocumentId) -> Task<Message> {
        if self
            .pending_save
            .as_ref()
            .is_some_and(|request| request.document_id == document_id)
        {
            self.file_status = Some(String::from("Finish the current save before closing."));
            return Task::none();
        }
        self.close_prompt.dismiss(document_id);

        self.workspace.close(document_id);
        if let Some(handle) = self.load_handles.remove(&document_id) {
            handle.abort();
        }
        self.session.pending.remove(&document_id);
        self.session.folds.remove(&document_id);
        self.pending_reloads.remove(&document_id);
        self.refresh_file_loading_state();
        self.outline_parsing.remove(document_id);

        let active_document_id = self.workspace.active_document_id;
        let load = self.activate_document(active_document_id);
        self.refresh_find_matches();
        Task::batch([load, self.schedule_outline_parse(active_document_id)])
    }

    pub(super) fn close_documents(&mut self, document_ids: Vec<DocumentId>) -> Task<Message> {
        self.pending_close_documents = VecDeque::from(document_ids);
        self.continue_close()
    }

    pub(super) fn continue_close(&mut self) -> Task<Message> {
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

    pub(in crate::app) fn exit_request(&mut self) -> Task<Message> {
        if self.close_prompt.is_closing() {
            return Task::none();
        }
        self.active_menu = None;
        self.active_menu_path.clear();

        if self.pending_save.is_some() {
            self.file_status = Some(String::from("Finish the current save before closing."));
            return Task::none();
        }

        if self.session.enabled {
            return self.persist_before_exit();
        }

        let dirty_documents = self
            .workspace
            .documents()
            .iter()
            .filter(|document| document.is_dirty)
            .map(|document| document.id)
            .collect::<Vec<_>>();

        if dirty_documents.is_empty() {
            return self.exit_after_settings();
        }

        self.close_goal = CloseGoal::ExitApp;
        self.close_documents(dirty_documents)
    }

    pub(super) fn clear_close(&mut self) {
        self.pending_close_documents.clear();
    }

    pub(super) fn should_exit(&self) -> bool {
        self.close_goal == CloseGoal::ExitApp
    }
}
