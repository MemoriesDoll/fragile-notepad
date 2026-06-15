use iced::event::{Event, Status};
use iced::{Task, keyboard, mouse, window};

use crate::core::{KeyBinding, ShortcutCommand};
use crate::editor::EditorAction;
use crate::message::Message;

use super::App;

impl App {
    pub(super) fn update_runtime_event(
        &mut self,
        event: Event,
        _status: Status,
        window_id: window::Id,
    ) -> Task<Message> {
        if let Event::Window(window::Event::FileDropped(path)) = event {
            return self.update_file(Message::FileDropped(window_id, path));
        }

        if matches!(event, Event::Window(window::Event::Focused)) {
            self.focused_window_id = Some(window_id);
        }

        if let Some(command) = self.shortcut_capture_for_event(&event) {
            return self.update_settings(Message::ShortcutCaptured(command.0, command.1));
        }

        if let Some(command) = self.shortcut_for_key_event(&event) {
            return self.update_shortcut(command);
        }

        if let Some(shortcut) = self.shortcut_for_runtime_event(&event, self.keyboard_modifiers) {
            return self.update_shortcut(shortcut);
        }

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            self.keyboard_modifiers = modifiers;
        }

        Task::none()
    }

    pub(super) fn update_shortcut(&mut self, shortcut: ShortcutCommand) -> Task<Message> {
        self.active_menu = None;

        match shortcut {
            ShortcutCommand::ZoomIn => {
                self.settings.zoom_in();
                self.persist_settings()
            }
            ShortcutCommand::ZoomOut => {
                self.settings.zoom_out();
                self.persist_settings()
            }
            ShortcutCommand::ZoomReset => {
                self.settings.reset_zoom();
                self.persist_settings()
            }
            ShortcutCommand::FoldCurrent => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::FoldCurrent)
            }
            ShortcutCommand::UnfoldCurrent => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::UnfoldCurrent)
            }
            ShortcutCommand::ToggleCurrentFold => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::ToggleCurrentFold)
            }
            ShortcutCommand::FoldAll => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::FoldAll)
            }
            ShortcutCommand::UnfoldAll => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::UnfoldAll)
            }
            ShortcutCommand::GoToMatchingDelimiter => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::GoToMatchingDelimiter)
            }
            ShortcutCommand::SelectMatchingDelimiter => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::SelectMatchingDelimiter)
            }
            ShortcutCommand::NextFunction => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::NextFunction)
            }
            ShortcutCommand::PreviousFunction => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::PreviousFunction)
            }
            ShortcutCommand::SelectCurrentFunction => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::SelectCurrentFunction)
            }
            ShortcutCommand::SelectCurrentFunctionBody => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::SelectCurrentFunctionBody)
            }
            ShortcutCommand::AddCaretAbove => self.update_editor(
                self.workspace.active_document_id,
                EditorAction::AddCaretAbove,
            ),
            ShortcutCommand::AddCaretBelow => self.update_editor(
                self.workspace.active_document_id,
                EditorAction::AddCaretBelow,
            ),
            ShortcutCommand::SplitSelectionIntoLines => self.update_editor(
                self.workspace.active_document_id,
                EditorAction::SplitSelectionIntoLines,
            ),
            ShortcutCommand::ConvertSelectionToRectangle => self.update_editor(
                self.workspace.active_document_id,
                EditorAction::ConvertSelectionToRectangle,
            ),
            ShortcutCommand::NewFile => self.update_file(Message::NewFile),
            ShortcutCommand::OpenFile => self.update_file(Message::OpenFile),
            ShortcutCommand::SaveFile => self.update_file(Message::SaveFile),
            ShortcutCommand::SaveFileAs => self.update_file(Message::SaveFileAs),
            ShortcutCommand::ToggleFind => self.update_search(Message::ToggleFind),
            ShortcutCommand::AdvancedFind => self.update_search(Message::ToggleAdvancedSearch(
                crate::message::AdvancedSearchTab::Find,
            )),
            ShortcutCommand::AdvancedReplace => self.update_search(Message::ToggleAdvancedSearch(
                crate::message::AdvancedSearchTab::Replace,
            )),
            ShortcutCommand::Indent => {
                let document_id = self.workspace.active_document_id;
                let text = match self.settings.indentation {
                    crate::core::IndentationMode::Tabs => "\t".to_owned(),
                    crate::core::IndentationMode::Spaces(width) => " ".repeat(width as usize),
                };

                self.update_editor(document_id, EditorAction::InsertText(text))
            }
            ShortcutCommand::Unindent => {
                let document_id = self.workspace.active_document_id;

                self.update_editor(document_id, EditorAction::Unindent)
            }
            ShortcutCommand::Cut => {
                self.update_editor(self.workspace.active_document_id, EditorAction::Cut)
            }
            ShortcutCommand::Copy => {
                self.update_editor(self.workspace.active_document_id, EditorAction::Copy)
            }
            ShortcutCommand::Paste => {
                self.update_editor(self.workspace.active_document_id, EditorAction::Paste)
            }
            ShortcutCommand::Undo => self.update_editor_command(Message::Undo),
            ShortcutCommand::Redo => self.update_editor_command(Message::Redo),
            ShortcutCommand::SelectAll => {
                self.update_editor(self.workspace.active_document_id, EditorAction::SelectAll)
            }
            ShortcutCommand::DuplicateLine => self.update_editor(
                self.workspace.active_document_id,
                EditorAction::DuplicateLine,
            ),
            ShortcutCommand::DeleteLine => {
                self.update_editor(self.workspace.active_document_id, EditorAction::DeleteLine)
            }
            ShortcutCommand::CopyLine => {
                self.update_editor(self.workspace.active_document_id, EditorAction::CopyLine)
            }
            ShortcutCommand::CutLine => {
                self.update_editor(self.workspace.active_document_id, EditorAction::CutLine)
            }
        }
    }

    fn shortcut_capture_for_event(
        &mut self,
        event: &Event,
    ) -> Option<(ShortcutCommand, KeyBinding)> {
        let command = self.settings_dialog.capturing_shortcut?;
        let Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            modifiers,
            ..
        }) = event
        else {
            return None;
        };
        let binding = KeyBinding::from_event(modified_key, *modifiers)
            .or_else(|| KeyBinding::from_event(key, *modifiers))?;

        Some((command, binding))
    }

    pub(super) fn shortcut_for_runtime_event(
        &self,
        event: &Event,
        modifiers: keyboard::Modifiers,
    ) -> Option<ShortcutCommand> {
        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if command_or_control(modifiers) =>
            {
                shortcut_for_scroll(*delta)
            }
            _ => None,
        }
    }

    fn shortcut_for_key_event(&self, event: &Event) -> Option<ShortcutCommand> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modified_key,
                modifiers,
                ..
            }) => self
                .settings
                .shortcuts
                .resolve(key, modified_key, *modifiers),
            _ => None,
        }
    }
}

