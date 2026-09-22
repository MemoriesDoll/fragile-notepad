//! Shared typography and navigation for the application's utility windows.
use iced::widget::{button, container, text};
use iced::{Element, Fill, Font, font};

use super::styles;
use crate::message::Message;

pub fn semibold() -> Font {
    Font {
        weight: font::Weight::Semibold,
        ..Font::DEFAULT
    }
}

pub fn heading(label: &str) -> Element<'_, Message> {
    text(label).size(24).font(semibold()).into()
}

pub fn description(label: &str) -> Element<'_, Message> {
    container(text(label).size(12))
        .style(styles::info_muted)
        .into()
}

pub fn eyebrow(label: &str) -> Element<'_, Message> {
    container(text(label).size(10).font(semibold()))
        .padding([0, 10])
        .style(styles::info_muted)
        .into()
}

pub fn badge(label: impl Into<String>) -> Element<'static, Message> {
    container(text(label.into()).size(11))
        .padding([4, 8])
        .style(styles::info_badge)
        .into()
}

pub fn navigation(label: &str, selected: bool, message: Message) -> Element<'_, Message> {
    button(text(label).size(13).font(semibold()))
        .padding([10, 12])
        .width(Fill)
        .style(styles::settings_category_button(selected))
        .on_press(message)
        .into()
}
