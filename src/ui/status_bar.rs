use iced::widget::{container, row, space, text};
use iced::{Center, Element, Fill, Length};

use crate::core::{Document, EditorSettings};
use crate::editor::layout::visual_column_for;
use crate::message::Message;
use crate::ui::styles;

const STATUS_TEXT_SIZE: u32 = 13;

pub fn view<'a>(
    document: Option<&'a Document>,
    settings: &'a EditorSettings,
    file_status: Option<&'a str>,
) -> Element<'a, Message> {
    let Some(document) = document else {
        return container(row![segment("No document", 112.0), space::horizontal()])
            .padding([2, 6])
            .width(Fill)
            .style(styles::status_bar)
            .into();
    };

    let cursor = document.main_selection().cursor;
    let cursor_column = cursor_display_column(document, settings.decorations.indent_width);
    let selection_count = document.selection_set().len();
    let line_count = document.buffer.line_count();
    let path_or_title = document
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| document.title());
    let dirty = if document.is_dirty {
        "Modified"
    } else {
        "Saved"
    };
    let line_ending = document
        .line_ending
        .map(|ending| ending.as_str().replace('\r', "CR").replace('\n', "LF"))
        .unwrap_or_else(|| String::from("Unknown"));

    container(
        row![
            container(text(path_or_title).size(STATUS_TEXT_SIZE))
                .padding([3, 2])
                .width(Fill)
                .style(styles::status_path),
            segment(file_status.unwrap_or(dirty), 148.0),
            space::horizontal(),
            segment(
                format!("Ln {}, Col {}", cursor.line + 1, cursor_column),
                118.0
            ),
            segment(
                if selection_count > 1 {
                    format!("{} selections", selection_count)
                } else {
                    String::from("1 selection")
                },
                104.0
            ),
            segment(&format!("{} lines", line_count), 86.0),
            segment(&document.syntax_token.to_uppercase(), 68.0),
            segment(&line_ending, 58.0),
            segment(
                if settings.word_wrap {
                    "Wrap"
                } else {
                    "No wrap"
                },
                72.0
            ),
            segment(&format!("{:.0}%", settings.zoom * 100.0), 54.0),
        ]
        .spacing(3)
        .padding([2, 6])
        .align_y(Center),
    )
    .width(Fill)
    .style(styles::status_bar)
    .into()
}

fn segment<'a>(label: impl Into<String>, width: f32) -> Element<'a, Message> {
    container(text(label.into()).size(STATUS_TEXT_SIZE))
        .padding([3, 6])
        .height(22)
        .width(Length::Fixed(width))
        .center_y(22)
        .style(styles::status_segment)
        .into()
}

fn cursor_display_column(document: &Document, tab_width: usize) -> usize {
    document
        .buffer
        .line(document.main_selection().cursor.line)
        .map(|line| visual_column_for(line, document.main_selection().cursor.column, tab_width) + 1)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::cursor_display_column;
    use crate::core::{Document, DocumentId};
    use crate::editor::{EditorPosition, EditorSelection};

    #[test]
    fn cursor_display_column_uses_visual_columns_for_unicode_and_tabs() {
        let mut document =
            Document::from_path(DocumentId::new(1), "unicode.txt", "\u{00e9}\t\u{597d}");
        document.set_main_selection(EditorSelection::new(
            EditorPosition::new(0, "\u{00e9}\t".len()),
            EditorPosition::new(0, "\u{00e9}\t".len()),
        ));

        assert_eq!(cursor_display_column(&document, 4), 5);
    }

    #[test]
    fn cursor_display_column_uses_configured_tab_width() {
        let mut document = Document::from_path(DocumentId::new(1), "tabs.txt", "a\tb");
        document.set_main_selection(EditorSelection::new(
            EditorPosition::new(0, "a\t".len()),
            EditorPosition::new(0, "a\t".len()),
        ));

        assert_eq!(cursor_display_column(&document, 2), 3);
        assert_eq!(cursor_display_column(&document, 8), 9);
    }
}
