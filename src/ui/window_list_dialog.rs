use iced::widget::{button, column, container, opaque, row, space, stack, text};
use iced::{Alignment, Center, Element, Fill, Length};

use crate::message::{Message, WindowTarget};
use crate::ui::{centered_button_label, styles};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowListEntry {
    pub target: WindowTarget,
    pub title: String,
    pub is_focused: bool,
}

pub fn view(entries: Vec<WindowListEntry>) -> Element<'static, Message> {
    stack![
        opaque(
            container(space::vertical())
                .width(Fill)
                .height(Fill)
                .style(styles::modal_scrim)
        ),
        container(dialog(entries))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    ]
    .into()
}

fn dialog(entries: Vec<WindowListEntry>) -> Element<'static, Message> {
    let rows = entries
        .into_iter()
        .fold(column![].spacing(6), |column, entry| {
            column.push(window_row(entry))
        });

    container(
        column![
            text("Windows").size(20),
            rows.width(Fill),
            row![
                space::horizontal(),
                button(centered_button_label("Close", 13))
                    .padding([7, 18])
                    .style(styles::primary_command_button)
                    .on_press(Message::WindowListClosed),
            ]
            .align_y(Center)
            .width(Fill),
        ]
        .spacing(16)
        .align_x(Alignment::Start),
    )
    .width(Length::Fixed(500.0))
    .padding(20)
    .style(styles::modal_dialog)
    .into()
}

fn window_row(entry: WindowListEntry) -> Element<'static, Message> {
    let status = if entry.is_focused { "Active" } else { "" };
    let button_label = if entry.is_focused { "Focus" } else { "Switch" };

    row![
        column![text(entry.title).size(13), text(status).size(12),]
            .spacing(2)
            .width(Fill),
        button(centered_button_label(button_label, 13))
            .padding([6, 14])
            .style(styles::command_button)
            .on_press(Message::WindowFocusRequested(entry.target)),
    ]
    .align_y(Center)
    .spacing(12)
    .width(Fill)
    .into()
}
