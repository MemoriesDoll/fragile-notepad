use crate::core::Document;
use crate::editor::layout::{byte_column_for, visual_column_for};
use crate::editor::{
    CaretMotion, DelimiterMatch, EditorPosition, EditorSelection, FunctionEntry,
    containing_function, is_vertical_motion, matching_delimiter_near_caret, move_position,
    next_function_after, outline_for_syntax, position_for_byte_offset, previous_function_before,
    word_range_at_position,
};
use unicode_segmentation::UnicodeSegmentation;

pub(in crate::app) fn go_to_matching_delimiter(document: &mut Document) {
    let Some(delimiter_match) = delimiter_match_for_selection(document) else {
        return;
    };
    let text = document.buffer.text();
    let Some(position) = position_for_byte_offset(&text, delimiter_match.matching_delimiter) else {
        return;
    };

    document.set_main_selection(EditorSelection::new(position, position));
    document.preferred_vertical_column = None;
    document.reveal_position(position);
}

pub(in crate::app) fn select_matching_delimiter(document: &mut Document) {
    let Some(matching_position) = select_delimiter_range(document) else {
        return;
    };

    document.reveal_position(matching_position);
}

pub(in crate::app) fn select_delimiter_in_place(document: &mut Document) {
    let _ = select_delimiter_range(document);
}

pub(in crate::app) fn select_word_at(document: &mut Document, position: EditorPosition) {
    document.preferred_vertical_column = None;
    let position = document.buffer.clamp_position(position);

    let Some(range) = word_range_at_position(&document.buffer, position, &document.syntax_token)
    else {
        document.set_main_selection(EditorSelection::new(position, position));
        select_delimiter_in_place(document);
        return;
    };

    document.set_main_selection(EditorSelection::new(range.start, range.end));
}

pub(in crate::app) fn go_to_next_function(
    document: &mut Document,
    outline_entries: Option<&[FunctionEntry]>,
) {
    let fallback;
    let entries = match outline_entries {
        Some(entries) => entries,
        None => {
            fallback = outline_for_syntax(&document.buffer, &document.syntax_token);
            &fallback
        }
    };
    let Some(target) = next_function_after(entries, document.main_selection().cursor)
        .map(|entry| entry.range.start)
    else {
        return;
    };

    document.set_main_selection(EditorSelection::new(target, target));
    document.preferred_vertical_column = None;
    document.reveal_position(target);
}

pub(in crate::app) fn go_to_previous_function(
    document: &mut Document,
    outline_entries: Option<&[FunctionEntry]>,
) {
    let fallback;
    let entries = match outline_entries {
        Some(entries) => entries,
        None => {
            fallback = outline_for_syntax(&document.buffer, &document.syntax_token);
            &fallback
        }
    };
    let Some(target) = previous_function_before(entries, document.main_selection().cursor)
        .map(|entry| entry.range.start)
    else {
        return;
    };

    document.set_main_selection(EditorSelection::new(target, target));
    document.preferred_vertical_column = None;
    document.reveal_position(target);
}

pub(in crate::app) fn select_current_function(
    document: &mut Document,
    outline_entries: Option<&[FunctionEntry]>,
) {
    let fallback;
    let entries = match outline_entries {
        Some(entries) => entries,
        None => {
            fallback = outline_for_syntax(&document.buffer, &document.syntax_token);
            &fallback
        }
    };
    let Some(range) =
        containing_function(entries, document.main_selection().cursor).map(|entry| entry.range)
    else {
        return;
    };

    document.set_main_selection(EditorSelection::new(range.start, range.end));
    document.preferred_vertical_column = None;
    document.reveal_position(range.start);
}

