//! Side-effecting service modules.

mod atomic_write;
pub mod file_system;
pub mod settings_store;

pub use crate::message::{
    FileError, FileOpenResult, FileSaveResult, SettingsLoadResult, SettingsSaveResult,
};
pub use file_system::{FileResult, LoadedFile, load_file, open_file, save_file, save_file_as};
pub use settings_store::{load_settings, save_settings};
