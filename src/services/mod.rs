//! Side-effecting service modules.

mod atomic_write;
pub mod chunked_file;
pub mod file_cache;
pub mod file_dialogs;
pub mod file_system;
pub mod session_store;
pub mod settings_store;
pub mod types;

pub use chunked_file::{DEFAULT_CHUNK_SIZE, load_file_chunks};
pub use file_cache::{SMALL_FILE_CACHE_LIMIT, SmallFileCacheUpdate, update_small_file_cache};
pub use file_system::{LoadedFile, load_file, open_file, pick_file, save_file, save_file_as};
pub use session_store::{flush_session, load_session, save_session};
pub use settings_store::{flush_settings, load_settings, save_settings};
pub use types::{
    FileError, FileOpenResult, FileResult, FileSaveResult, SettingsLoadResult, SettingsSaveResult,
};
