use iced::widget::{button, column, container, opaque, row, scrollable, space, stack, text};
use iced::{Center, Element, Fill, Font};

use crate::message::{Message, WindowTarget};
use crate::ui::icons::hero::{self, HeroIcon, IconTone};
use crate::ui::{motion, styles};

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
        container(motion::popup(dialog(entries)))
            .padding(24)
            .center(Fill),
    ]
    .into()
}

fn dialog(entries: Vec<WindowListEntry>) -> Element<'static, Message> {
    let count = entries.len();
    let rows = entries
        .into_iter()
        .fold(column![].spacing(8), |rows, entry| {
            rows.push(window_row(entry))
        });

    container(
        column![
            row![
                column![
                    text("Windows").size(24).font(Font {
                        weight: iced::font::Weight::Semibold,
                        ..Font::DEFAULT
                    }),
                    container(text("Pick a window to bring it to the front.").size(13))
                        .style(styles::info_muted),
                ]
                .spacing(5)
                .width(Fill),
                container(text(format!("{count} open")).size(12))
                    .padding([5, 9])
                    .style(styles::info_badge),
            ]
            .spacing(16)
            .align_y(Center),
            container(scrollable(rows).smooth_scroll(true).spacing(8).height(Fill))
                .max_height(250)
                .width(Fill),
            row![
                container(text("Your open documents stay in the editor.").size(12))
                    .style(styles::info_muted)
                    .width(Fill),
                button(text("Done").size(13))
                    .padding([9, 22])
                    .style(styles::command_button)
                    .on_press(Message::WindowListClosed),
            ]
            .spacing(16)
            .align_y(Center),
        ]
        .spacing(20),
    )
    .width(Fill)
    .max_width(560)
    .height(Fill)
    .max_height((count as f32 * 72.0 + 180.0).min(400.0))
    .padding(24)
    .style(styles::utility_dialog)
    .into()
}

fn window_row(entry: WindowListEntry) -> Element<'static, Message> {
    let (label, symbol) = match entry.target {
        WindowTarget::Main => ("Editor", "Aa"),
        WindowTarget::AdvancedSearch => ("Find & Replace", ".*"),
        WindowTarget::Settings => ("Preferences", "≡"),
    };
    let title = match entry.target {
        WindowTarget::Main => entry
            .title
            .trim_end_matches(" - Fragile Notepad")
            .to_owned(),
        WindowTarget::AdvancedSearch => "Search and edit across documents".into(),
        WindowTarget::Settings => "Personalize your workspace".into(),
    };
    let mut trailing = row![].spacing(12).align_y(Center);
    if entry.is_focused {
        trailing = trailing.push(
            container(text("Active").size(11))
                .padding([4, 8])
                .style(styles::info_badge),
        );
    }
    trailing = trailing.push(hero::icon(HeroIcon::ChevronRight, 16, IconTone::Muted));

    button(
        row![
            container(text(symbol).size(18).font(Font::MONOSPACE))
                .center(40)
                .style(styles::info_card),
            column![
                text(label).size(14).font(Font {
                    weight: iced::font::Weight::Semibold,
                    ..Font::DEFAULT
                }),
                container(
                    text(title)
                        .size(12)
                        .wrapping(iced::widget::text::Wrapping::None)
                )
                .style(styles::info_muted),
            ]
            .spacing(4)
            .width(Fill)
            .clip(true),
            trailing,
        ]
        .spacing(14)
        .align_y(Center),
    )
    .padding(12)
    .width(Fill)
    .style(styles::utility_selection(entry.is_focused))
    .on_press(Message::WindowFocusRequested(entry.target))
    .into()
}
