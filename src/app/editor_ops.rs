use crate::editor::layout::visual_column_for;
use crate::editor::{
    CaretMotion, EditTransaction, EditorBuffer, EditorPosition, EditorRange, EditorSelection,
    ProjectedSelectionLine, SelectionRange, SelectionSet, line_end, next_grapheme_offset,
    position_for_byte_offset, previous_grapheme_offset,
};
use crate::message::ClipboardMode;

mod folds;
mod navigation;

pub(super) use folds::{
    set_all_folds_collapsed, set_current_fold_collapsed, toggle_current_fold, toggle_fold,
};
pub(super) use navigation::{
    go_to_matching_delimiter, go_to_next_function, go_to_previous_function, move_document_position,
    select_current_function, select_current_function_body, select_delimiter_in_place,
    select_matching_delimiter, select_word_at,
};

#[derive(Debug, Clone)]
struct ConcreteReplacement {
    range: EditorRange,
    replacement: String,
    source_index: usize,
    main_preferred: bool,
}

pub(super) fn selected_text(document: &crate::core::Document, tab_width: usize) -> Option<String> {
    let newline = document_line_ending(document);
    let projected = concrete_selected_lines(document, document.selection_set(), tab_width);

    if projected.is_empty() || projected.iter().all(|line| line.range().is_empty()) {
        return None;
    }

    if document
        .selection_set()
        .ranges()
        .iter()
        .any(|selection| selection.is_rectangular())
    {
        let mut lines = Vec::new();
        for line in projected {
            let range = document.buffer.clamp_range(line.range());
            lines.push(document.buffer.slice_text(range));
        }

        return Some(lines.join(&newline));
    }

    let mut ranges =
        concrete_ranges_for_selection_set(document.selection_set(), &document.buffer, tab_width);
    ranges.sort_by_key(|range| {
        let range = range.normalized();

        (
            document.buffer.byte_offset(range.start),
            document.buffer.byte_offset(range.end),
        )
    });

    let mut chunks = Vec::new();
    for range in ranges {
        let range = document.buffer.clamp_range(range);
        if range.is_empty() {
            continue;
        }
        chunks.push(document.buffer.slice_text(range));
    }

    (!chunks.is_empty()).then(|| chunks.join(&newline))
}

pub(super) fn line_span_text(document: &crate::core::Document, tab_width: usize) -> Option<String> {
    let newline = document_line_ending(document);
    let mut ranges = Vec::new();

    for selection in document.selection_set().ranges() {
        for line in selection.projected_lines(&document.buffer, tab_width) {
            if let Some(range) = selected_touched_line_range(&document.buffer, line.selection()) {
                ranges.push(range);
            }
        }
    }

    ranges.sort_by_key(|range| document.buffer.byte_offset(range.start));
    ranges.dedup();

    let mut chunks = Vec::new();
    for range in ranges {
        chunks.push(document.buffer.slice_text(range));
    }

    (!chunks.is_empty()).then(|| chunks.join(&newline))
}

pub(super) fn replace_selection(
    document: &mut crate::core::Document,
    replacement: &str,
    allow_grouping: bool,
    tab_width: usize,
) -> bool {
    let before_selection_set = document.selection_set().clone();
    let replacements = replacement_edits_for_selection_set(
        &document.buffer,
        &before_selection_set,
        tab_width,
        |_| replacement.to_owned(),
    );

    apply_concrete_replacements(document, before_selection_set, replacements, allow_grouping)
}

fn replace_document_range(
    document: &mut crate::core::Document,
    range: EditorRange,
    replacement: &str,
    after_selection: EditorSelection,
) -> bool {
    let before_selection_set = document.selection_set().clone();
    let range = document.buffer.clamp_range(range);
    let first_changed_line = range.start.line;
    let delta = document.buffer.replace_range(range, replacement);

    if let Some(line_ending) = crate::core::document::detect_line_ending(replacement) {
        document.line_ending = Some(line_ending);
    }

    document.set_main_selection(after_selection);
    document.preferred_vertical_column = None;

    if delta.before_text == delta.after_text {
        return false;
    }

    let transaction = EditTransaction {
        delta,
        before_selection: before_selection_set.main(),
        after_selection: document.main_selection(),
    };

    document.history.record_with_selection_sets(
        transaction,
        before_selection_set,
        document.selection_set().clone(),
    );
    document.refresh_text_from(first_changed_line);
    true
}

