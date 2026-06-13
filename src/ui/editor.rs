use iced::highlighter;
use iced::widget::{container, text};
use iced::{Element, Fill};

use crate::core::{Document, EditorSettings};
use crate::editor::{AdvancedEditor, EditorMetrics};
use crate::message::Message;

pub const EDITOR_ID: &str = "fragile-notepad-editor";
const BASE_TEXT_SIZE: f32 = 16.0;

pub fn view<'a>(document: &'a Document, settings: &'a EditorSettings) -> Element<'a, Message> {
    let document_id = document.id;
    let metrics = EditorMetrics {
        line_height: BASE_TEXT_SIZE * settings.zoom * 1.25,
        character_width: BASE_TEXT_SIZE * settings.zoom * 0.55,
        ..EditorMetrics::default()
    };
    AdvancedEditor::new(
        &document.buffer,
        &document.viewport,
        &document.decorations,
        &document.syntax_cache,
        highlighter::Settings {
            token: document.syntax_token.clone(),
            theme: settings.syntax_theme,
        },
        document.selection_set().clone(),
        move |action| Message::EditorAction(document_id, action),
    )
    .id(EDITOR_ID)
    .height(Fill)
    .metrics(metrics)
    .scroll(document.scroll)
    .scroll_speed(settings.scroll_speed)
    .shortcuts(&settings.shortcuts)
    .into()
}

pub fn empty<'a>() -> Element<'a, Message> {
    container(text("No document open").size(18))
        .center_x(Fill)
        .center_y(Fill)
        .height(Fill)
        .into()
}
