use super::test_support::*;

#[test]
fn manual_non_plain_language_selection_survives_save_as() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    let _ = app.update(Message::LanguageSelected("rs".to_owned()));
    let request = SaveRequest {
        document_id,
        snapshot: Arc::new(Vec::new()),
    };
    let _ = app.update(Message::FileSaved(request, Ok(PathBuf::from("README"))));

    let document = app
        .workspace
        .active_document()
        .expect("workspace should have an active document");

    assert_eq!(document.syntax_token, "rs");
}

#[test]
fn plain_text_language_selection_returns_to_auto_detection_on_save_as() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    let _ = app.update(Message::LanguageSelected("txt".to_owned()));
    let request = SaveRequest {
        document_id,
        snapshot: Arc::new(Vec::new()),
    };
    let _ = app.update(Message::FileSaved(request, Ok(PathBuf::from("main.c"))));

    let document = app
        .workspace
        .active_document()
        .expect("workspace should have an active document");

    assert_eq!(document.syntax_token, "c");
}

#[test]
fn dirty_close_discard_closes_document() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.workspace
        .active_document_mut()
        .expect("active document")
        .mark_dirty();

    let _ = app.update(Message::DirtyCloseResolved(
        document_id,
        DirtyCloseDecision::Discard,
    ));

    assert_ne!(app.workspace.active_document_id, document_id);
    assert!(app.workspace.document(document_id).is_none());
}

#[test]
fn dirty_close_cancel_keeps_document_open() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.workspace
        .active_document_mut()
        .expect("active document")
        .mark_dirty();

    let _ = app.update(Message::DirtyCloseResolved(
        document_id,
        DirtyCloseDecision::Cancel,
    ));

    assert!(app.workspace.document(document_id).is_some());
    assert_eq!(app.workspace.active_document_id, document_id);
    assert_eq!(app.pending_dirty_close, None);
}

#[test]
fn closing_dirty_document_opens_in_app_prompt() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.workspace
        .active_document_mut()
        .expect("active document")
        .mark_dirty();

    let _ = app.update(Message::CloseFile);

    assert_eq!(app.pending_dirty_close, Some(document_id));
    assert!(app.workspace.document(document_id).is_some());
}

#[test]
fn dirty_close_save_closes_after_successful_save() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let path = PathBuf::from("note.txt");

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.set_path(path.clone());
        document.mark_dirty();
    }

    let _ = app.update(Message::DirtyCloseResolved(
        document_id,
        DirtyCloseDecision::Save,
    ));

    let request = app
        .pending_save
        .clone()
        .expect("dirty close save should start a save");
    let _ = app.update(Message::FileSaved(request, Ok(path)));

    assert!(app.workspace.document(document_id).is_none());
}

#[test]
fn dirty_close_save_encoding_failure_keeps_document_open_and_clears_pending_close() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("\u{20ac}");
        document.set_encoding(crate::core::TextEncoding::Iso8859_1);
    }

    let _ = app.update(Message::DirtyCloseResolved(
        document_id,
        DirtyCloseDecision::Save,
    ));

    assert!(app.pending_save.is_none());
    assert_eq!(app.pending_close_after_save, None);
    assert!(app.pending_close_documents.is_empty());
    assert!(app.workspace.document(document_id).is_some());
    assert_eq!(
        app.file_status.as_deref(),
        Some("Save failed: encoding error")
    );
}

#[test]
fn file_open_error_sets_visible_status() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::FileOpened(Err(crate::message::FileError::Io(
        std::io::ErrorKind::PermissionDenied,
    ))));

    assert_eq!(app.file_status.as_deref(), Some("Open failed: I/O error"));
    assert!(!app.is_loading);
}

