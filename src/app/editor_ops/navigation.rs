use crate::core::Document;
use crate::editor::{
    CaretMotion, DelimiterMatch, EditorPosition, EditorSelection, FunctionEntry,
    containing_function, is_vertical_motion, matching_delimiter_near_caret, move_position,
    move_position_with_column, next_function_after, outline_for_syntax, position_for_byte_offset,
    previous_function_before, word_range_at_position,
};

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
    document.reveal_line(position.line);
}

pub(in crate::app) fn select_matching_delimiter(document: &mut Document) {
    let Some(matching_position) = select_delimiter_range(document) else {
        return;
    };

    document.reveal_line(matching_position.line);
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
    document.reveal_line(target.line);
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
    document.reveal_line(target.line);
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
    document.reveal_line(range.start.line);
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
    document.reveal_line(range.start.line);
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
    if !is_vertical_motion(motion) {
        document.preferred_vertical_column = None;
        return move_position(&document.buffer, position, motion);
    }

    let position = document.buffer.clamp_position(position);
    let preferred_column = document
        .preferred_vertical_column
        .get_or_insert(position.column);

    move_position_with_column(&document.buffer, position, motion, *preferred_column)
}
