use iced::keyboard::{self, key};

use crate::core::ShortcutMap;
use crate::editor::action::{CaretMotion, EditorAction, shortcut_action};

pub fn key_action(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
    text: Option<&str>,
    shortcuts: &ShortcutMap,
) -> Option<EditorAction> {
    shortcuts
        .resolve(key, modified_key, modifiers)
        .and_then(shortcut_action)
        .or_else(|| match modified_key.as_ref() {
            keyboard::Key::Named(key::Named::Space) => {
                Some(EditorAction::InsertText(" ".to_owned()))
            }
            keyboard::Key::Named(key::Named::Tab) if modifiers.shift() => {
                Some(EditorAction::Unindent)
            }
            keyboard::Key::Named(key::Named::Tab) => Some(EditorAction::Indent),
            keyboard::Key::Named(key::Named::Enter) => Some(EditorAction::InsertNewline),
            keyboard::Key::Named(key::Named::Backspace) => Some(EditorAction::Backspace),
            keyboard::Key::Named(key::Named::Delete) => Some(EditorAction::Delete),
            keyboard::Key::Named(named) => {
                let motion = caret_motion(named, modifiers)?;

                Some(if modifiers.shift() {
                    EditorAction::Select(motion)
                } else {
                    EditorAction::MoveCaret(motion)
                })
            }
            _ => text
                .and_then(|text| text.chars().find(|ch| !ch.is_control()))
                .map(|ch| EditorAction::InsertText(ch.to_string())),
        })
}

fn caret_motion(named: key::Named, modifiers: keyboard::Modifiers) -> Option<CaretMotion> {
    match named {
        key::Named::ArrowLeft if modifiers.command() => Some(CaretMotion::WordLeft),
        key::Named::ArrowRight if modifiers.command() => Some(CaretMotion::WordRight),
        key::Named::ArrowLeft => Some(CaretMotion::Left),
        key::Named::ArrowRight => Some(CaretMotion::Right),
        key::Named::ArrowUp => Some(CaretMotion::Up),
        key::Named::ArrowDown => Some(CaretMotion::Down),
        key::Named::Home if modifiers.command() => Some(CaretMotion::DocumentStart),
        key::Named::End if modifiers.command() => Some(CaretMotion::DocumentEnd),
        key::Named::Home => Some(CaretMotion::LineStart),
        key::Named::End => Some(CaretMotion::LineEnd),
        key::Named::PageUp => Some(CaretMotion::PageUp),
        key::Named::PageDown => Some(CaretMotion::PageDown),
        _ => None,
    }
}