pub(super) fn duplicate_line(document: &mut crate::core::Document) -> bool {
    let before_selection_set = document.selection_set().clone();
    let before_selection = before_selection_set.main();
    let Some(range) = selected_touched_line_range(&document.buffer, before_selection) else {
        return false;
    };

    let insert_position = range.end;
    let start_offset = document.buffer.byte_offset(range.start);
    let end_offset = document.buffer.byte_offset(range.end);
    let text = document.buffer.text();
    let Some(copied_text) = text.get(start_offset..end_offset) else {
        return false;
    };

    let line_ending = document
        .line_ending
        .map(|ending| ending.as_str())
        .unwrap_or("\n");
    let needs_leading_line_ending =
        end_offset == text.len() && !copied_text.ends_with(['\r', '\n']);
    let mut insertion = String::with_capacity(copied_text.len() + line_ending.len());

    if needs_leading_line_ending {
        insertion.push_str(line_ending);
    }
    insertion.push_str(copied_text);

    let insert_range = EditorRange::new(insert_position, insert_position);
    let delta = document.buffer.replace_range(insert_range, &insertion);

    if let Some(line_ending) = crate::core::document::detect_line_ending(&insertion) {
        document.line_ending = Some(line_ending);
    }

    let duplicate_start_offset = end_offset
        + needs_leading_line_ending
            .then_some(line_ending.len())
            .unwrap_or(0);
    let duplicate_end_offset = end_offset + insertion.len();
    let text = document.buffer.text();
    let Some(duplicate_start) = position_for_byte_offset(&text, duplicate_start_offset) else {
        return false;
    };
    let Some(duplicate_end) = position_for_byte_offset(&text, duplicate_end_offset) else {
        return false;
    };

    document.set_main_selection(EditorSelection::new(duplicate_start, duplicate_end));
    document.preferred_vertical_column = None;

    if delta.before_text == delta.after_text {
        return false;
    }

    document.history.record(EditTransaction {
        delta,
        before_selection,
        after_selection: document.main_selection(),
    });
    document.refresh_text_from(insert_position.line);
    true
}

pub(super) fn delete_line(document: &mut crate::core::Document) -> bool {
    let Some(range) = selected_touched_line_range(&document.buffer, document.main_selection())
    else {
        return false;
    };
    let after_selection = EditorSelection::new(range.start, range.start);

    replace_document_range(document, range, "", after_selection)
}

pub(super) fn replace_selection_for_search(
    document: &mut crate::core::Document,
    replacement: &str,
) -> bool {
    replace_selection(
        document,
        replacement,
        false,
        document.decorations.settings.indent_width,
    )
}

pub(super) fn backspace(document: &mut crate::core::Document, tab_width: usize) -> bool {
    if !selection_set_is_all_carets(document.selection_set(), &document.buffer, tab_width) {
        return replace_selection(document, "", false, tab_width);
    }

    let before_selection_set = document.selection_set().clone();
    let mut replacements = Vec::new();
    let text = document.buffer.text();

    for (source_index, line) in concrete_selected_lines(document, &before_selection_set, tab_width)
        .into_iter()
        .enumerate()
    {
        let cursor = line.start;
        let offset = document.buffer.byte_offset(cursor);
        let Some(start_offset) = previous_grapheme_offset(&text, offset) else {
            continue;
        };
        let Some(start) = position_for_byte_offset(&text, start_offset) else {
            continue;
        };

        replacements.push(ConcreteReplacement {
            range: EditorRange::new(start, cursor),
            replacement: String::new(),
            source_index,
            main_preferred: cursor == before_selection_set.main().cursor,
        });
    }

    apply_concrete_replacements(document, before_selection_set, replacements, false)
}