pub(in crate::app) fn select_current_function_body(
    document: &mut Document,
    outline_entries: Option<&[FunctionEntry]>,
) {
    let fallback;
    let entries = match outline_entries {
        Some(entries) => entries,
        None => {
            fallback = outline_for_syntax(&document.buffer, &document.syntax_token);
            &fallback
        }
    };
    let Some(range) = containing_function(entries, document.main_selection().cursor)
        .and_then(|entry| entry.body_range)
    else {
        return;
    };

    document.set_main_selection(EditorSelection::new(range.start, range.end));
    document.preferred_vertical_column = None;
    document.reveal_position(range.start);
}

fn select_delimiter_range(document: &mut Document) -> Option<EditorPosition> {
    let delimiter_match = delimiter_match_for_selection(document)?;

    let start_offset = delimiter_match
        .delimiter
        .min(delimiter_match.matching_delimiter);
    let end_offset = delimiter_match
        .delimiter
        .max(delimiter_match.matching_delimiter)
        .saturating_add(1);
    let text = document.buffer.text();
    let start = position_for_byte_offset(&text, start_offset)?;
    let end = position_for_byte_offset(&text, end_offset)?;
    let matching_position = position_for_byte_offset(&text, delimiter_match.matching_delimiter)?;

    document.set_main_selection(EditorSelection::new(start, end));
    document.preferred_vertical_column = None;
    Some(matching_position)
}

fn delimiter_match_for_selection(document: &Document) -> Option<DelimiterMatch> {
    let caret_offset = document
        .buffer
        .byte_offset(document.main_selection().cursor);

    let text = document.buffer.text();
    matching_delimiter_near_caret(&text, caret_offset)
}

pub(in crate::app) fn move_document_position(
    document: &mut Document,
    position: EditorPosition,
    motion: CaretMotion,
) -> EditorPosition {
    let position = document.buffer.clamp_position(position);
    let row = document.position_visible_row(position).unwrap_or(0);
    if !is_vertical_motion(motion) {
        document.preferred_vertical_column = None;
        document.clear_caret_row_affinity();
        if document.word_wrap()
            && matches!(motion, CaretMotion::LineStart | CaretMotion::LineEnd)
            && let Some(segment) = document.viewport.row_segment(row, &document.buffer)
        {
            let target = EditorPosition::new(
                position.line,
                if motion == CaretMotion::LineStart {
                    segment.start_column
                } else {
                    segment.end_column
                },
            );
            document.set_caret_row_affinity(target, row);
            return target;
        }
        return move_position(&document.buffer, position, motion);
    }

    let tab_width = document.decorations.settings.indent_width;
    let current_text = document.buffer.line(position.line).unwrap_or_default();
    let row_start_column = document
        .viewport
        .row_segment(row, &document.buffer)
        .map_or(0, |segment| segment.start_visual_column);
    let current_column = visual_column_for(&current_text, position.column, tab_width)
        .saturating_sub(row_start_column);
    let preferred_column = *document
        .preferred_vertical_column
        .get_or_insert(current_column);
    let rows = match motion {
        CaretMotion::PageUp | CaretMotion::PageDown => document.viewport_visible_rows.max(1),
        _ => 1,
    };
    let target_row = match motion {
        CaretMotion::Up | CaretMotion::PageUp => row.saturating_sub(rows),
        _ => row
            .saturating_add(rows)
            .min(document.viewport.visible_row_count().saturating_sub(1)),
    };
    let target_line = document
        .viewport
        .visible_row_to_document_line(target_row)
        .unwrap_or(position.line);
    let target_text = document.buffer.line(target_line).unwrap_or_default();
    let column = if let Some(segment) = document.viewport.row_segment(target_row, &document.buffer)
    {
        byte_column_for(
            &target_text,
            segment.start_visual_column.saturating_add(preferred_column),
            tab_width,
        )
        .clamp(segment.start_column, segment.end_column)
    } else {
        byte_column_for(&target_text, preferred_column, tab_width)
    };
    let column = if column == target_text.len() {
        column
    } else {
        target_text
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .take_while(|offset| *offset <= column)
            .last()
            .unwrap_or(0)
    };
    let target = EditorPosition::new(target_line, column);
    document.set_caret_row_affinity(target, target_row);
    target
}