#[test]
fn save_all_queues_dirty_documents_and_skips_clean_documents() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();
    let third = app.workspace.create_untitled();

    {
        let document = app.workspace.document_mut(first).expect("first document");
        document.set_path("first.txt");
        document.mark_dirty();
    }
    {
        let document = app.workspace.document_mut(second).expect("second document");
        document.set_path("second.txt");
    }
    {
        let document = app.workspace.document_mut(third).expect("third document");
        document.set_path("third.txt");
        document.mark_dirty();
    }

    let _ = app.update(Message::SaveAllFiles);

    assert_eq!(
        app.pending_save.as_ref().map(|request| request.document_id),
        Some(first)
    );
    assert_eq!(pending_save_all_ids(&app), vec![first, third]);

    let first_request = app.pending_save.clone().expect("first save request");
    let _ = app.update(Message::FileSaved(
        first_request,
        Ok(PathBuf::from("first.txt")),
    ));

    assert_eq!(
        app.pending_save.as_ref().map(|request| request.document_id),
        Some(third)
    );
    assert_eq!(pending_save_all_ids(&app), vec![third]);

    let third_request = app.pending_save.clone().expect("third save request");
    let _ = app.update(Message::FileSaved(
        third_request,
        Ok(PathBuf::from("third.txt")),
    ));

    assert!(app.pending_save.is_none());
    assert!(app.pending_save_all.is_empty());
    assert!(!app.workspace.document(first).expect("first").is_dirty);
    assert!(!app.workspace.document(second).expect("second").is_dirty);
    assert!(!app.workspace.document(third).expect("third").is_dirty);
}

#[test]
fn save_all_stops_after_failed_save() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();

    {
        let document = app.workspace.document_mut(first).expect("first document");
        document.set_path("first.txt");
        document.mark_dirty();
    }
    {
        let document = app.workspace.document_mut(second).expect("second document");
        document.set_path("second.txt");
        document.mark_dirty();
    }

    let _ = app.update(Message::SaveAllFiles);

    let first_request = app.pending_save.clone().expect("first save request");
    let _ = app.update(Message::FileSaved(
        first_request,
        Err(crate::message::FileError::Io(std::io::ErrorKind::Other)),
    ));

    assert!(app.pending_save.is_none());
    assert!(app.pending_save_all.is_empty());
    assert!(app.workspace.document(first).expect("first").is_dirty);
    assert!(app.workspace.document(second).expect("second").is_dirty);
}

#[test]
fn close_all_but_active_keeps_active_document_and_closes_clean_neighbors() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();
    let third = app.workspace.create_untitled();

    app.workspace.select(second);

    let _ = app.update(Message::CloseAllButActiveFile);

    assert!(app.workspace.document(first).is_none());
    assert!(app.workspace.document(third).is_none());
    assert!(app.workspace.document(second).is_some());
    assert_eq!(app.workspace.active_document_id, second);
}

#[test]
fn close_all_but_pinned_keeps_pinned_documents_open() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();
    let third = app.workspace.create_untitled();

    assert!(app.workspace.toggle_pin(first));
    assert!(app.workspace.toggle_pin(third));

    let _ = app.update(Message::CloseAllButPinnedFiles);

    assert!(app.workspace.document(first).is_some());
    assert!(app.workspace.document(second).is_none());
    assert!(app.workspace.document(third).is_some());
}

#[test]
fn close_all_to_left_prompts_for_first_dirty_left_document() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();
    let third = app.workspace.create_untitled();

    app.workspace
        .document_mut(first)
        .expect("first")
        .mark_dirty();
    app.workspace.select(third);

    let _ = app.update(Message::CloseAllToLeft);

    assert_eq!(app.pending_dirty_close, Some(first));
    assert_eq!(
        app.pending_close_documents
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![second]
    );
    assert!(app.workspace.document(first).is_some());
    assert!(app.workspace.document(second).is_some());
    assert!(app.workspace.document(third).is_some());
}

#[test]
fn dirty_close_discard_continues_pending_close_queue() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();
    let third = app.workspace.create_untitled();

    app.workspace
        .document_mut(first)
        .expect("first")
        .mark_dirty();
    app.workspace.select(third);

    let _ = app.update(Message::CloseAllToLeft);
    let _ = app.update(Message::DirtyCloseResolved(
        first,
        DirtyCloseDecision::Discard,
    ));

    assert!(app.workspace.document(first).is_none());
    assert!(app.workspace.document(second).is_none());
    assert!(app.workspace.document(third).is_some());
    assert!(app.pending_close_documents.is_empty());
}

#[test]
fn close_all_unchanged_keeps_dirty_documents_open() {
    let (mut app, _) = App::new();
    let first = app.workspace.active_document_id;
    let second = app.workspace.create_untitled();
    let third = app.workspace.create_untitled();

    app.workspace
        .document_mut(second)
        .expect("second")
        .mark_dirty();

    let _ = app.update(Message::CloseAllUnchanged);

    assert!(app.workspace.document(first).is_none());
    assert!(app.workspace.document(third).is_none());
    assert!(app.workspace.document(second).is_some());
}
