//! Dispatches file commands to the loading, saving, and closing workflows.

use crate::app::{App, CloseGoal};
use crate::message::Message;
use crate::services;
use iced::{Task, window};
use std::path::PathBuf;

mod closing;
mod loading;
mod saving;

impl App {
    pub(super) fn update_file(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(document_id) => {
                self.active_menu = None;
                if self.workspace.select(document_id) {
                    let load = self.activate_document(document_id);
                    self.refresh_find_matches();
                    return Task::batch([load, self.schedule_outline_parse(document_id)]);
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
                    let load = self.activate_document(document_id);
                    self.refresh_find_matches();
                    return Task::batch([load, self.schedule_outline_parse(document_id)]);
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
                    document.set_word_wrap(self.settings.word_wrap);
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
            Message::SaveCopyAs => {
                self.active_menu = None;
                self.file_status = None;
                self.save_copy_active()
            }
            Message::FileSaved(request, result) => self.save_done(request, result),
            Message::FileCopySaved(request, result) => self.save_copy_done(request, result),
            Message::ReloadFromDisk => {
                self.active_menu = None;
                self.reload_active_from_disk()
            }
            Message::EncodingSelected(encoding) => {
                self.active_menu = None;
                if let Some(document) = self.workspace.active_document_mut() {
                    if !document.has_complete_text_index() {
                        self.file_status =
                            Some(String::from("Finish loading before changing encoding."));
                        return Task::none();
                    }
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
                match self.close_prompt.resolve(document_id, decision) {
                    Some(decision) => self.resolve_close(document_id, decision),
                    None => Task::none(),
                }
            }
            Message::DirtyCloseFadeFinished(document_id) => {
                match self.close_prompt.finish(document_id) {
                    Some(decision) => self.resolve_close(document_id, decision),
                    None => Task::none(),
                }
            }
            _ => unreachable!("file handler received non-file message"),
        }
    }

    fn record_open_history(&mut self, path: PathBuf) -> Task<Message> {
        if self.settings.record_open_history_path(path) {
            self.settings_dialog.draft.open_history = self.settings.open_history.clone();
            self.persist_settings()
        } else {
            Task::none()
        }
    }
}
