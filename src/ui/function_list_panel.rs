use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Center, Element, Fill, Length};

use crate::core::Document;
use crate::editor::{FunctionEntry, FunctionKind, OutlineState, OutlineStatus};
use crate::message::Message;
use crate::ui::styles;

pub const FUNCTION_LIST_PANEL_WIDTH: f32 = 244.0;
pub const FUNCTION_LIST_PANEL_TITLE: &str = "Function List";
pub const FUNCTION_LIST_EMPTY_MESSAGE: &str = "No functions found.";
pub const FUNCTION_LIST_PENDING_MESSAGE: &str = "Scanning...";

const TITLE_TEXT_SIZE: u32 = 13;
const BODY_TEXT_SIZE: u32 = 13;
const LABEL_TEXT_SIZE: u32 = 11;
const INDENT_WIDTH: f32 = 14.0;

pub fn view<'a>(_: &'a Document, outline_state: Option<&'a OutlineState>) -> Element<'a, Message> {
    let body = match outline_state {
        Some(state) if state.status == OutlineStatus::Pending => {
            empty_state(FUNCTION_LIST_PENDING_MESSAGE)
        }
        Some(state) if !state.functions.is_empty() => function_rows(state.functions.clone()),
        _ => empty_state(FUNCTION_LIST_EMPTY_MESSAGE),
    };

    container(
        column![
            container(text(FUNCTION_LIST_PANEL_TITLE).size(TITLE_TEXT_SIZE))
                .padding([8, 10])
                .width(Fill)
                .style(styles::function_list_header),
            scrollable(body).height(Fill),
        ]
        .height(Fill),
    )
    .width(Length::Fixed(FUNCTION_LIST_PANEL_WIDTH))
    .height(Fill)
    .style(styles::function_list_panel)
    .into()
}

fn function_rows(entries: Vec<FunctionEntry>) -> Element<'static, Message> {
    let rows = entries
        .into_iter()
        .fold(column![].spacing(1).padding(4), |rows, entry| {
            rows.push(function_row(entry))
        });

    rows.into()
}

fn function_row(entry: FunctionEntry) -> Element<'static, Message> {
    button(
        row![
            space::horizontal().width(Length::Fixed(indent_for(entry.depth))),
            text(entry.name).size(BODY_TEXT_SIZE).width(Fill),
            kind_label(entry.kind),
        ]
        .spacing(7)
        .align_y(Center)
        .width(Fill),
    )
    .padding([6, 7])
    .width(Fill)
    .style(styles::menu_dropdown_item)
    .on_press(Message::FunctionListEntrySelected(entry.range.start))
    .into()
}

fn kind_label<'a>(kind: FunctionKind) -> Element<'a, Message> {
    container(text(kind_text(kind)).size(LABEL_TEXT_SIZE))
        .padding([2, 5])
        .style(styles::function_list_kind_label)
        .into()
}

fn empty_state<'a>(message: &'static str) -> Element<'a, Message> {
    container(text(message).size(BODY_TEXT_SIZE))
        .padding(12)
        .width(Fill)
        .style(styles::function_list_empty)
        .into()
}

fn kind_text(kind: FunctionKind) -> &'static str {
    match kind {
        FunctionKind::Function => "fn",
        FunctionKind::Method => "method",
        FunctionKind::Declaration => "decl",
    }
}

fn indent_for(depth: usize) -> f32 {
    (depth.min(8) as f32) * INDENT_WIDTH
}
