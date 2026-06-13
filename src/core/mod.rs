//! Pure document and editing state modules.

pub mod commands;
pub mod document;
pub mod encoding;
pub mod search;
pub mod settings;
pub mod shortcuts;
pub mod workspace;

pub use commands::{CoreCommand, EditorCommand, FileCommand, SearchCommand, SettingsCommand};
pub use document::{Document, DocumentId};
pub use encoding::{DecodedText, EncodingError, TextEncoding, decode_bytes, encode_text};
pub use search::{FindState, PreparedSearch, SearchError, SearchMode, SearchOptions, TextMatch};
pub use settings::{AppearanceMode, EditorSettings, HardwareAccelerationMode, IndentationMode};
pub use shortcuts::{
    KeyBinding, ShortcutCommand, ShortcutConflict, ShortcutDisplay, ShortcutDisplayPart,
    ShortcutEntry, ShortcutGroup, ShortcutKey, ShortcutMap, ShortcutModifierIcon,
    ShortcutModifiers,
};
pub use workspace::Workspace;
