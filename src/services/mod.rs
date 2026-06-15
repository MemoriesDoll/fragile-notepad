//! Side-effecting service modules.

mod atomic_write;
pub mod chunked_file;
pub mod file_system;
pub mod settings_store;

pub use crate::message::{
    FileError, FileOpenResult, FileResult, FileSaveResult, SettingsLoadResult, SettingsSaveResult,
};
pub use chunked_file::{DEFAULT_CHUNK_SIZE, load_file_chunks};
pub use file_system::{
    LoadedFile, load_file, load_file_request, open_file, pick_file, save_file, save_file_as,
};
pub use settings_store::{load_settings, save_settings};
