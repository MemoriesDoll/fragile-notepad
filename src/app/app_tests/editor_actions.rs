use super::test_support::*;

#[test]
fn editor_tab_action_inserts_configured_indentation() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.settings.indentation = crate::core::IndentationMode::Spaces(2);
    let _ = app.update(Message::EditorAction(document_id, EditorAction::Indent));

    assert_eq!(
        app.workspace.active_document().expect("active").text(),
        "  "
    );
    assert!(app.workspace.active_document().expect("active").is_dirty);
}

#[test]
fn editor_unindent_caret_removes_configured_width_leading_spaces_from_current_line() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.settings.indentation = IndentationMode::Spaces(2);
    set_active_document_text(
        &mut app,
        "  alpha\n    beta",
        EditorSelection::new(EditorPosition::new(1, 4), EditorPosition::new(1, 4)),
    );

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Unindent));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "  alpha\n  beta");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(1, 2), EditorPosition::new(1, 2))
    );
    assert!(document.is_dirty);
}

#[test]
fn editor_unindent_selection_excludes_final_line_when_selection_ends_at_column_zero() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.settings.indentation = IndentationMode::Spaces(2);
    set_active_document_text(
        &mut app,
        "  one\n  two\n  three",
        EditorSelection::new(EditorPosition::new(0, 2), EditorPosition::new(2, 0)),
    );

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Unindent));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "one\ntwo\n  three");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(2, 0))
    );
}

#[test]
fn editor_unindent_full_selection_includes_final_touched_line() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.settings.indentation = IndentationMode::Spaces(2);
    set_active_document_text(
        &mut app,
        "  one\n  two\n  three",
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(2, 7)),
    );

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Unindent));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "one\ntwo\nthree");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(2, 5))
    );
}

#[test]
fn editor_unindent_removes_leading_tab() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.settings.indentation = IndentationMode::Spaces(4);
    set_active_document_text(
        &mut app,
        "\talpha",
        EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1)),
    );

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Unindent));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "alpha");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0))
    );
}

#[test]
fn editor_unindent_reversed_selection_preserves_anchor_and_cursor_direction() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.settings.indentation = IndentationMode::Spaces(4);
    set_active_document_text(
        &mut app,
        "    one\n    two",
        EditorSelection::new(EditorPosition::new(1, 7), EditorPosition::new(0, 4)),
    );

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Unindent));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "one\ntwo");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(1, 3), EditorPosition::new(0, 0))
    );
}

#[test]
fn editor_unindent_is_one_undoable_edit_restoring_text_and_selection() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let original_text = "  one\n  two\nplain";
    let original_selection =
        EditorSelection::new(EditorPosition::new(0, 2), EditorPosition::new(1, 5));

    app.settings.indentation = IndentationMode::Spaces(2);
    set_active_document_text(&mut app, original_text, original_selection);

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Unindent));

    assert_eq!(
        app.workspace.active_document().expect("active").text(),
        "one\ntwo\nplain"
    );

    let _ = app.update(Message::Undo);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), original_text);
    assert_eq!(document.selection, original_selection);
    assert!(!document.is_dirty);
    assert!(!document.can_undo());
}

#[test]
fn applying_indentation_settings_updates_document_decoration_width() {
    let (mut app, _) = App::new();

    let _ = app.update(Message::DraftIndentationSelected(
        crate::core::IndentationMode::Spaces(2),
    ));
    let _ = app.update(Message::ApplySettings);

    assert_eq!(
        app.workspace
            .active_document()
            .expect("active")
            .decorations
            .settings
            .indent_width,
        2
    );
}

#[test]
fn backspace_deletes_previous_utf8_character() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("a\u{597d}b");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 4), EditorPosition::new(0, 4));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Backspace));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "ab");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1))
    );
}

#[test]
fn delete_removes_next_utf8_character() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("a\u{597d}b");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Delete));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "ab");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1))
    );
}

#[test]
fn backspace_deletes_previous_grapheme_cluster() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;
    let text = "a\u{0065}\u{0301}b";
    let cursor = "a\u{0065}\u{0301}".len();

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text(text);
        document.selection = EditorSelection::new(
            EditorPosition::new(0, cursor),
            EditorPosition::new(0, cursor),
        );
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Backspace));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "ab");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1))
    );
}

#[test]
fn delete_removes_next_grapheme_cluster() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("a\u{0065}\u{0301}b");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Delete));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "ab");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 1))
    );
}

#[test]
fn new_file_applies_current_decoration_settings_immediately() {
    let (mut app, _) = App::new();

    app.settings.decorations.show_line_numbers = false;
    let _ = app.update(Message::NewFile);

    let document = app.workspace.active_document().expect("active document");
    assert!(
        document
            .decorations
            .line_decorations
            .iter()
            .all(|line| line.line_number.is_none())
    );
}

