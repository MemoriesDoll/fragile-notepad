//! Async file-system helpers for UI task wiring.

use crate::core::{TextEncoding, decode_bytes, encode_text};
use crate::message::{FileError, FileOpenResult, FileSaveResult, OpenedFile};

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

pub type LoadedFile = OpenedFile;
pub type FileResult<T> = Result<T, FileError>;

pub fn open_file(window: &dyn iced::Window) -> impl Future<Output = FileOpenResult> + use<> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Open a text file...")
        .set_parent(&window);

    async move {
        let picked_file = dialog.pick_file().await.ok_or(FileError::DialogClosed)?;

        load_file(picked_file.path().to_owned()).await
    }
}

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

pub fn save_file_as(
    window: &dyn iced::Window,
    contents: Vec<u8>,
) -> impl Future<Output = FileSaveResult> + use<> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Save text file...")
        .set_parent(&window);

    async move {
        let picked_file = dialog.save_file().await.ok_or(FileError::DialogClosed)?;

        save_file(picked_file.path().to_owned(), contents).await
    }
}

pub fn encode_for_save(text: &str, encoding: TextEncoding) -> Result<Vec<u8>, FileError> {
    encode_text(text, encoding).map_err(FileError::Encoding)
}