pub fn event_to_message(event: Event, status: Status, window_id: window::Id) -> Option<Message> {
    should_forward_runtime_event(&event, status)
        .then_some(Message::RuntimeEvent(event, status, window_id))
}

fn should_forward_runtime_event(event: &Event, status: Status) -> bool {
    status == Status::Ignored
        || matches!(
            event,
            Event::Keyboard(keyboard::Event::ModifiersChanged(_))
                | Event::Mouse(mouse::Event::WheelScrolled { .. })
                | Event::Window(window::Event::FileDropped(_))
        )
}

fn shortcut_for_scroll(delta: mouse::ScrollDelta) -> Option<ShortcutCommand> {
    let y = match delta {
        mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
    };

    if y > 0.0 {
        Some(ShortcutCommand::ZoomIn)
    } else if y < 0.0 {
        Some(ShortcutCommand::ZoomOut)
    } else {
        None
    }
}

fn command_or_control(modifiers: keyboard::Modifiers) -> bool {
    modifiers.command() || modifiers.control()
}

#[cfg(test)]
mod tests {
    use super::{event_to_message, shortcut_for_scroll, should_forward_runtime_event};
    use crate::core::ShortcutCommand;
    use crate::message::Message;
    use iced::event::Event;
    use iced::{keyboard, mouse, window};
    use std::path::PathBuf;

    #[test]
    fn scroll_direction_maps_to_zoom_shortcuts() {
        assert_eq!(
            shortcut_for_scroll(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            Some(ShortcutCommand::ZoomIn)
        );
        assert_eq!(
            shortcut_for_scroll(mouse::ScrollDelta::Pixels { x: 0.0, y: -1.0 }),
            Some(ShortcutCommand::ZoomOut)
        );
        assert_eq!(
            shortcut_for_scroll(mouse::ScrollDelta::Lines { x: 0.0, y: 0.0 }),
            None
        );
    }

    #[test]
    fn captured_wheel_and_modifier_events_are_forwarded_to_shortcut_processor() {
        let wheel = Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        });
        let modifiers =
            Event::Keyboard(keyboard::Event::ModifiersChanged(keyboard::Modifiers::CTRL));
        let captured_key = Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character("x".into()),
            modified_key: keyboard::Key::Character("x".into()),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        });

        assert!(should_forward_runtime_event(
            &wheel,
            iced::event::Status::Captured
        ));
        assert!(should_forward_runtime_event(
            &modifiers,
            iced::event::Status::Captured
        ));
        assert!(!should_forward_runtime_event(
            &captured_key,
            iced::event::Status::Captured
        ));
    }

    #[test]
    fn file_drop_events_are_forwarded_even_when_captured() {
        let path = PathBuf::from("dropped.txt");
        let event = Event::Window(window::Event::FileDropped(path.clone()));
        let window_id = window::Id::unique();

        assert!(should_forward_runtime_event(
            &event,
            iced::event::Status::Captured
        ));

        assert!(matches!(
            event_to_message(event, iced::event::Status::Captured, window_id),
            Some(Message::RuntimeEvent(
                Event::Window(window::Event::FileDropped(forwarded_path)),
                iced::event::Status::Captured,
                forwarded_window_id
            )) if forwarded_path == path && forwarded_window_id == window_id
        ));
    }
}
