use fragile_notepad::core::{Document, DocumentId, DocumentLoadGeneration};
use fragile_notepad::message::{FileLoadEvent, FileLoadRequest};
use fragile_notepad::services::{DEFAULT_CHUNK_SIZE, load_file_chunks};
use fragile_notepad::ui::status_bar::document_status_label;
use futures::{StreamExt, pin_mut};
use iced::widget::text_editor::LineEnding;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

fn temp_file_path(name: &str) -> PathBuf {
    let id = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fragile-notepad-large-file-loading-{name}-{id}.txt"
    ))
}

fn collect_load_events(request: FileLoadRequest) -> Vec<FileLoadEvent> {
    futures::executor::block_on(async {
        let stream = load_file_chunks(request);
        pin_mut!(stream);
        stream.collect::<Vec<_>>().await
    })
}

#[test]
fn chunked_loader_streams_progress_chunks_and_final_decoded_text() {
    let path = temp_file_path("utf8-crlf-bom");
    let mut bytes = Vec::from(&b"\xef\xbb\xbfalpha\r\ncaf"[..]);
    bytes.extend_from_slice("é\r\nomega".as_bytes());
    fs::write(&path, &bytes).expect("write temp input");

    let document_id = DocumentId::new(41);
    let generation = DocumentLoadGeneration::next();
    let events = collect_load_events(FileLoadRequest {
        document_id,
        generation,
        path: path.clone(),
        chunk_size: 1,
    });

    let _ = fs::remove_file(&path);

    assert!(
        matches!(events.first(), Some(FileLoadEvent::Progress(progress))
        if progress.document_id == document_id
            && progress.generation == generation
            && progress.bytes_read == 0
            && progress.total_bytes == Some(bytes.len() as u64))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, FileLoadEvent::Chunk(chunk)
            if chunk.document_id == document_id
                && chunk.generation == generation
                && chunk.bytes_read > 0
                && chunk.total_bytes == Some(bytes.len() as u64)))
    );

    let finished = events
        .iter()
        .find_map(|event| match event {
            FileLoadEvent::Finished(Ok(finished)) => Some(finished),
            _ => None,
        })
        .expect("finished event");

    assert_eq!(finished.document_id, document_id);
    assert_eq!(finished.generation, generation);
    assert_eq!(finished.bytes_read, bytes.len() as u64);
    assert_eq!(finished.total_bytes, Some(bytes.len() as u64));
    assert_eq!(
        finished.encoding,
        fragile_notepad::core::TextEncoding::Utf8Bom
    );
    let streamed = events
        .iter()
        .filter_map(|event| match event {
            FileLoadEvent::Chunk(chunk) => Some(chunk.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(streamed.as_bytes(), &bytes[3..]);
}

#[test]
fn loading_document_applies_matching_progress_and_completion_only() {
    let document_id = DocumentId::new(42);
    let generation = DocumentLoadGeneration::next();
    let stale_generation = DocumentLoadGeneration::next();
    let mut document = Document::loading(document_id, "large.txt", generation);

    assert_eq!(document_status_label(&document, Some("Saved")), "indexing");
    assert!(document.update_load_progress(generation, 5, Some(20)));
    assert!(!document.replace_loading_preview(stale_generation, "stale", 5, Some(5)));
    assert_eq!(document.text(), "");
    assert!(document.replace_loading_preview(generation, "alpha\r", 6, Some(12)));
    assert!(document.replace_loading_preview(generation, "\nbeta", 11, Some(12)));
    assert_eq!(document.text(), "alpha\r\nbeta");
    assert_eq!(document.buffer.line(0).as_deref(), Some("alpha"));
    assert_eq!(document.buffer.line(1).as_deref(), Some("beta"));
    assert!(!document.complete_loading(
        stale_generation,
        fragile_notepad::core::decode_bytes(b"stale")
    ));
    assert_eq!(document.text(), "alpha\r\nbeta");
    assert!(document.is_loading_or_indexing());

    assert!(document.complete_loading(
        generation,
        fragile_notepad::core::decode_bytes(b"alpha\r\nbeta\n")
    ));

    assert_eq!(document.text(), "alpha\r\nbeta\n");
    assert_eq!(document.line_ending, Some(LineEnding::CrLf));
    assert!(!document.is_loading_or_indexing());
    assert_eq!(document_status_label(&document, Some("Saved")), "Saved");
}

#[test]
fn chunked_loader_finished_result_can_replace_lossy_preview() {
    let path = temp_file_path("split-multibyte");
    fs::write(&path, "AéB").expect("write temp input");

    let document_id = DocumentId::new(43);
    let generation = DocumentLoadGeneration::next();
    let mut document = Document::loading(document_id, path.clone(), generation);
    let events = collect_load_events(FileLoadRequest {
        document_id,
        generation,
        path: path.clone(),
        chunk_size: 1,
    });

    let _ = fs::remove_file(&path);

    for event in events {
        match event {
            FileLoadEvent::Progress(progress) => {
                document.update_load_progress(
                    progress.generation,
                    progress.bytes_read,
                    progress.total_bytes,
                );
            }
            FileLoadEvent::Chunk(chunk) => {
                document.replace_loading_preview(
                    chunk.generation,
                    &chunk.text,
                    chunk.bytes_read,
                    chunk.total_bytes,
                );
            }
            FileLoadEvent::Finished(Ok(finished)) => {
                assert!(document.complete_streaming_load(finished.generation, finished.encoding));
            }
            FileLoadEvent::Finished(Err(error)) => panic!("load failed: {error:?}"),
        }
    }

    assert_eq!(document.text(), "AéB");
    assert_eq!(
        document.bytes_for_save().expect("save bytes"),
        "AéB".as_bytes()
    );
}

#[test]
fn chunked_loader_preserves_legacy_fallback_decoding() {
    let path = temp_file_path("windows-1252");
    let bytes = b"caf\xe9".to_vec();
    fs::write(&path, &bytes).expect("write temp input");

    let document_id = DocumentId::new(45);
    let generation = DocumentLoadGeneration::next();
    let events = collect_load_events(FileLoadRequest {
        document_id,
        generation,
        path: path.clone(),
        chunk_size: 1,
    });

    let _ = fs::remove_file(&path);

    let finished = events
        .iter()
        .find_map(|event| match event {
            FileLoadEvent::Finished(Ok(finished)) => Some(finished),
            _ => None,
        })
        .expect("finished event");

    assert_eq!(
        finished.encoding,
        fragile_notepad::core::TextEncoding::Windows1252
    );
    assert!(finished.had_errors);
    assert_eq!(
        finished
            .fallback_contents
            .as_ref()
            .expect("fallback decoded contents")
            .text,
        "café"
    );
}

#[test]
fn malformed_utf8_does_not_stream_replacement_preview_per_byte() {
    let path = temp_file_path("malformed-utf8");
    let bytes = vec![0xff; 4096];
    fs::write(&path, &bytes).expect("write temp input");

    let document_id = DocumentId::new(46);
    let generation = DocumentLoadGeneration::next();
    let events = collect_load_events(FileLoadRequest {
        document_id,
        generation,
        path: path.clone(),
        chunk_size: 1,
    });

    let _ = fs::remove_file(&path);

    let chunk_count = events
        .iter()
        .filter(|event| matches!(event, FileLoadEvent::Chunk(_)))
        .count();
    assert!(
        chunk_count <= 1,
        "malformed input should use a single fallback chunk, not {chunk_count} preview chunks"
    );

    let finished = events
        .iter()
        .find_map(|event| match event {
            FileLoadEvent::Finished(Ok(finished)) => Some(finished),
            _ => None,
        })
        .expect("finished event");

    assert_eq!(
        finished.encoding,
        fragile_notepad::core::TextEncoding::Windows1252
    );
    assert!(finished.had_errors);
    assert!(finished.fallback_contents.is_some());
}

#[test]
fn chunked_loader_delivers_finished_after_many_droppable_chunks() {
    let path = temp_file_path("many-chunks");
    let bytes = vec![b'x'; DEFAULT_CHUNK_SIZE * 32];
    fs::write(&path, &bytes).expect("write temp input");

    let document_id = DocumentId::new(44);
    let generation = DocumentLoadGeneration::next();
    let events = collect_load_events(FileLoadRequest {
        document_id,
        generation,
        path: path.clone(),
        chunk_size: 1,
    });

    let _ = fs::remove_file(&path);

    assert!(
        events.iter().any(|event| matches!(
            event,
            FileLoadEvent::Finished(Ok(finished))
                if finished.document_id == document_id
                    && finished.generation == generation
                    && finished.bytes_read == bytes.len() as u64
        )),
        "terminal finished event should not be dropped when the bounded stream is full"
    );
}

#[test]
fn default_chunk_size_remains_large_file_oriented() {
    assert_eq!(DEFAULT_CHUNK_SIZE, 64 * 1024);
}
