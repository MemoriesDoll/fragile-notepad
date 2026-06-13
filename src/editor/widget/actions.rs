use iced::keyboard::{self, key};

use crate::core::{ShortcutCommand, ShortcutMap};
use crate::editor::fold::FoldRange;
use crate::editor::position::{EditorPosition, EditorSelection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    InsertText(String),
    InsertNewline,
    Backspace,
    Delete,
    MoveCaret(CaretMotion),
    Select(CaretMotion),
    SelectAll,
    ReplaceSelection(String),
    Indent,
    Unindent,
    DuplicateLine,
    DeleteLine,
    CopyLine,
    CutLine,
    ScrollLines(i32),
    ScrollToRow(usize),
    ToggleFold(FoldRange),
    FoldCurrent,
    UnfoldCurrent,
    ToggleCurrentFold,
    FoldAll,
    UnfoldAll,
    GoToMatchingDelimiter,
    SelectMatchingDelimiter,
    SelectMatchingDelimiterInPlace,
    NextFunction,
    PreviousFunction,
    SelectCurrentFunction,
    SelectCurrentFunctionBody,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Focus,
    PlaceCaret(EditorPosition),
    SelectWordAt(EditorPosition),
    SelectRegion(EditorSelection),
    AddCaretAbove,
    AddCaretBelow,
    SplitSelectionIntoLines,
    ConvertSelectionToRectangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretMotion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    ParagraphUp,
    ParagraphDown,
    LineStart,
    LineEnd,
    PageUp,
    PageDown,
    DocumentStart,
    DocumentEnd,
}

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

pub fn shortcut_action(command: ShortcutCommand) -> Option<EditorAction> {
    match command {
        ShortcutCommand::Cut => Some(EditorAction::Cut),
        ShortcutCommand::Copy => Some(EditorAction::Copy),
        ShortcutCommand::Paste => Some(EditorAction::Paste),
        ShortcutCommand::Undo => Some(EditorAction::Undo),
        ShortcutCommand::Redo => Some(EditorAction::Redo),
        ShortcutCommand::SelectAll => Some(EditorAction::SelectAll),
        ShortcutCommand::DuplicateLine => Some(EditorAction::DuplicateLine),
        ShortcutCommand::DeleteLine => Some(EditorAction::DeleteLine),
        ShortcutCommand::CopyLine => Some(EditorAction::CopyLine),
        ShortcutCommand::CutLine => Some(EditorAction::CutLine),
        ShortcutCommand::FoldCurrent => Some(EditorAction::FoldCurrent),
        ShortcutCommand::UnfoldCurrent => Some(EditorAction::UnfoldCurrent),
        ShortcutCommand::ToggleCurrentFold => Some(EditorAction::ToggleCurrentFold),
        ShortcutCommand::FoldAll => Some(EditorAction::FoldAll),
        ShortcutCommand::UnfoldAll => Some(EditorAction::UnfoldAll),
        ShortcutCommand::GoToMatchingDelimiter => Some(EditorAction::GoToMatchingDelimiter),
        ShortcutCommand::SelectMatchingDelimiter => Some(EditorAction::SelectMatchingDelimiter),
        ShortcutCommand::NextFunction => Some(EditorAction::NextFunction),
        ShortcutCommand::PreviousFunction => Some(EditorAction::PreviousFunction),
        ShortcutCommand::SelectCurrentFunction => Some(EditorAction::SelectCurrentFunction),
        ShortcutCommand::SelectCurrentFunctionBody => Some(EditorAction::SelectCurrentFunctionBody),
        ShortcutCommand::AddCaretAbove => Some(EditorAction::AddCaretAbove),
        ShortcutCommand::AddCaretBelow => Some(EditorAction::AddCaretBelow),
        ShortcutCommand::SplitSelectionIntoLines => Some(EditorAction::SplitSelectionIntoLines),
        ShortcutCommand::ConvertSelectionToRectangle => {
            Some(EditorAction::ConvertSelectionToRectangle)
        }
        ShortcutCommand::Indent => Some(EditorAction::Indent),
        ShortcutCommand::Unindent => Some(EditorAction::Unindent),
        _ => None,
    }
}

fn caret_motion(named: key::Named, modifiers: keyboard::Modifiers) -> Option<CaretMotion> {
    match named {
        key::Named::ArrowLeft if modifiers.command() => Some(CaretMotion::WordLeft),
        key::Named::ArrowRight if modifiers.command() => Some(CaretMotion::WordRight),
        key::Named::ArrowLeft => Some(CaretMotion::Left),
        key::Named::ArrowRight => Some(CaretMotion::Right),
        key::Named::ArrowUp => Some(CaretMotion::Up),
        key::Named::ArrowDown => Some(CaretMotion::Down),
        key::Named::Home => Some(CaretMotion::LineStart),
        key::Named::End => Some(CaretMotion::LineEnd),
        key::Named::PageUp => Some(CaretMotion::PageUp),
        key::Named::PageDown => Some(CaretMotion::PageDown),
        _ => None,
    }
}
