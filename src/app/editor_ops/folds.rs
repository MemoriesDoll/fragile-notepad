use crate::core::Document;
use crate::editor::{EditorPosition, EditorSelection, FoldModel, FoldRange};

pub(in crate::app) fn toggle_fold(document: &mut Document, range: FoldRange) {
    let was_collapsed = document.folds.is_collapsed(range);

    if document.folds.toggle(range) {
        let before_selection = document.main_selection();
        if !was_collapsed && document.folds.is_collapsed(range) {
            document.set_main_selection(clamp_to_fold_header(before_selection, range));
        }
        if document.main_selection() != before_selection {
            document.preferred_vertical_column = None;
        }
        document.refresh_view_models();
    }
}

pub(in crate::app) fn set_current_fold_collapsed(document: &mut Document, collapsed: bool) {
    let Some(range) = document
        .folds
        .range_at_or_parent(document.main_selection().cursor.line)
    else {
        return;
    };

    set_fold_collapsed(document, range, collapsed);
}

pub(in crate::app) fn toggle_current_fold(document: &mut Document) {
    let Some(range) = document
        .folds
        .range_at_or_parent(document.main_selection().cursor.line)
    else {
        return;
    };

    toggle_fold(document, range);
}

fn set_fold_collapsed(document: &mut Document, range: FoldRange, collapsed: bool) {
    let was_collapsed = document.folds.is_collapsed(range);

    if document.folds.set_collapsed(range, collapsed) {
        let before_selection = document.main_selection();
        if !was_collapsed && document.folds.is_collapsed(range) {
            document.set_main_selection(clamp_to_fold_header(before_selection, range));
        }
        if document.main_selection() != before_selection {
            document.preferred_vertical_column = None;
        }
        document.refresh_view_models();
    }
}

pub(in crate::app) fn set_all_folds_collapsed(document: &mut Document, collapsed: bool) {
    if document.folds.set_all_collapsed(collapsed) {
        let before_selection = document.main_selection();
        if collapsed {
            document
                .set_main_selection(clamp_to_collapsed_header(before_selection, &document.folds));
        }
        if document.main_selection() != before_selection {
            document.preferred_vertical_column = None;
        }
        document.refresh_view_models();
    }
}

fn clamp_to_fold_header(selection: EditorSelection, range: FoldRange) -> EditorSelection {
    if !range.contains_hidden_line(selection.anchor.line)
        && !range.contains_hidden_line(selection.cursor.line)
    {
        return selection;
    }

    let header = EditorPosition::new(range.start_line, 0);
    EditorSelection::new(header, header)
}

fn clamp_to_collapsed_header(selection: EditorSelection, folds: &FoldModel) -> EditorSelection {
    let Some(range) = folds
        .collapsed_covering(selection.cursor.line)
        .or_else(|| folds.collapsed_covering(selection.anchor.line))
    else {
        return selection;
    };

    let header = EditorPosition::new(range.start_line, 0);
    EditorSelection::new(header, header)
}
