use iced::{Task, widget::operation};

use super::App;
use crate::core::DocumentId;
use crate::editor::{EditorPosition, EditorSelection};
use crate::message::Message;
use crate::ui::go_to_line_prompt::INPUT_ID;

#[derive(Debug)]
pub(super) struct GoToLinePrompt {
    document_id: DocumentId,
    pub input: String,
    pub error: Option<String>,
}

impl App {
    pub(super) fn update_go_to_line(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::GoToLineOpened => {
                if self.close_prompt.document().is_some()
                    || self.is_about_visible
                    || self.is_window_list_visible
                {
                    return Task::none();
                }
                let Some(document) = self.workspace.active_document_mut() else {
                    return Task::none();
                };
                document.sync_selection_mirror();
                self.go_to_line_prompt = Some(GoToLinePrompt {
                    document_id: document.id,
                    input: (document.main_selection().cursor.line + 1).to_string(),
                    error: None,
                });
                self.active_menu = None;
                self.active_menu_path.clear();
                self.main_window_id
                    .map(iced::window::gain_focus)
                    .unwrap_or_else(Task::none)
                    .chain(operation::focus(INPUT_ID))
                    .chain(operation::select_all(INPUT_ID))
            }
            Message::GoToLineChanged(input) => {
                if let Some(prompt) = &mut self.go_to_line_prompt {
                    prompt.input = input;
                    prompt.error = None;
                }
                Task::none()
            }
            Message::GoToLineSubmitted => {
                let Some(prompt) = &mut self.go_to_line_prompt else {
                    return Task::none();
                };
                let Ok(line_number) = prompt.input.trim().parse::<usize>() else {
                    prompt.error = Some("Enter a valid line number.".into());
                    return operation::focus(INPUT_ID);
                };
                let Some(document) = self.workspace.active_document_mut() else {
                    return self.update_go_to_line(Message::GoToLineClosed);
                };
                if document.id != prompt.document_id {
                    return self.update_go_to_line(Message::GoToLineClosed);
                }
                if !document.has_complete_text_index() {
                    prompt.error = Some("Wait for the document to finish loading.".into());
                    return operation::focus(INPUT_ID);
                }
                let last_line = document.buffer.line_count().saturating_sub(1);
                let target_line = line_number.saturating_sub(1).min(last_line);
                let position = document
                    .buffer
                    .clamp_position(EditorPosition::new(target_line, 0));
                document.set_main_selection(EditorSelection::new(position, position));
                document.reveal_position(position);
                self.update_go_to_line(Message::GoToLineClosed)
            }
            Message::GoToLineClosed => {
                self.go_to_line_prompt = None;
                operation::focus(crate::ui::editor::EDITOR_ID)
            }
            _ => unreachable!("go-to-line handler received unrelated message"),
        }
    }
}
