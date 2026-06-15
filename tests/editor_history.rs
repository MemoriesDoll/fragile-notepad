use fragile_notepad::editor::{
    EditTransaction, EditorBuffer, EditorHistory, EditorPosition, EditorRange, EditorSelection,
    SelectionSet,
};

fn caret(line: usize, column: usize) -> EditorSelection {
    let position = EditorPosition::new(line, column);

    EditorSelection::new(position, position)
}

fn transaction(
    before_range: EditorRange,
    after_range: EditorRange,
    before_text: &str,
    after_text: &str,
    before_selection: EditorSelection,
    after_selection: EditorSelection,
) -> EditTransaction {
    EditTransaction {
        delta: fragile_notepad::editor::EditDelta {
            before_range,
            after_range,
            before_text: before_text.to_owned(),
            after_text: after_text.to_owned(),
        },
        before_selection,
        after_selection,
    }
}

fn insert(
    line: usize,
    column: usize,
    text: &str,
    after_selection: EditorSelection,
) -> EditTransaction {
    let start = EditorPosition::new(line, column);
    transaction(
        EditorRange::new(start, start),
        EditorRange::new(start, after_selection.cursor),
        "",
        text,
        caret(line, column),
        after_selection,
    )
}

#[test]
fn editor_history_record_undo_and_redo_restore_text_and_selection() {
    let mut buffer = EditorBuffer::from_text("hello");
    let mut history = EditorHistory::new(buffer.text());

    history.record(insert(0, 5, "!", caret(0, 6)));

    assert!(history.can_undo());
    assert!(!history.can_redo());

    let undo_selection = history.undo(&mut buffer);

    assert_eq!(buffer.text(), "hello");
    assert_eq!(undo_selection, Some(caret(0, 5)));
    assert!(!history.can_undo());
    assert!(history.can_redo());

    let redo_selection = history.redo(&mut buffer);

    assert_eq!(buffer.text(), "hello!");
    assert_eq!(redo_selection, Some(caret(0, 6)));
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn editor_history_record_with_selection_sets_restores_full_snapshots() {
    let mut buffer = EditorBuffer::from_text("hello");
    let mut history = EditorHistory::new(buffer.text());
    let before = SelectionSet::from_ranges(
        vec![
            EditorSelection::new(EditorPosition::new(0, 0), EditorPosition::new(0, 1)),
            EditorSelection::new(EditorPosition::new(0, 3), EditorPosition::new(0, 5)),
        ],
        1,
    );
    let after =
        SelectionSet::rectangular(EditorPosition::new(0, 1), EditorPosition::new(0, 4), 1, 4);
    let delta = buffer.replace_range(
        EditorRange::new(EditorPosition::new(0, 1), EditorPosition::new(0, 4)),
        "i",
    );

    history.record_with_selection_sets(
        EditTransaction {
            delta,
            before_selection: before.main(),
            after_selection: after.main(),
        },
        before.clone(),
        after.clone(),
    );

    assert_eq!(history.undo_selection_set(&mut buffer), Some(before));
    assert_eq!(buffer.text(), "hello");
    assert_eq!(history.redo_selection_set(&mut buffer), Some(after));
    assert_eq!(buffer.text(), "hio");
}

#[test]
fn editor_history_record_after_undo_clears_redo_stack() {
    let mut buffer = EditorBuffer::from_text("a");
    let mut history = EditorHistory::new(buffer.text());

    history.record(insert(0, 1, "b", caret(0, 2)));
    assert_eq!(history.undo(&mut buffer), Some(caret(0, 1)));

    history.record(insert(0, 1, "c", caret(0, 2)));

    assert!(history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.redo(&mut buffer), None);
}

#[test]
fn editor_history_dirty_state_tracks_clean_text_snapshot() {
    let mut history = EditorHistory::new("saved");

    assert!(!history.is_dirty("saved"));

    history.record(insert(0, 5, " plus edits", caret(0, 16)));
    assert!(history.is_dirty("saved plus edits"));

    history.mark_clean("saved plus edits");

    assert!(!history.is_dirty("saved plus edits"));

    let mut buffer = EditorBuffer::from_text("saved plus edits");
    assert_eq!(history.undo(&mut buffer), Some(caret(0, 5)));
    assert!(history.is_dirty(&buffer.text()));
}

#[test]
fn editor_history_noop_transaction_is_ignored() {
    let mut buffer = EditorBuffer::from_text("same");
    let mut history = EditorHistory::new(buffer.text());

    history.record(transaction(
        EditorRange::new(EditorPosition::new(0, 4), EditorPosition::new(0, 4)),
        EditorRange::new(EditorPosition::new(0, 4), EditorPosition::new(0, 4)),
        "",
        "",
        caret(0, 4),
        caret(0, 4),
    ));

    assert!(!history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.undo(&mut buffer), None);
    assert_eq!(buffer.text(), "same");
}

#[test]
fn editor_history_plain_record_keeps_adjacent_inserts_separate() {
    let mut buffer = EditorBuffer::from_text("ab");
    let mut history = EditorHistory::new("");

    history.record(insert(0, 0, "a", caret(0, 1)));
    history.record(insert(0, 1, "b", caret(0, 2)));

    assert_eq!(history.undo(&mut buffer), Some(caret(0, 1)));
    assert_eq!(buffer.text(), "a");
    assert_eq!(history.undo(&mut buffer), Some(caret(0, 0)));
    assert_eq!(buffer.text(), "");
}

#[test]
fn editor_history_grouped_record_merges_adjacent_single_character_inserts() {
    let mut buffer = EditorBuffer::from_text("ab");
    let mut history = EditorHistory::new("");

    history.record_with_grouping(insert(0, 0, "a", caret(0, 1)));
    history.record_with_grouping(insert(0, 1, "b", caret(0, 2)));

    assert_eq!(history.undo(&mut buffer), Some(caret(0, 0)));
    assert_eq!(buffer.text(), "");
    assert!(!history.can_undo());
    assert!(history.can_redo());

    assert_eq!(history.redo(&mut buffer), Some(caret(0, 2)));
    assert_eq!(buffer.text(), "ab");
}

#[test]
fn editor_history_grouped_record_does_not_merge_newline_inserts() {
    let mut buffer = EditorBuffer::from_text("a\nb");
    let mut history = EditorHistory::new("");

    history.record_with_grouping(insert(0, 0, "a", caret(0, 1)));
    history.record_with_grouping(insert(0, 1, "\n", caret(1, 0)));
    history.record_with_grouping(insert(1, 0, "b", caret(1, 1)));

    assert_eq!(history.undo(&mut buffer), Some(caret(1, 0)));
    assert_eq!(buffer.text(), "a\n");
    assert_eq!(history.undo(&mut buffer), Some(caret(0, 1)));
    assert_eq!(buffer.text(), "a");
}

#[test]
fn editor_history_selection_replacement_restores_selection_and_dirty_snapshot() {
    let mut buffer = EditorBuffer::from_text("hello");
    let mut history = EditorHistory::new("hello");
    let selected = EditorSelection::new(EditorPosition::new(0, 1), EditorPosition::new(0, 4));
    let delta = buffer.replace_range(selected.range(), "i");

    history.record(EditTransaction {
        delta,
        before_selection: selected,
        after_selection: caret(0, 2),
    });
    assert!(history.is_dirty("hio"));

    assert_eq!(history.undo(&mut buffer), Some(selected));
    assert_eq!(buffer.text(), "hello");
    assert!(!history.is_dirty(&buffer.text()));

    assert_eq!(history.redo(&mut buffer), Some(caret(0, 2)));
    assert_eq!(buffer.text(), "hio");
    assert!(history.is_dirty(&buffer.text()));

    history.mark_clean(&buffer.text());
    assert!(!history.is_dirty(&buffer.text()));
}

#[test]
fn editor_history_new_grouped_edit_after_undo_clears_redo_stack() {
    let mut buffer = EditorBuffer::from_text("a");
    let mut history = EditorHistory::new("");

    history.record_with_grouping(insert(0, 0, "a", caret(0, 1)));
    assert_eq!(history.undo(&mut buffer), Some(caret(0, 0)));

    history.record_with_grouping(insert(0, 0, "b", caret(0, 1)));

    assert!(!history.can_redo());
    assert_eq!(history.redo(&mut buffer), None);
}