pub(super) fn delete(document: &mut crate::core::Document, tab_width: usize) -> bool {
    if !selection_set_is_all_carets(document.selection_set(), &document.buffer, tab_width) {
        return replace_selection(document, "", false, tab_width);
    }

    let before_selection_set = document.selection_set().clone();
    let mut replacements = Vec::new();
    let text = document.buffer.text();

    for (source_index, line) in concrete_selected_lines(document, &before_selection_set, tab_width)
        .into_iter()
        .enumerate()
    {
        let cursor = line.start;
        let offset = document.buffer.byte_offset(cursor);
        let Some(end_offset) = next_grapheme_offset(&text, offset) else {
            continue;
        };
        let Some(end) = position_for_byte_offset(&text, end_offset) else {
            continue;
        };

        replacements.push(ConcreteReplacement {
            range: EditorRange::new(cursor, end),
            replacement: String::new(),
            source_index,
            main_preferred: cursor == before_selection_set.main().cursor,
        });
    }

    apply_concrete_replacements(document, before_selection_set, replacements, false)
}

pub(super) fn unindent(document: &mut crate::core::Document, indentation_width: usize) -> bool {
    let before_selection_set = document.selection_set().clone();
    let before_selection = before_selection_set.main();
    let Some((first_line, last_line)) = selected_line_span(&document.buffer, before_selection)
    else {
        return false;
    };

    let removals = unindent_removals(&document.buffer, first_line, last_line, indentation_width);
    if removals.iter().all(|removal| *removal == 0) {
        return false;
    }

    let before_range = EditorRange::new(
        EditorPosition::new(first_line, 0),
        line_end(&document.buffer, last_line),
    );
    let start_offset = document.buffer.byte_offset(before_range.start);
    let end_offset = document.buffer.byte_offset(before_range.end);
    let before_text = document.buffer.text();
    let mut source_offset = start_offset;
    let mut replacement = String::with_capacity(end_offset.saturating_sub(start_offset));

    for (index, line) in (first_line..=last_line).enumerate() {
        let line_start = document.buffer.byte_offset(EditorPosition::new(line, 0));
        let line_end = document
            .buffer
            .byte_offset(line_end(&document.buffer, line));
        let removal = removals[index];

        replacement.push_str(&before_text[source_offset..line_start]);
        replacement.push_str(&before_text[line_start + removal..line_end]);
        source_offset = line_end;
    }

    replacement.push_str(&before_text[source_offset..end_offset]);

    let delta = document.buffer.replace_range(before_range, &replacement);
    document.set_main_selection(EditorSelection::new(
        adjust_unindented_position(before_selection.anchor, first_line, &removals),
        adjust_unindented_position(before_selection.cursor, first_line, &removals),
    ));
    document.preferred_vertical_column = None;

    let transaction = EditTransaction {
        delta,
        before_selection,
        after_selection: document.main_selection(),
    };
    document.history.record_with_selection_sets(
        transaction,
        before_selection_set,
        document.selection_set().clone(),
    );
    document.refresh_text_from(first_line);

    true
}

fn selected_line_span(buffer: &EditorBuffer, selection: EditorSelection) -> Option<(usize, usize)> {
    if buffer.line_count() == 0 {
        return None;
    }

    if selection.is_caret() {
        let line = buffer.clamp_position(selection.cursor).line;
        return Some((line, line));
    }

    let range = buffer.clamp_range(selection.range());
    let first_line = range.start.line;
    let last_line = if range.end.column == 0 {
        range.end.line.saturating_sub(1)
    } else {
        range.end.line
    };

    (first_line <= last_line).then_some((first_line, last_line))
}

