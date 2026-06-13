use iced::widget::{button, column, container, opaque, row, space, stack, text};
use iced::{Alignment, Center, Element, Fill, Length};

use crate::core::Document;
use crate::message::{DirtyCloseDecision, Message};
use crate::ui::{centered_button_label, styles};

pub fn view(document: &Document) -> Element<'_, Message> {
    stack![
        opaque(
            container(space::vertical())
                .width(Fill)
                .height(Fill)
                .style(styles::modal_scrim)
        ),
        container(dialog(document))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    ]
    .into()
}

fn dialog(document: &Document) -> Element<'_, Message> {
    let document_id = document.id;

    container(
        column![
            column![
                text("Save changes?").size(18),
                text(format!(
                    "Do you want to save changes to \"{}\" before closing?",
                    document.title()
                ))
                .size(13)
                .width(Fill),
            ]
            .spacing(8),
            row![
                space::horizontal(),
                button(centered_button_label("Discard", 13))
                    .padding([7, 14])
                    .style(styles::danger_command_button)
                    .on_press(Message::DirtyCloseResolved(
                        document_id,
                        DirtyCloseDecision::Discard
                    )),
                button(centered_button_label("Cancel", 13))
                    .padding([7, 14])
                    .style(styles::command_button)
                    .on_press(Message::DirtyCloseResolved(
                        document_id,
                        DirtyCloseDecision::Cancel
                    )),
                button(centered_button_label("Save", 13))
                    .padding([7, 18])
                    .style(styles::primary_command_button)
                    .on_press(Message::DirtyCloseResolved(
                        document_id,
                        DirtyCloseDecision::Save
                    )),
            ]
            .spacing(8)
            .align_y(Center)
            .width(Fill),
        ]
        .spacing(18)
        .align_x(Alignment::Start),
    )
    .width(Length::Fixed(420.0))
    .padding(20)
    .style(styles::modal_dialog)
    .into()
}
