//! Async file-system helpers for UI task wiring.

use crate::core::{TextEncoding, decode_bytes, encode_text};
use crate::message::{
    FileError, FileLoadRequest, FileOpenResult, FileResult, FileSaveResult, OpenedFile,
};

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

pub type LoadedFile = OpenedFile;

pub fn pick_file(window: &dyn iced::Window) -> impl Future<Output = FileResult<PathBuf>> + use<> {
    let dialog = rfd::AsyncFileDialog::new()
        .set_title("Open a text file...")
        .set_parent(&window);

    async move {
        dialog
            .pick_file()
            .await
            .map(|picked_file| picked_file.path().to_owned())
            .ok_or(FileError::DialogClosed)
    }
}

pub fn open_file(window: &dyn iced::Window) -> impl Future<Output = FileOpenResult> + use<'_> {
    async move {
        let path = pick_file(window).await?;

        load_file(path).await
    }
}

pub fn load_file_request(request: FileLoadRequest) -> iced::Task<crate::message::Message> {
    iced::Task::run(
        super::chunked_file::load_file_chunks(request),
        |event| match event {
            crate::message::FileLoadEvent::Progress(progress) => {
                crate::message::Message::FileLoadProgress(progress)
            }
            crate::message::FileLoadEvent::Chunk(chunk) => {
                crate::message::Message::FileLoadChunk(chunk)
            }
            crate::message::FileLoadEvent::Finished(result) => {
                crate::message::Message::FileLoadFinished(result)
            }
        },
    )
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