fn selected_touched_line_range(
    buffer: &EditorBuffer,
    selection: EditorSelection,
) -> Option<EditorRange> {
    if buffer.line_count() == 0 {
        return None;
    }

    let range = buffer.clamp_range(selection.range());
    let (first_line, last_line) = if selection.is_caret() {
        (range.start.line, range.start.line)
    } else if range.end.column == 0 {
        (range.start.line, range.end.line.saturating_sub(1))
    } else {
        (range.start.line, range.end.line)
    };
    if first_line > last_line {
        return None;
    }

    let end = if last_line + 1 < buffer.line_count() {
        EditorPosition::new(last_line + 1, 0)
    } else {
        line_end(buffer, last_line)
    };

    Some(EditorRange::new(EditorPosition::new(first_line, 0), end))
}

fn unindent_removals(
    buffer: &EditorBuffer,
    first_line: usize,
    last_line: usize,
    indentation_width: usize,
) -> Vec<usize> {
    let indentation_width = indentation_width.max(1);

    (first_line..=last_line)
        .map(|line| {
            let text = buffer.line(line).unwrap_or_default();

            if text.starts_with('\t') {
                1
            } else {
                text.as_bytes()
                    .iter()
                    .take(indentation_width)
                    .take_while(|byte| **byte == b' ')
                    .count()
            }
        })
        .collect()
}

fn adjust_unindented_position(
    position: EditorPosition,
    first_line: usize,
    removals: &[usize],
) -> EditorPosition {
    let Some(removal) = position
        .line
        .checked_sub(first_line)
        .and_then(|index| removals.get(index))
    else {
        return position;
    };

    EditorPosition::new(position.line, position.column.saturating_sub(*removal))
}

fn document_line_ending(document: &crate::core::Document) -> String {
    document
        .line_ending
        .map(|ending| ending.as_str())
        .unwrap_or("\n")
        .to_owned()
}

fn concrete_selected_lines(
    document: &crate::core::Document,
    selection_set: &SelectionSet,
    tab_width: usize,
) -> Vec<ProjectedSelectionLine> {
    let mut lines = selection_set.projected_lines(&document.buffer, tab_width);
    lines.sort_by_key(|line| {
        (
            document.buffer.byte_offset(line.start),
            document.buffer.byte_offset(line.end),
        )
    });
    lines
}

fn concrete_ranges_for_selection_set(
    selection_set: &SelectionSet,
    buffer: &EditorBuffer,
    tab_width: usize,
) -> Vec<EditorRange> {
    selection_set
        .projected_lines(buffer, tab_width)
        .into_iter()
        .map(|line| buffer.clamp_range(line.range()))
        .collect()
}

pub(super) fn selection_set_is_all_carets(
    selection_set: &SelectionSet,
    buffer: &EditorBuffer,
    tab_width: usize,
) -> bool {
    let lines = selection_set.projected_lines(buffer, tab_width);

    !lines.is_empty()
        && lines
            .iter()
            .all(|line| buffer.clamp_range(line.range()).is_empty())
}

pub(super) fn add_adjacent_caret(document: &mut crate::core::Document, motion: CaretMotion) {
    let before = document.selection_set().clone();
    let main = before.main_range();
    let target = move_document_position(document, main.cursor, motion);
    let target = document.buffer.clamp_position(target);

    if target == main.cursor {
        return;
    }

    let mut ranges = before.ranges().to_vec();
    ranges.push(SelectionRange::new(target, target));
    document.set_selection_set(SelectionSet::from_selection_ranges(
        ranges,
        before.main_index(),
    ));
    document.preferred_vertical_column = None;
}

pub(super) fn split_selection_into_lines(document: &mut crate::core::Document, tab_width: usize) {
    let projected = document
        .selection_set()
        .projected_lines(&document.buffer, tab_width);
    let mut ranges = Vec::new();
    let mut main = 0;

    for line in projected {
        if line.selection() == document.selection_set().main() {
            main = ranges.len();
        }
        ranges.push(line.selection());
    }

    if !ranges.is_empty() {
        document.set_selection_set(SelectionSet::from_ranges(ranges, main));
        document.preferred_vertical_column = None;
    }
}

