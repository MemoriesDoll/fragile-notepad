//! Chunked file loading for responsive open/drop flows.

use crate::core::{TextEncoding, decode_bytes};
use crate::message::{
    FileError, FileLoadChunk, FileLoadEvent, FileLoadFailure, FileLoadFinished, FileLoadProgress,
    FileLoadRequest,
};

use futures::executor::block_on;
use iced::futures::{SinkExt, channel::mpsc};
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;
const UTF8_BOM_BYTES: &[u8] = &[0xef, 0xbb, 0xbf];
const UTF16BE_BOM_BYTES: &[u8] = &[0xfe, 0xff];
const UTF16LE_BOM_BYTES: &[u8] = &[0xff, 0xfe];

pub fn load_file_chunks(
    request: FileLoadRequest,
) -> impl iced::futures::Stream<Item = FileLoadEvent> {
    iced::stream::channel(8, async move |sender| {
        std::thread::spawn(move || load_file_on_thread(request, sender));
    })
}

fn load_file_on_thread(request: FileLoadRequest, mut sender: mpsc::Sender<FileLoadEvent>) {
    let total_bytes = std::fs::metadata(&request.path)
        .ok()
        .map(|metadata| metadata.len());

    send_progress(&mut sender, &request, 0, total_bytes);

    let mut file = match File::open(&request.path) {
        Ok(file) => file,
        Err(error) => {
            send_failure(&mut sender, request, FileError::Io(error.kind()));
            return;
        }
    };

    let chunk_size = request.chunk_size.max(1);
    let mut first_read = vec![0; chunk_size];
    let read = match file.read(&mut first_read) {
        Ok(read) => read,
        Err(error) => {
            send_failure(&mut sender, request, FileError::Io(error.kind()));
            return;
        }
    };

    if read == 0 {
        send_terminal(
            &mut sender,
            FileLoadEvent::Finished(Ok(FileLoadFinished {
                document_id: request.document_id,
                generation: request.generation,
                path: request.path,
                encoding: TextEncoding::Utf8,
                had_errors: false,
                fallback_contents: None,
                bytes_read: 0,
                total_bytes,
            })),
        );
        return;
    }

    first_read.truncate(read);
    while first_read.len() < UTF8_BOM_BYTES.len() {
        let mut byte = [0; 1];
        let read = match file.read(&mut byte) {
            Ok(read) => read,
            Err(error) => {
                send_failure(&mut sender, request, FileError::Io(error.kind()));
                return;
            }
        };
        if read == 0 {
            break;
        }
        first_read.push(byte[0]);
    }
    let encoding = detect_initial_encoding(&first_read);

    if matches!(encoding, TextEncoding::Utf8 | TextEncoding::Utf8Bom) {
        load_utf8_chunks(request, sender, file, first_read, encoding, total_bytes);
    } else {
        load_legacy_chunks(request, sender, file, first_read, total_bytes);
    }
}

fn load_utf8_chunks(
    request: FileLoadRequest,
    mut sender: mpsc::Sender<FileLoadEvent>,
    mut file: File,
    first_read: Vec<u8>,
    encoding: TextEncoding,
    total_bytes: Option<u64>,
) {
    let chunk_size = request.chunk_size.max(1);
    let mut buffer = vec![0; chunk_size];
    let mut bytes_read = first_read.len() as u64;
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut pending = strip_initial_bom(&first_read, encoding);
    let mut had_errors = false;

    loop {
        if send_decoded_chunk(
            &mut sender,
            &request,
            &mut decoder,
            &pending,
            false,
            bytes_read,
            total_bytes,
        ) {
            had_errors = true;
            load_legacy_from_start(request, sender, total_bytes, had_errors);
            return;
        }

        let read = match file.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                send_failure(&mut sender, request, FileError::Io(error.kind()));
                return;
            }
        };

        if read == 0 {
            break;
        }

        pending.clear();
        pending.extend_from_slice(&buffer[..read]);
        bytes_read += read as u64;
    }

    if send_decoded_chunk(
        &mut sender,
        &request,
        &mut decoder,
        &[],
        true,
        bytes_read,
        total_bytes,
    ) {
        had_errors = true;
        load_legacy_from_start(request, sender, total_bytes, had_errors);
        return;
    }

    send_terminal(
        &mut sender,
        FileLoadEvent::Finished(Ok(FileLoadFinished {
            document_id: request.document_id,
            generation: request.generation,
            path: request.path,
            encoding,
            had_errors,
            fallback_contents: None,
            bytes_read,
            total_bytes,
        })),
    );
}

fn load_legacy_from_start(
    request: FileLoadRequest,
    sender: mpsc::Sender<FileLoadEvent>,
    total_bytes: Option<u64>,
    had_errors: bool,
) {
    let mut file = match File::open(&request.path) {
        Ok(file) => file,
        Err(error) => {
            let mut sender = sender;
            send_failure(&mut sender, request, FileError::Io(error.kind()));
            return;
        }
    };

    load_legacy_chunks_with_error_state(
        request,
        sender,
        &mut file,
        Vec::new(),
        total_bytes,
        had_errors,
    );
}

