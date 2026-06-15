use iced::event;
use iced::highlighter;
use iced::window;

use crate::core::{
    AppearanceMode, DecodedText, DocumentId, EditorSettings, EncodingError,
    HardwareAccelerationMode, IndentationMode, KeyBinding, SearchMode, ShortcutCommand,
    ShortcutConflict, TextEncoding,
};
use crate::editor::{
    EditorAction, EditorPosition, EditorSelection, OutlineParseResult, SelectionSet,
};
use crate::ipc::ActivationRequest;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    Search,
    View,
    Encoding,
    Language,
    Settings,
    Window,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuPath {
    pub depth: usize,
    pub segments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    General,
    Appearance,
    Editor,
    Shortcuts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedSearchTab {
    Find,
    Replace,
    FindInFiles,
    ReplaceInFiles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AboutTab {
    About,
    Debug,
    Licenses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowTarget {
    Main,
    Settings,
    AdvancedSearch,
}

#[derive(Debug, Clone)]
pub enum Message {
    None,
    Shortcut(ShortcutCommand),
    EditorAction(DocumentId, EditorAction),
    OutlineParseCompleted(OutlineParseResult),
    ClipboardRead(PasteRequest, ClipboardReadResult),
    ClipboardWritten(ClipboardWriteResult),
    BackendBoostRequested,
    BackendBoostConfigured(iced::backend::StrictHandoffOutcome),
    AboutAnimationFrame(Instant),
    RuntimeEvent(event::Event, event::Status, window::Id),
    MenuToggled(Menu),
    MenuHovered(Menu),
    MenuPathHovered(MenuPath),
    MenuClosed,
    DraftThemeSelected(highlighter::Theme),
    DraftWordWrapToggled(bool),
    DraftAppearanceSelected(AppearanceMode),
    DraftHardwareAccelerationSelected(HardwareAccelerationMode),
    DraftIndentationSelected(IndentationMode),
    DraftLineNumbersToggled(bool),
    DraftVisibleSpacesToggled(bool),
    DraftVisibleTabsToggled(bool),
    DraftEolMarkersToggled(bool),
    DraftIndentationGuidesToggled(bool),
    DraftFoldingControlsToggled(bool),
    SettingsCategorySelected(SettingsCategory),
    ShortcutGroupSelected(crate::core::ShortcutGroup),
    SettingsZoomIn,
    SettingsZoomOut,
    SettingsZoomReset,
    SettingsScrollSpeedIncrease,
    SettingsScrollSpeedDecrease,
    SettingsScrollSpeedReset,
    ApplySettings,
    SaveSettings,
    SettingsLoaded(SettingsLoadResult),
    SettingsPersisted(SettingsSaveResult),
    CancelSettings,
    ToggleSettingsPanel,
    ToggleFunctionList,
    FunctionListEntrySelected(EditorPosition),
    ShortcutCaptureStarted(ShortcutCommand),
    ShortcutCaptured(ShortcutCommand, KeyBinding),
    ShortcutCleared(ShortcutCommand),
    ShortcutsResetToDefaults,
    ShortcutConflictDismissed,
    ShortcutCaptureConflict(ShortcutConflict),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleWordWrap,
    ToggleLineNumbers,
    ToggleSpaceAndTab,
    ToggleVisibleSpaces,
    ToggleVisibleTabs,
    ToggleEolMarkers,
    ToggleAllCharacters,
    ToggleIndentationGuides,
    ToggleFoldingControls,
    FoldCurrent,
    UnfoldCurrent,
    ToggleCurrentFold,
    FoldAll,
    UnfoldAll,
    GoToMatchingDelimiter,
    SelectMatchingDelimiter,
    NextFunction,
    PreviousFunction,
    SelectCurrentFunction,
    SelectCurrentFunctionBody,
    Cut,
    Copy,
    Paste,
    Delete,
    EncodingSelected(TextEncoding),
    Undo,
    Redo,
    TabSelected(DocumentId),
    TabClosed(DocumentId),
    TabPinToggled(DocumentId),
    TabDragStarted(DocumentId),
    TabDragHovered(DocumentId),
    TabDragLeft(DocumentId),
    TabDragReleased(DocumentId),
    NewFile,
    OpenFile,
    FileDropped(window::Id, PathBuf),
    FileOpened(FileOpenResult),
    SaveFile,
    SaveAllFiles,
    SaveFileAs,
    FileSaved(SaveRequest, FileSaveResult),
    CloseFile,
    CloseAllFiles,
    CloseAllButActiveFile,
    CloseAllButPinnedFiles,
    CloseAllToLeft,
    CloseAllToRight,
    CloseAllUnchanged,
    DirtyCloseResolved(DocumentId, DirtyCloseDecision),
    FindQueryChanged(String),
    FindReplacementChanged(String),
    FindCaseSensitiveToggled(bool),
    FindWholeWordToggled(bool),
    ToggleInlineReplace,
    ShowInlineReplace,
    ToggleFind,
    HideFind,
    ToggleAdvancedSearch(AdvancedSearchTab),
    AdvancedSearchTabSelected(AdvancedSearchTab),
    AdvancedSearchQueryChanged(String),
    AdvancedSearchReplacementChanged(String),
    AdvancedSearchCaseSensitiveToggled(bool),
    AdvancedSearchWholeWordToggled(bool),
    AdvancedSearchWrapAroundToggled(bool),
    AdvancedSearchModeSelected(SearchMode),
    AdvancedSearchIncludeChanged(String),
    AdvancedSearchRun,
    AdvancedCountRun,
    AdvancedFindNextRun,
    AdvancedFindAllCurrentRun,
    AdvancedFindAllOpenRun,
    AdvancedReplaceRun,
    AdvancedReplaceAllRun,
    AdvancedReplaceAllCurrentRun,
    AdvancedReplaceAllOpenRun,
    AdvancedSearchResultSelected(DocumentId, EditorSelection),
    AdvancedSearchClosed,
    AboutOpened,
    AboutTabSelected(AboutTab),
    AboutClosed,
    WindowListOpened,
    WindowListClosed,
    WindowFocusRequested(WindowTarget),
    WindowFocusNext,
    WindowFocusPrevious,
    WindowOpened(window::Id),
    WindowCloseRequested(window::Id),
    WindowClosed(window::Id),
    SingleInstanceShowRequested(ActivationRequest),
    FindNext,
    FindPrevious,
    SelectAndFindNext,
    SelectAndFindPrevious,
    VolatileFindNext,
    VolatileFindPrevious,
    ReplaceCurrent,
    ReplaceAll,
    LanguageSelected(String),
}

pub type FileOpenResult = Result<OpenedFile, FileError>;
pub type FileSaveResult = Result<PathBuf, FileError>;
pub type SettingsLoadResult = Result<Option<EditorSettings>, SettingsError>;
pub type SettingsSaveResult = Result<(), SettingsError>;
pub type ClipboardReadResult = Result<Arc<String>, iced::clipboard::Error>;
pub type ClipboardWriteResult = Result<(), iced::clipboard::Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardMode {
    Linear,
    Rectangular { line_count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteRequest {
    pub document_id: DocumentId,
    pub selection: EditorSelection,
    pub selection_set: SelectionSet,
    pub clipboard_mode: ClipboardMode,
}

#[derive(Debug, Clone)]
pub struct SaveRequest {
    pub document_id: DocumentId,
    pub snapshot: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyCloseDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct OpenedFile {
    pub path: PathBuf,
    pub contents: Arc<DecodedText>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileError {
    DialogClosed,
    Io(io::ErrorKind),
    Encoding(EncodingError),
}

impl FileError {
    pub fn summary(&self) -> &'static str {
        match self {
            Self::DialogClosed => "dialog closed",
            Self::Io(_) => "I/O error",
            Self::Encoding(_) => "encoding error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    Unavailable,
    Io(io::ErrorKind),
}

impl SettingsError {
    pub fn summary(&self) -> &'static str {
        match self {
            Self::Unavailable => "settings directory unavailable",
            Self::Io(_) => "I/O error",
        }
    }
}