pub(super) fn convert_selection_to_rectangle(
    document: &mut crate::core::Document,
    tab_width: usize,
) {
    let selection = document.selection_set().main_range();
    let range = document.buffer.clamp_range(selection.range());

    if range.is_empty() {
        return;
    }

    let start_text = document.buffer.line(range.start.line).unwrap_or_default();
    let end_text = document.buffer.line(range.end.line).unwrap_or_default();
    let start_visual_column = visual_column_for(&start_text, range.start.column, tab_width);
    let end_visual_column = visual_column_for(&end_text, range.end.column, tab_width);
    let rectangular = SelectionRange::rectangular(
        range.start,
        range.end,
        start_visual_column,
        end_visual_column,
    );

    document.set_selection_set(SelectionSet::from(rectangular));
    document.preferred_vertical_column = None;
}

fn replacement_edits_for_selection_set(
    buffer: &EditorBuffer,
    selection_set: &SelectionSet,
    tab_width: usize,
    replacement_for_line: impl Fn(usize) -> String,
) -> Vec<ConcreteReplacement> {
    let main_selection = selection_set.main();

    selection_set
        .projected_lines(buffer, tab_width)
        .into_iter()
        .enumerate()
        .map(|(source_index, line)| {
            let range = buffer.clamp_range(line.range());

            ConcreteReplacement {
                range,
                replacement: replacement_for_line(source_index),
                source_index,
                main_preferred: line.selection() == main_selection,
            }
        })
        .collect()
}

pub(super) fn paste_clipboard_mode(
    document: &crate::core::Document,
    tab_width: usize,
) -> ClipboardMode {
    if document
        .selection_set()
        .ranges()
        .iter()
        .any(|selection| selection.is_rectangular())
    {
        return ClipboardMode::Rectangular {
            line_count: document
                .selection_set()
                .projected_lines(&document.buffer, tab_width)
                .len(),
        };
    }

    ClipboardMode::Linear
}

pub(super) fn paste_selection(
    document: &mut crate::core::Document,
    clipboard_mode: ClipboardMode,
    text: &str,
    tab_width: usize,
) -> bool {
    match clipboard_mode {
        ClipboardMode::Rectangular { line_count } => {
            let before_selection_set = document.selection_set().clone();
            let lines = clipboard_lines(text);
            let target_count = before_selection_set
                .projected_lines(&document.buffer, tab_width)
                .len();

            if !lines.is_empty() && lines.len() == line_count && lines.len() == target_count {
                let replacements = replacement_edits_for_selection_set(
                    &document.buffer,
                    &before_selection_set,
                    tab_width,
                    |index| lines.get(index).cloned().unwrap_or_default(),
                );

                return apply_concrete_replacements(
                    document,
                    before_selection_set,
                    replacements,
                    false,
                );
            }

            replace_selection(document, text, false, tab_width)
        }
        ClipboardMode::Linear => replace_selection(document, text, false, tab_width),
    }
}

fn clipboard_lines(text: &str) -> Vec<String> {
    text.split_inclusive(['\r', '\n'])
        .scan(String::new(), |pending_cr, chunk| {
            if !pending_cr.is_empty() {
                let mut line = std::mem::take(pending_cr);
                line.push_str(chunk);
                return Some(Some(line));
            }

            if chunk.ends_with('\r') && !chunk.ends_with("\r\n") {
                pending_cr.push_str(chunk);
                return Some(None);
            }

            Some(Some(chunk.trim_end_matches(['\r', '\n']).to_owned()))
        })
        .flatten()
        .chain(if text.ends_with(['\r', '\n']) {
            Some(String::new())
        } else {
            None
        })
        .collect()
}

