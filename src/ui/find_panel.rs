use iced::widget::{button, checkbox, column, container, row, space, text, text_input};
use iced::{Center, Element, Fill, FillPortion, Length};

use crate::core::FindState;
use crate::message::Message;
use crate::ui::icons::hero::{self, HeroIcon, IconTone};
use crate::ui::{centered_button_content, controls, styles};

pub const FIND_INPUT_ID: &str = "fragile-notepad-find-input";
const BODY_TEXT_SIZE: u32 = 14;
const SECONDARY_TEXT_SIZE: u32 = 13;

pub fn view(find: &FindState, is_replace_visible: bool) -> Element<'_, Message> {
    let status = match (find.current_match, find.matches.len()) {
        (_, 0) if find.query.is_empty() => String::from("No query"),
        (_, 0) => String::from("No matches"),
        (Some(index), count) => format!("{} of {}", index + 1, count),
        (None, count) => format!("0 of {}", count),
    };
    let replace_icon = if is_replace_visible {
        HeroIcon::ChevronDown
    } else {
        HeroIcon::ChevronRight
    };

    let find_row = row![
        button(centered_button_content(hero_icon(replace_icon, 17)))
            .width(28)
            .height(28)
            .padding(0)
            .style(styles::icon_button)
            .on_press(Message::ToggleInlineReplace),
        field_label("Find"),
        text_input("Search in file", &find.query)
            .id(FIND_INPUT_ID)
            .on_input(Message::FindQueryChanged)
            .on_submit(Message::FindNext)
            .padding([7, 10])
            .size(BODY_TEXT_SIZE)
            .width(FillPortion(2))
            .style(styles::input),
        container(text(status).size(SECONDARY_TEXT_SIZE))
            .width(86)
            .height(28)
            .center_x(86)
            .center_y(28)
            .style(styles::find_status),
        checkbox(find.case_sensitive)
            .label("Case")
            .on_toggle(Message::FindCaseSensitiveToggled),
        checkbox(find.whole_word)
            .label("Word")
            .on_toggle(Message::FindWholeWordToggled),
        controls::command_button("Prev", SECONDARY_TEXT_SIZE, Message::FindPrevious),
        controls::command_button("Next", SECONDARY_TEXT_SIZE, Message::FindNext),
        controls::command_button(
            "Advanced",
            SECONDARY_TEXT_SIZE,
            Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::Find),
        ),
        button(centered_button_content(hero_icon(HeroIcon::XMark, 17)))
            .width(28)
            .height(28)
            .padding(0)
            .style(styles::text_button)
            .on_press(Message::HideFind),
    ]
    .spacing(7)
    .align_y(Center)
    .width(Fill);

    let mut rows = column![find_row].spacing(6).padding([6, 8]).width(Fill);

    if is_replace_visible {
        rows = rows.push(
            row![
                space::horizontal().width(28),
                field_label("Replace"),
                text_input("Replacement", &find.replacement)
                    .on_input(Message::FindReplacementChanged)
                    .padding([7, 10])
                    .size(BODY_TEXT_SIZE)
                    .width(FillPortion(2))
                    .style(styles::input),
                space::horizontal().width(86),
                controls::command_button("Replace", SECONDARY_TEXT_SIZE, Message::ReplaceCurrent),
                controls::primary_command_button("All", SECONDARY_TEXT_SIZE, Message::ReplaceAll),
                controls::command_button(
                    "Advanced",
                    SECONDARY_TEXT_SIZE,
                    Message::ToggleAdvancedSearch(crate::message::AdvancedSearchTab::Replace),
                ),
                space::horizontal(),
            ]
            .spacing(7)
            .align_y(Center)
            .width(Fill),
        );
    }

    container(rows)
        .width(Fill)
        .style(styles::utility_bar)
        .into()
}

fn field_label<'a>(label: &'static str) -> Element<'a, Message> {
    container(text(label).size(BODY_TEXT_SIZE))
        .width(Length::Fixed(58.0))
        .into()
}

fn hero_icon<'a>(icon: HeroIcon, size: u32) -> Element<'a, Message> {
    let tone = match icon {
        HeroIcon::ChevronDown | HeroIcon::ChevronRight | HeroIcon::XMark => IconTone::Muted,
        _ => IconTone::Text,
    };

    hero::icon(icon, size, tone)
}
