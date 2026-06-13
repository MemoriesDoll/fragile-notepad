use crate::core::document::DocumentId;
use crate::core::settings::{AppearanceMode, IndentationMode};
use crate::editor::EditorAction;

use iced::highlighter;

#[derive(Debug, Clone, PartialEq)]
pub enum CoreCommand {
    File(FileCommand),
    Editor(EditorCommand),
    Search(SearchCommand),
    Settings(SettingsCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileCommand {
    New,
    Open,
    Save(DocumentId),
    SaveAs(DocumentId),
    Close(DocumentId),
    Select(DocumentId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorCommand {
    Action(DocumentId, EditorAction),
    MarkClean(DocumentId),
    MarkDirty(DocumentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchCommand {
    SetQuery(String),
    SetReplacement(String),
    SetCaseSensitive(bool),
    FindNext,
    FindPrevious,
    ReplaceCurrent,
    ReplaceAll,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsCommand {
    SetWordWrap(bool),
    SetZoom(f32),
    ZoomIn,
    ZoomOut,
    ResetZoom,
    SetIndentation(IndentationMode),
    SetAppearance(AppearanceMode),
    SetSyntaxTheme(highlighter::Theme),
}
