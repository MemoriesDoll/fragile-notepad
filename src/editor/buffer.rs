use super::position::{EditorPosition, EditorRange, position_after_text};
use ropey::Rope;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditDelta {
    pub before_range: EditorRange,
    pub after_range: EditorRange,
    pub before_text: String,
    pub after_text: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EditorBuffer {
    rope: Rope,
    line_starts: Vec<usize>,
}

impl fmt::Debug for EditorBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EditorBuffer")
            .field("text", &self.text())
            .finish()
    }
}

impl EditorBuffer {
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let line_starts = line_starts(&text);

        Self {
            rope: Rope::from_str(&text),
            line_starts,
        }
    }

    /// Compatibility copy of the complete buffer text.
    ///
    /// The rope is the authoritative storage. New code that can operate
    /// incrementally should prefer `chunks`, `slice_text`, or `line_text`.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Compatibility copy of the complete buffer text for existing save paths.
    pub fn text_for_save(&self) -> String {
        self.text()
    }

    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        self.rope.chunks()
    }

    pub fn slice_text(&self, range: EditorRange) -> String {
        let range = self.clamp_range(range);
        let start = self.char_offset(range.start);
        let end = self.char_offset(range.end);

        self.rope.slice(start..end).to_string()
    }

    pub fn position_for_byte_offset(&self, byte_offset: usize) -> Option<EditorPosition> {
        if byte_offset > self.byte_len() {
            return None;
        }

        self.byte_to_char_boundary(byte_offset)?;
        let line = match self.line_starts.binary_search(&byte_offset) {
            Ok(line) => line,
            Err(next_line) => next_line.saturating_sub(1),
        };
        let line_start = *self.line_starts.get(line)?;
        let column = byte_offset.saturating_sub(line_start);

        Some(EditorPosition::new(line, column))
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line(&self, index: usize) -> Option<String> {
        self.line_text(index)
    }

    pub fn line_text(&self, index: usize) -> Option<String> {
        if index >= self.line_count() {
            return None;
        }

        let start = self.byte_to_char_boundary(*self.line_starts.get(index)?)?;
        let end = self.line_content_end_char(index);

        Some(self.rope.slice(start..end).to_string())
    }

    pub fn replace_range(&mut self, range: EditorRange, replacement: &str) -> EditDelta {
        let before_range = self.clamp_range(range);
        let start_offset = self.char_offset(before_range.start);
        let end_offset = self.char_offset(before_range.end);
        let before_text = self.rope.slice(start_offset..end_offset).to_string();

        self.rope.remove(start_offset..end_offset);
        self.rope.insert(start_offset, replacement);
        self.rebuild_line_starts();

        let after_end = position_after_text(before_range.start, replacement);
        let after_range = EditorRange::new(
            self.clamp_position(before_range.start),
            self.clamp_position(after_end),
        );

        EditDelta {
            before_range,
            after_range,
            before_text,
            after_text: replacement.to_owned(),
        }
    }

    pub fn append_text(&mut self, text: &str) {
        self.rope.insert(self.rope.len_chars(), text);
        self.rebuild_line_starts();
    }

    pub fn clamp_position(&self, position: EditorPosition) -> EditorPosition {
        let line = position.line.min(self.line_count().saturating_sub(1));
        let line_start = self
            .line_starts
            .get(line)
            .and_then(|offset| self.byte_to_char_boundary(*offset))
            .unwrap_or(0);
        let line_end = self.line_content_end_char(line);
        let line_byte_len = self.rope.slice(line_start..line_end).len_bytes();
        let target_column = position.column.min(line_byte_len);
        let column =
            previous_char_boundary_in_slice(self.rope.slice(line_start..line_end), target_column);

        EditorPosition::new(line, column)
    }

    pub fn clamp_range(&self, range: EditorRange) -> EditorRange {
        let range = range.normalized();

        EditorRange::new(
            self.clamp_position(range.start),
            self.clamp_position(range.end),
        )
    }

    pub fn byte_offset(&self, position: EditorPosition) -> usize {
        let position = self.clamp_position(position);
        self.line_starts
            .get(position.line)
            .copied()
            .unwrap_or(0)
            .saturating_add(position.column)
    }

    fn byte_len(&self) -> usize {
        self.rope.len_bytes()
    }

    fn char_offset(&self, position: EditorPosition) -> usize {
        let position = self.clamp_position(position);
        let line_start_byte = self.line_starts.get(position.line).copied().unwrap_or(0);
        let line_start_char = self.byte_to_char_boundary(line_start_byte).unwrap_or(0);

        self.rope
            .byte_to_char(line_start_byte + position.column)
            .max(line_start_char)
    }

    fn byte_to_char_boundary(&self, byte_offset: usize) -> Option<usize> {
        let char_offset = self.rope.try_byte_to_char(byte_offset).ok()?;

        if self.rope.char_to_byte(char_offset) == byte_offset {
            Some(char_offset)
        } else {
            None
        }
    }

    fn line_content_end_char(&self, index: usize) -> usize {
        let Some(start_byte) = self.line_starts.get(index).copied() else {
            return self.rope.len_chars();
        };
        let start = self.byte_to_char_boundary(start_byte).unwrap_or(0);
        let raw_end = if index + 1 < self.line_count() {
            self.line_starts
                .get(index + 1)
                .and_then(|offset| self.byte_to_char_boundary(*offset))
                .unwrap_or_else(|| self.rope.len_chars())
        } else {
            self.rope.len_chars()
        };
        let line = self.rope.slice(start..raw_end);
        let mut end = raw_end;
        let line_char_count = line.len_chars();

        if line_char_count > 0 && line.char(line_char_count - 1) == '\n' {
            end -= 1;

            if line_char_count > 1 && line.char(line_char_count - 2) == '\r' {
                end -= 1;
            }
        } else if line_char_count > 0 && line.char(line_char_count - 1) == '\r' {
            end -= 1;

            if line_char_count > 1 && line.char(line_char_count - 2) == '\n' {
                end -= 1;
            }
        }

        end
    }

    fn rebuild_line_starts(&mut self) {
        self.line_starts = line_starts(&self.text());
    }
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    let mut index = 0;
    let bytes = text.as_bytes();

    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                index += 2;
                starts.push(index);
            }
            b'\n' if bytes.get(index + 1) == Some(&b'\r') => {
                index += 2;
                starts.push(index);
            }
            b'\r' | b'\n' => {
                index += 1;
                starts.push(index);
            }
            _ => {
                let Some(ch) = text[index..].chars().next() else {
                    break;
                };
                index += ch.len_utf8();
            }
        }
    }

    starts
}

fn previous_char_boundary_in_slice(text: ropey::RopeSlice<'_>, target: usize) -> usize {
    let mut index = target.min(text.len_bytes());

    while index > 0 {
        let Ok(char_index) = text.try_byte_to_char(index) else {
            index -= 1;
            continue;
        };

        if text.char_to_byte(char_index) == index {
            break;
        }

        index -= 1;
    }

    index
}
