use super::position::{EditorPosition, EditorRange, position_after_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditDelta {
    pub before_range: EditorRange,
    pub after_range: EditorRange,
    pub before_text: String,
    pub after_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBuffer {
    text: String,
    line_starts: Vec<usize>,
}

impl EditorBuffer {
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let line_starts = line_starts(&text);

        Self { text, line_starts }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn text_for_save(&self) -> &str {
        &self.text
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line(&self, index: usize) -> Option<&str> {
        let start = *self.line_starts.get(index)?;
        let end = self.line_content_end(index)?;

        self.text.get(start..end)
    }

    pub fn replace_range(&mut self, range: EditorRange, replacement: &str) -> EditDelta {
        let before_range = self.clamp_range(range);
        let start_offset = self.byte_offset(before_range.start);
        let end_offset = self.byte_offset(before_range.end);
        let before_text = self.text[start_offset..end_offset].to_owned();

        self.text
            .replace_range(start_offset..end_offset, replacement);
        self.line_starts = line_starts(&self.text);

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

    pub fn clamp_position(&self, position: EditorPosition) -> EditorPosition {
        let line = position.line.min(self.line_count().saturating_sub(1));
        let line_start = self.line_starts[line];
        let line_end = self.line_content_end(line).unwrap_or(line_start);
        let target = line_start + position.column.min(line_end.saturating_sub(line_start));
        let column = previous_char_boundary(&self.text, line_start, target) - line_start;

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
        self.line_starts[position.line] + position.column
    }

    fn line_content_end(&self, index: usize) -> Option<usize> {
        let start = *self.line_starts.get(index)?;
        let next_start = self.line_starts.get(index + 1).copied();
        let raw_end = next_start.unwrap_or(self.text.len());
        let bytes = self.text.as_bytes();

        if raw_end > start && bytes.get(raw_end - 1) == Some(&b'\n') {
            if raw_end > start + 1 && bytes.get(raw_end - 2) == Some(&b'\r') {
                return Some(raw_end - 2);
            }

            return Some(raw_end - 1);
        }

        if raw_end > start && bytes.get(raw_end - 1) == Some(&b'\r') {
            return Some(raw_end - 1);
        }

        Some(raw_end)
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

fn previous_char_boundary(text: &str, min: usize, target: usize) -> usize {
    let mut index = target.min(text.len());

    while index > min && !text.is_char_boundary(index) {
        index -= 1;
    }

    index
}
