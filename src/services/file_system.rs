//! Disk I/O and encoding helpers.

// Preserve existing callers while native dialogs live in their own adapter.
pub use super::file_dialogs::{
    open_file, pick_file, save_file_as, save_file_as_with_options, save_file_copy_as,
    save_file_copy_as_with_options,
};

use super::types::{FileError, FileOpenResult, FileSaveResult, OpenedFile};
use crate::core::{TextEncoding, decode_bytes, encode_text};

use std::path::PathBuf;
use std::sync::Arc;

pub type LoadedFile = OpenedFile;

pub async fn load_file(path: PathBuf) -> FileOpenResult {
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| FileError::Io(error.kind()))?;
    let contents = Arc::new(decode_bytes(&bytes));

    Ok(OpenedFile { path, contents })
}

pub async fn save_file(path: PathBuf, contents: Vec<u8>) -> FileSaveResult {
    super::atomic_write::write(&path, &contents)
        .await
        .map_err(|error| FileError::Io(error.kind()))?;

    Ok(path)
}

pub fn encode_for_save(text: &str, encoding: TextEncoding) -> Result<Vec<u8>, FileError> {
    encode_text(text, encoding).map_err(FileError::Encoding)
}