#[test]
fn copy_action_does_not_mutate_active_document() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("copy me");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 4));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Copy));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "copy me");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 4))
    );
    assert!(!document.is_dirty);
}

#[test]
fn edit_menu_clipboard_messages_route_to_active_editor() {
    let (mut app, _) = App::new();

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = EditorBuffer::from_text("cut me");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 3));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    app.active_menu = Some(Menu::Edit);
    app.active_menu_path = vec![String::from("clipboard")];
    let _ = app.update(Message::Cut);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), " me");
    assert!(document.is_dirty);
    assert_eq!(app.active_menu, None);
    assert!(app.active_menu_path.is_empty());

    let _ = app.update(Message::Undo);
    let _ = app.update(Message::Copy);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "cut me");
    assert!(!document.is_dirty);

    let task = app.update(Message::Paste);
    assert_eq!(task.units(), 1);
}

#[test]
fn edit_delete_menu_message_deletes_selection() {
    let (mut app, _) = App::new();

    set_active_document_text(
        &mut app,
        "delete me",
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 7)),
    );

    let _ = app.update(Message::Delete);

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "me");
    assert!(document.is_dirty);
}

#[test]
fn cut_action_deletes_selection_as_one_undoable_edit() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("cut me");
        document.selection =
            EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 3));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(document_id, EditorAction::Cut));

    assert_eq!(
        app.workspace.active_document().expect("active").text(),
        " me"
    );
    assert!(app.workspace.active_document().expect("active").is_dirty);

    let _ = app.update(Message::Undo);

    assert_eq!(
        app.workspace.active_document().expect("active").text(),
        "cut me"
    );
    assert_eq!(
        app.workspace.active_document().expect("active").selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 3))
    );
    assert!(!app.workspace.active_document().expect("active").is_dirty);
}

#[test]
fn clipboard_read_replaces_selection_and_refreshes_find_matches() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    app.find.set_query("paste");
    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("replace");
        document.set_main_selection(EditorSelection::new(
            EditorPosition::new(0, 0),
            EditorPosition::new(0, 7),
        ));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let request = PasteRequest {
        document_id,
        selection: EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 7)),
        selection_set: app
            .workspace
            .active_document()
            .expect("active document")
            .selection_set()
            .clone(),
        clipboard_mode: ClipboardMode::Linear,
    };
    let _ = app.update(Message::ClipboardRead(
        request,
        Ok(Arc::new("paste".to_owned())),
    ));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "paste");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 5), EditorPosition::new(0, 5))
    );
    assert!(document.is_dirty);
    assert_eq!(app.find.matches.len(), 1);
}

#[test]
fn clipboard_read_replaces_original_paste_selection_after_caret_moves() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer = crate::editor::EditorBuffer::from_text("abc def");
        document.set_main_selection(EditorSelection::new(
            EditorPosition::new(0, 4),
            EditorPosition::new(0, 7),
        ));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let request = PasteRequest {
        document_id,
        selection: EditorSelection::new(EditorPosition::new(0, 4), EditorPosition::new(0, 7)),
        selection_set: app
            .workspace
            .active_document()
            .expect("active document")
            .selection_set()
            .clone(),
        clipboard_mode: ClipboardMode::Linear,
    };
    let _ = app.update(Message::EditorAction(
        document_id,
        EditorAction::PlaceCaret(EditorPosition::new(0, 0)),
    ));
    let _ = app.update(Message::ClipboardRead(
        request,
        Ok(Arc::new("XYZ".to_owned())),
    ));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(document.text(), "abc XYZ");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 7), EditorPosition::new(0, 7))
    );
}

#[test]
fn collapsing_fold_moves_hidden_selection_to_fold_header() {
    let (mut app, _) = App::new();
    let document_id = app.workspace.active_document_id;

    {
        let document = app
            .workspace
            .active_document_mut()
            .expect("active document");
        document.buffer =
            crate::editor::EditorBuffer::from_text("fn main() {\n    let value = 1;\n}\n");
        document.selection =
            EditorSelection::new(EditorPosition::new(1, 8), EditorPosition::new(1, 8));
        document.refresh_after_text_change();
        document.mark_clean();
    }

    let _ = app.update(Message::EditorAction(
        document_id,
        EditorAction::ToggleFold(crate::editor::FoldRange::new(0, 2)),
    ));

    let document = app.workspace.active_document().expect("active document");
    assert_eq!(
        document.selection,
        EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 0))
    );
    assert_eq!(document.viewport.document_line_to_visible_row(1), None);
}
