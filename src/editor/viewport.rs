use super::fold::FoldModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisibleRow {
    pub row: usize,
    pub document_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportModel {
    line_count: usize,
    visible_lines: Vec<usize>,
    document_to_visible: Vec<Option<usize>>,
}

impl ViewportModel {
    pub fn new(line_count: usize, folds: &FoldModel) -> Self {
        let mut visible_lines = Vec::new();
        let mut document_to_visible = vec![None; line_count];
        let mut line = 0;

        while line < line_count {
            let row = visible_lines.len();
            visible_lines.push(line);
            document_to_visible[line] = Some(row);

            if let Some(range) = folds
                .collapsed_ranges()
                .copied()
                .filter(|range| range.start_line == line)
                .max_by_key(|range| range.end_line)
            {
                line = range.end_line.saturating_add(1);
            } else {
                line += 1;
            }
        }

        Self {
            line_count,
            visible_lines,
            document_to_visible,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_count
    }

    pub fn visible_row_count(&self) -> usize {
        self.visible_lines.len()
    }

    pub fn document_line_to_visible_row(&self, document_line: usize) -> Option<usize> {
        self.document_to_visible
            .get(document_line)
            .copied()
            .flatten()
    }

    pub fn visible_row_to_document_line(&self, visible_row: usize) -> Option<usize> {
        self.visible_lines.get(visible_row).copied()
    }

    pub fn visible_rows(&self) -> impl Iterator<Item = VisibleRow> + '_ {
        self.visible_lines
            .iter()
            .enumerate()
            .map(|(row, document_line)| VisibleRow {
                row,
                document_line: *document_line,
            })
    }

    /// Synchronizes an unfolded viewport after text has only been appended.
    ///
    /// Loading documents do not expose folds until their text index is
    /// complete. Extending the identity mapping avoids rebuilding every
    /// previously loaded line for each streamed chunk.
    pub fn sync_unfolded_line_count(&mut self, line_count: usize) {
        if line_count < self.line_count
            || self.visible_lines.len() != self.line_count
            || self.document_to_visible.len() != self.line_count
        {
            self.visible_lines = (0..line_count).collect();
            self.document_to_visible = (0..line_count).map(Some).collect();
            self.line_count = line_count;
            return;
        }

        for line in self.line_count..line_count {
            self.visible_lines.push(line);
            self.document_to_visible.push(Some(line));
        }
        self.line_count = line_count;
    }
}