fn load_legacy_chunks(
    request: FileLoadRequest,
    sender: mpsc::Sender<FileLoadEvent>,
    mut file: File,
    first_read: Vec<u8>,
    total_bytes: Option<u64>,
) {
    load_legacy_chunks_with_error_state(request, sender, &mut file, first_read, total_bytes, false);
}

fn load_legacy_chunks_with_error_state(
    request: FileLoadRequest,
    mut sender: mpsc::Sender<FileLoadEvent>,
    file: &mut File,
    first_read: Vec<u8>,
    total_bytes: Option<u64>,
    forced_had_errors: bool,
) {
    let chunk_size = request.chunk_size.max(1);
    let mut buffer = vec![0; chunk_size];
    let mut all_bytes = first_read;
    let mut bytes_read = all_bytes.len() as u64;

    loop {
        let read = match file.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                send_failure(&mut sender, request, FileError::Io(error.kind()));
                return;
            }
        };

        if read == 0 {
            break;
        }

        let bytes = &buffer[..read];
        bytes_read += read as u64;
        all_bytes.extend_from_slice(bytes);
    }

    let contents = Arc::new(decode_bytes(&all_bytes));
    send_chunk(
        &mut sender,
        &request,
        contents.text.clone(),
        bytes_read,
        total_bytes,
    );
    send_terminal(
        &mut sender,
        FileLoadEvent::Finished(Ok(FileLoadFinished {
            document_id: request.document_id,
            generation: request.generation,
            path: request.path,
            encoding: contents.encoding,
            had_errors: forced_had_errors || contents.had_errors,
            fallback_contents: Some(contents),
            bytes_read,
            total_bytes,
        })),
    );
}

fn detect_initial_encoding(bytes: &[u8]) -> TextEncoding {
    if bytes.starts_with(UTF8_BOM_BYTES) {
        TextEncoding::Utf8Bom
    } else if bytes.starts_with(UTF16BE_BOM_BYTES) {
        TextEncoding::Utf16BeBom
    } else if bytes.starts_with(UTF16LE_BOM_BYTES) {
        TextEncoding::Utf16LeBom
    } else {
        TextEncoding::Utf8
    }
}

fn strip_initial_bom(bytes: &[u8], encoding: TextEncoding) -> Vec<u8> {
    if encoding == TextEncoding::Utf8Bom {
        bytes
            .get(UTF8_BOM_BYTES.len()..)
            .unwrap_or_default()
            .to_vec()
    } else {
        bytes.to_vec()
    }
}

fn send_decoded_chunk(
    sender: &mut mpsc::Sender<FileLoadEvent>,
    request: &FileLoadRequest,
    decoder: &mut encoding_rs::Decoder,
    bytes: &[u8],
    last: bool,
    bytes_read: u64,
    total_bytes: Option<u64>,
) -> bool {
    let max_output = decoder
        .max_utf8_buffer_length(bytes.len())
        .unwrap_or(bytes.len().saturating_mul(3).saturating_add(16));
    let mut output = String::with_capacity(max_output);
    let (_, _, malformed) = decoder.decode_to_string(bytes, &mut output, last);

    if !malformed && !output.is_empty() {
        send_chunk(sender, request, output, bytes_read, total_bytes);
    }

    malformed
}

fn send_chunk(
    sender: &mut mpsc::Sender<FileLoadEvent>,
    request: &FileLoadRequest,
    text: String,
    bytes_read: u64,
    total_bytes: Option<u64>,
) {
    let _ = block_on(sender.send(FileLoadEvent::Chunk(FileLoadChunk {
        document_id: request.document_id,
        generation: request.generation,
        path: request.path.clone(),
        text: Arc::new(text),
        bytes_read,
        total_bytes,
    })));
}

fn send_progress(
    sender: &mut mpsc::Sender<FileLoadEvent>,
    request: &FileLoadRequest,
    bytes_read: u64,
    total_bytes: Option<u64>,
) {
    let _ = sender.try_send(FileLoadEvent::Progress(FileLoadProgress {
        document_id: request.document_id,
        generation: request.generation,
        path: request.path.clone(),
        bytes_read,
        total_bytes,
    }));
}

fn send_failure(
    sender: &mut mpsc::Sender<FileLoadEvent>,
    request: FileLoadRequest,
    error: FileError,
) {
    send_terminal(
        sender,
        FileLoadEvent::Finished(Err(FileLoadFailure {
            document_id: request.document_id,
            generation: request.generation,
            path: request.path,
            error,
        })),
    );
}

fn send_terminal(sender: &mut mpsc::Sender<FileLoadEvent>, event: FileLoadEvent) {
    let _ = block_on(sender.send(event));
}
