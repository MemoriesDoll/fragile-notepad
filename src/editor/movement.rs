use unicode_segmentation::UnicodeSegmentation;

use super::buffer::EditorBuffer;
use super::position::{EditorPosition, position_for_byte_offset};
use super::widget::CaretMotion;
use super::word::is_default_word_char;

pub fn is_vertical_motion(motion: CaretMotion) -> bool {
    matches!(
        motion,
        CaretMotion::Up | CaretMotion::Down | CaretMotion::PageUp | CaretMotion::PageDown
    )
}

pub fn move_position(
    buffer: &EditorBuffer,
    position: EditorPosition,
    motion: CaretMotion,
) -> EditorPosition {
    let position = buffer.clamp_position(position);

    match motion {
        CaretMotion::Left => previous_position(buffer, position).unwrap_or(position),
        CaretMotion::Right => next_position(buffer, position).unwrap_or(position),
        CaretMotion::WordLeft => previous_word_position(buffer, position).unwrap_or(position),
        CaretMotion::WordRight => next_word_position(buffer, position).unwrap_or(position),
        CaretMotion::Up | CaretMotion::Down | CaretMotion::PageUp | CaretMotion::PageDown => {
            move_position_with_column(buffer, position, motion, position.column)
        }
        CaretMotion::ParagraphUp => previous_paragraph_position(buffer, position),
        CaretMotion::ParagraphDown => next_paragraph_position(buffer, position),
        CaretMotion::LineStart => EditorPosition::new(position.line, 0),
        CaretMotion::LineEnd => line_end(buffer, position.line),
        CaretMotion::DocumentStart => EditorPosition::new(0, 0),
        CaretMotion::DocumentEnd => document_end(buffer),
    }
}

pub fn move_position_with_column(
    buffer: &EditorBuffer,
    position: EditorPosition,
    motion: CaretMotion,
    column: usize,
) -> EditorPosition {
    let line = match motion {
        CaretMotion::Up => position.line.saturating_sub(1),
        CaretMotion::Down => position
            .line
            .saturating_add(1)
            .min(buffer.line_count().saturating_sub(1)),
        CaretMotion::PageUp => position.line.saturating_sub(20),
        CaretMotion::PageDown => position
            .line
            .saturating_add(20)
            .min(buffer.line_count().saturating_sub(1)),
        _ => position.line,
    };

    buffer.clamp_position(EditorPosition::new(line, column))
}

pub fn previous_grapheme_position(
    buffer: &EditorBuffer,
    position: EditorPosition,
) -> Option<EditorPosition> {
    let offset = buffer.byte_offset(position);
    let text = buffer.text();
    let offset = previous_grapheme_offset(&text, offset)?;

    position_for_byte_offset(&text, offset)
}

pub fn next_grapheme_position(
    buffer: &EditorBuffer,
    position: EditorPosition,
) -> Option<EditorPosition> {
    let offset = buffer.byte_offset(position);
    let text = buffer.text();
    let offset = next_grapheme_offset(&text, offset)?;

    position_for_byte_offset(&text, offset)
}

pub fn previous_grapheme_offset(text: &str, offset: usize) -> Option<usize> {
    text.get(..offset)?
        .grapheme_indices(true)
        .next_back()
        .map(|(index, _)| index)
}

pub fn next_grapheme_offset(text: &str, offset: usize) -> Option<usize> {
    let next = text
        .get(offset..)?
        .graphemes(true)
        .next()
        .filter(|grapheme| !grapheme.is_empty())?;

    Some(offset + next.len())
}

pub fn line_end(buffer: &EditorBuffer, line: usize) -> EditorPosition {
    EditorPosition::new(line, buffer.line(line).unwrap_or_default().len())
}

pub fn document_end(buffer: &EditorBuffer) -> EditorPosition {
    let line = buffer.line_count().saturating_sub(1);

    line_end(buffer, line)
}

fn previous_position(buffer: &EditorBuffer, position: EditorPosition) -> Option<EditorPosition> {
    previous_grapheme_position(buffer, position)
}

fn next_position(buffer: &EditorBuffer, position: EditorPosition) -> Option<EditorPosition> {
    next_grapheme_position(buffer, position)
}

fn previous_word_position(
    buffer: &EditorBuffer,
    position: EditorPosition,
) -> Option<EditorPosition> {
    let text = buffer.text();
    let mut offset = buffer.byte_offset(position);

    while let Some(previous) = previous_grapheme_offset(&text, offset) {
        if text[previous..]
            .chars()
            .next()
            .is_some_and(is_default_word_char)
        {
            break;
        }

        offset = previous;
    }

    while let Some(previous) = previous_grapheme_offset(&text, offset) {
        if !text[previous..]
            .chars()
            .next()
            .is_some_and(is_default_word_char)
        {
            break;
        }

        offset = previous;
    }

    position_for_byte_offset(&text, offset)
}

fn next_word_position(buffer: &EditorBuffer, position: EditorPosition) -> Option<EditorPosition> {
    let text = buffer.text();
    let mut offset = buffer.byte_offset(position);

    while offset < text.len() {
        let ch = text[offset..].chars().next()?;
        if !is_default_word_char(ch) {
            break;
        }

        offset += ch.len_utf8();
    }

    while offset < text.len() {
        let ch = text[offset..].chars().next()?;
        if is_default_word_char(ch) {
            break;
        }

        offset += ch.len_utf8();
    }

    position_for_byte_offset(&text, offset)
}

fn previous_paragraph_position(buffer: &EditorBuffer, position: EditorPosition) -> EditorPosition {
    let current_line = buffer.clamp_position(position).line;
    let Some(mut line) = current_line.checked_sub(1) else {
        return EditorPosition::new(0, 0);
    };

    while line > 0 && is_blank_line(buffer, line) {
        line -= 1;
    }

    while line > 0 && !is_blank_line(buffer, line - 1) {
        line -= 1;
    }

    EditorPosition::new(line, 0)
}

fn next_paragraph_position(buffer: &EditorBuffer, position: EditorPosition) -> EditorPosition {
    let current_line = buffer.clamp_position(position).line;
    let last_line = buffer.line_count().saturating_sub(1);

    if current_line >= last_line {
        return document_end(buffer);
    }

    let mut line = current_line + 1;

    if !is_blank_line(buffer, current_line) {
        while line <= last_line && !is_blank_line(buffer, line) {
            line += 1;
        }
    }

    while line <= last_line && is_blank_line(buffer, line) {
        line += 1;
    }

    if line > last_line {
        return document_end(buffer);
    }

    EditorPosition::new(line, 0)
}

fn is_blank_line(buffer: &EditorBuffer, line: usize) -> bool {
    buffer.line(line).unwrap_or_default().trim().is_empty()
}