fn apply_concrete_replacements(
    document: &mut crate::core::Document,
    before_selection_set: SelectionSet,
    mut replacements: Vec<ConcreteReplacement>,
    allow_grouping: bool,
) -> bool {
    if replacements.is_empty() {
        return false;
    }

    for replacement in &mut replacements {
        replacement.range = document.buffer.clamp_range(replacement.range);
    }

    replacements.sort_by_key(|replacement| {
        (
            document.buffer.byte_offset(replacement.range.start),
            document.buffer.byte_offset(replacement.range.end),
            replacement.source_index,
        )
    });
    replacements.dedup_by(|a, b| a.range == b.range && a.replacement == b.replacement);

    let Some(first) = replacements.first() else {
        return false;
    };
    let Some(last) = replacements.last() else {
        return false;
    };

    let span = EditorRange::new(first.range.start, last.range.end);
    let span_start_offset = document.buffer.byte_offset(span.start);
    let span_end_offset = document.buffer.byte_offset(span.end);
    let before_text = document.buffer.text();
    let Some(span_text) = before_text.get(span_start_offset..span_end_offset) else {
        return false;
    };

    let replacement_offsets = replacements
        .iter()
        .map(|replacement| {
            (
                document.buffer.byte_offset(replacement.range.start) - span_start_offset,
                document.buffer.byte_offset(replacement.range.end) - span_start_offset,
            )
        })
        .collect::<Vec<_>>();
    let mut replacement_text = span_text.to_owned();

    for (replacement, (start, end)) in replacements.iter().zip(replacement_offsets.iter()).rev() {
        replacement_text.replace_range(*start..*end, &replacement.replacement);
    }

    let mut final_text =
        String::with_capacity(before_text.len() - span_text.len() + replacement_text.len());
    final_text.push_str(&before_text[..span_start_offset]);
    final_text.push_str(&replacement_text);
    final_text.push_str(&before_text[span_end_offset..]);

    let mut cumulative_delta = 0isize;
    let mut after_ranges_by_source = Vec::with_capacity(replacements.len());

    for (replacement, (start, end)) in replacements.iter().zip(replacement_offsets.iter()) {
        let original_len = end.saturating_sub(*start);
        let final_start = start.saturating_add_signed(cumulative_delta);
        let final_end = final_start + replacement.replacement.len();
        cumulative_delta += replacement.replacement.len() as isize - original_len as isize;
        let Some(after_position) =
            position_for_byte_offset(&final_text, span_start_offset + final_end)
        else {
            continue;
        };

        after_ranges_by_source.push((
            replacement.source_index,
            replacement.main_preferred,
            EditorSelection::new(after_position, after_position),
        ));
    }

    let delta = document.buffer.replace_range(span, &replacement_text);

    if let Some(line_ending) = crate::core::document::detect_line_ending(&replacement_text) {
        document.line_ending = Some(line_ending);
    }

    let after_selection_set = after_selection_set_for_replacements(
        document,
        &after_ranges_by_source,
        before_selection_set.main_index(),
    );
    document.set_selection_set(after_selection_set);
    document.preferred_vertical_column = None;

    if delta.before_text == delta.after_text {
        return false;
    }

    let transaction = EditTransaction {
        delta,
        before_selection: before_selection_set.main(),
        after_selection: document.main_selection(),
    };

    if allow_grouping
        && before_selection_set.is_single()
        && document.selection_set().is_single()
        && before_selection_set.main().is_caret()
    {
        document.history.record_with_grouping(transaction);
    } else {
        document.history.record_with_selection_sets(
            transaction,
            before_selection_set,
            document.selection_set().clone(),
        );
    }

    document.refresh_text_from(span.start.line);
    true
}

fn after_selection_set_for_replacements(
    document: &crate::core::Document,
    after_ranges_by_source: &[(usize, bool, EditorSelection)],
    before_main: usize,
) -> SelectionSet {
    let mut ordered = after_ranges_by_source.to_vec();
    ordered.sort_by_key(|(source_index, _, _)| *source_index);

    let mut selections = Vec::new();
    let mut main = 0;

    for (index, (source_index, main_preferred, selection)) in ordered.iter().enumerate() {
        if *main_preferred || *source_index == before_main {
            main = index;
        }
        selections.push(*selection);
    }

    SelectionSet::from_ranges(selections, main).clamped(&document.buffer)
}
