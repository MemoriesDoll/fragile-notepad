use crate::app::editor_ops::{
    backspace, delete, indent, line_span_text, paste_selection, replace_ranges_for_search,
    replace_selection, selected_text,
};
use crate::core::DocumentId;
use crate::core::document::Document;
use crate::editor::{EditorPosition, EditorSelection, SelectionSet};
use crate::message::ClipboardMode;

fn position(line: usize, column: usize) -> EditorPosition {
    EditorPosition::new(line, column)
}

fn caret(line: usize, column: usize) -> EditorSelection {
    EditorSelection::new(position(line, column), position(line, column))
}

fn selection(
    anchor_line: usize,
    anchor_column: usize,
    cursor_line: usize,
    cursor_column: usize,
) -> EditorSelection {
    EditorSelection::new(
        position(anchor_line, anchor_column),
        position(cursor_line, cursor_column),
    )
}

fn document(text: &str, selection: EditorSelection) -> Document {
    let mut document = Document::untitled(DocumentId::new(1));
    document.buffer = crate::editor::EditorBuffer::from_text(text);
    document.set_main_selection(selection);
    document.refresh_after_text_change();
    document.mark_clean();
    document
}

#[test]
fn multiline_linear_replacement_and_deletion_include_line_endings() {
    let mut document = document("one\ntwo", selection(0, 0, 1, 3));
    assert!(replace_selection(&mut document, "X", false, 4));
    assert_eq!(document.text(), "X");
    assert_eq!(document.selection_set().len(), 1);
    assert!(document.undo());
    assert!(delete(&mut document, 4));
    assert_eq!(document.text(), "");
    assert!(document.undo());
    document.set_main_selection(selection(0, 3, 1, 0));
    assert!(backspace(&mut document, 4));
    assert_eq!(document.text(), "onetwo");
}

#[test]
fn clipboard_preserves_blank_lines_and_original_line_endings() {
    let document = document("one\r\n\r\ntwo\n", selection(0, 0, 3, 0));
    assert_eq!(
        selected_text(&document, 4).as_deref(),
        Some("one\r\n\r\ntwo\n")
    );
    assert_eq!(
        line_span_text(&document, 4).as_deref(),
        Some("one\r\n\r\ntwo\n")
    );
}

#[test]
fn indent_preserves_selected_contents_direction_and_undo() {
    let original = selection(2, 0, 0, 1);
    let mut document = document("one\ntwo\nthree", original);
    assert!(indent(&mut document, 2, "  "));
    assert_eq!(document.text(), "  one\n  two\nthree");
    assert_eq!(document.main_selection(), selection(2, 0, 0, 3));
    assert!(document.undo());
    assert_eq!(document.text(), "one\ntwo\nthree");
    assert_eq!(document.main_selection(), original);
}

#[test]
fn search_batch_is_one_undoable_transaction() {
    let mut document = document("one one one", caret(0, 0));
    let revision = document.revision();
    let ranges = [0, 4, 8]
        .into_iter()
        .map(|column| {
            (
                crate::editor::EditorRange::new(position(0, column), position(0, column + 3)),
                "X".to_owned(),
            )
        })
        .collect();
    assert!(replace_ranges_for_search(&mut document, ranges));
    assert_eq!(document.text(), "X X X");
    assert_eq!(document.selection_set().len(), 1);
    assert_eq!(document.main_selection(), caret(0, 1));
    assert_eq!(document.revision(), revision + 1);
    assert!(document.undo());
    assert_eq!(document.text(), "one one one");
    assert!(!document.can_undo());
    assert!(document.redo());
    assert_eq!(document.selection_set().len(), 1);
    assert_eq!(document.main_selection(), caret(0, 1));
}

#[test]
fn replace_selection_records_single_edit_and_updates_selection() {
    let mut document = document("hello world", selection(0, 6, 0, 11));

    assert!(replace_selection(&mut document, "notepad", false, 4));

    assert_eq!(document.text(), "hello notepad");
    assert_eq!(document.main_selection(), caret(0, 13));
    assert!(document.is_dirty);
    assert!(document.can_undo());
}

#[test]
fn replace_selection_maps_many_replacement_carets_without_full_document_copy() {
    let line_count = 1024;
    let text = (0..line_count).map(|_| "x").collect::<Vec<_>>().join("\n");
    let mut document = document(&text, caret(0, 1));
    document.set_selection_set(SelectionSet::from_ranges(
        (0..line_count).map(|line| caret(line, 1)).collect(),
        line_count - 1,
    ));

    assert!(replace_selection(&mut document, "!", false, 4));

    let expected = (0..line_count).map(|_| "x!").collect::<Vec<_>>().join("\n");
    assert_eq!(document.text(), expected);
    assert_eq!(document.main_selection(), caret(line_count - 1, 2));
}

#[test]
fn backspace_removes_previous_grapheme_for_each_caret() {
    let mut document = document("alpha\nbeta", caret(1, 2));

    assert!(backspace(&mut document, 4));

    assert_eq!(document.text(), "alpha\nbta");
    assert_eq!(document.main_selection(), caret(1, 1));
}

#[test]
fn delete_removes_next_grapheme_for_each_caret() {
    let mut document = document("alpha", caret(0, 1));

    assert!(delete(&mut document, 4));

    assert_eq!(document.text(), "apha");
    assert_eq!(document.main_selection(), caret(0, 1));
}

#[test]
fn rectangular_paste_replaces_projected_lines() {
    let mut document = document("one\ntwo\nthree", caret(0, 1));
    document.set_selection_set(SelectionSet::rectangular(
        position(0, 1),
        position(2, 2),
        1,
        2,
    ));

    assert!(paste_selection(
        &mut document,
        ClipboardMode::Rectangular { line_count: 3 },
        "A\nB\nC",
        4,
    ));

    assert_eq!(document.text(), "oAe\ntBo\ntCree");
}

#[test]
fn rectangular_paste_splits_crlf_clipboard_lines() {
    let mut document = document("one\ntwo\nthree", caret(0, 1));
    document.set_selection_set(SelectionSet::rectangular(
        position(0, 1),
        position(2, 2),
        1,
        2,
    ));

    assert!(paste_selection(
        &mut document,
        ClipboardMode::Rectangular { line_count: 3 },
        "A\r\nB\r\nC",
        4,
    ));

    assert_eq!(document.text(), "oAe\ntBo\ntCree");
}

#[test]
fn rectangular_paste_preserves_trailing_crlf_as_empty_line() {
    let mut document = document("one\ntwo\nthree", caret(0, 1));
    document.set_selection_set(SelectionSet::rectangular(
        position(0, 1),
        position(2, 2),
        1,
        2,
    ));

    assert!(paste_selection(
        &mut document,
        ClipboardMode::Rectangular { line_count: 3 },
        "A\r\nB\r\n",
        4,
    ));

    assert_eq!(document.text(), "oAe\ntBo\ntree");
}

#[test]
fn rectangular_paste_falls_back_to_linear_when_line_counts_do_not_match() {
    let mut document = document("one\ntwo", caret(0, 1));
    document.set_selection_set(SelectionSet::rectangular(
        position(0, 1),
        position(1, 2),
        1,
        2,
    ));

    assert!(paste_selection(
        &mut document,
        ClipboardMode::Rectangular { line_count: 3 },
        "A\nB\nC",
        4,
    ));

    assert_eq!(document.text(), "oA\nB\nCe\ntA\nB\nCo");
}
