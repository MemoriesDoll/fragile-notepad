use iced::widget::{
    button, checkbox, column, container, row, rule, scrollable, space, text, text_input,
};
use iced::{Center, Element, Fill, FillPortion, Length};

use crate::core::SearchMode;
use crate::message::{AdvancedSearchTab, Message};
use crate::search_dialog::{SearchDialogState, SearchResult};
use crate::ui::{centered_button_label, centered_fill_button_label, styles};

const TABS: &[(AdvancedSearchTab, &str)] = &[
    (AdvancedSearchTab::Find, "Find"),
    (AdvancedSearchTab::Replace, "Replace"),
    (AdvancedSearchTab::FindInFiles, "Find in Files"),
    (AdvancedSearchTab::ReplaceInFiles, "Replace in Files"),
];
const BODY_TEXT_SIZE: u32 = 14;
const SECONDARY_TEXT_SIZE: u32 = 13;
const OPTION_TEXT_SIZE: u32 = 12;

pub fn view(dialog: &SearchDialogState) -> Element<'_, Message> {
    container(
        column![
            tabs(dialog.active_tab),
            rule::horizontal(1),
            container(
                row![
                    column![fields(dialog), options(dialog), status_line(dialog)]
                        .spacing(12)
                        .width(FillPortion(3)),
                    container(command_column(dialog)).width(Length::Fixed(188.0)),
                ]
                .spacing(14)
                .padding([14, 16])
                .height(Length::Fixed(304.0))
            )
            .width(Fill)
            .style(styles::settings_content),
            rule::horizontal(1),
            results(dialog),
        ]
        .height(Fill)
        .width(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(styles::settings_panel)
    .into()
}

fn tabs(active: AdvancedSearchTab) -> Element<'static, Message> {
    let tab_row = TABS.iter().fold(row![].spacing(2), |row, (tab, label)| {
        row.push(
            button(centered_button_label(label, BODY_TEXT_SIZE))
                .padding([7, 14])
                .style(styles::settings_category_button(active == *tab))
                .on_press(Message::AdvancedSearchTabSelected(*tab)),
        )
    });

    container(tab_row.padding([8, 10]).width(Fill))
        .width(Fill)
        .style(styles::settings_category_list)
        .into()
}

fn fields(dialog: &SearchDialogState) -> Element<'_, Message> {
    let mut fields = column![field_row(
        "Find what:",
        text_input("Text to find", &dialog.query)
            .on_input(Message::AdvancedSearchQueryChanged)
            .on_submit(Message::AdvancedFindNextRun)
            .padding([7, 10])
            .size(BODY_TEXT_SIZE)
            .width(Fill)
            .style(styles::input)
            .into(),
    ),]
    .spacing(8);

    if needs_replace(dialog.active_tab) {
        fields = fields.push(field_row(
            "Replace with:",
            text_input("Replacement text", &dialog.replacement)
                .on_input(Message::AdvancedSearchReplacementChanged)
                .padding([7, 10])
                .size(BODY_TEXT_SIZE)
                .width(Fill)
                .style(styles::input)
                .into(),
        ));
    }

    if needs_open_document_scope(dialog.active_tab) {
        fields = fields.push(field_row(
            "Filters:",
            text_input("*.rs;*.txt or *", &dialog.include_pattern)
                .on_input(Message::AdvancedSearchIncludeChanged)
                .padding([7, 10])
                .size(BODY_TEXT_SIZE)
                .width(Fill)
                .style(styles::input)
                .into(),
        ));
    }

    fields.into()
}

fn options(dialog: &SearchDialogState) -> Element<'_, Message> {
    row![
        group_box(
            "Search mode",
            column![
                mode_checkbox(dialog.mode, SearchMode::Normal, "Normal"),
                mode_checkbox(dialog.mode, SearchMode::Extended, "Extended"),
                mode_checkbox(dialog.mode, SearchMode::Regex, "Regular expression"),
            ]
            .spacing(5)
            .into(),
        )
        .height(Length::Fixed(104.0))
        .width(FillPortion(1)),
        group_box(
            "Options",
            column![
                checkbox(dialog.case_sensitive)
                    .label("Match case")
                    .text_size(OPTION_TEXT_SIZE)
                    .on_toggle(Message::AdvancedSearchCaseSensitiveToggled),
                checkbox(dialog.whole_word)
                    .label("Match whole word only")
                    .text_size(OPTION_TEXT_SIZE)
                    .on_toggle(Message::AdvancedSearchWholeWordToggled),
                checkbox(dialog.wrap_around)
                    .label("Wrap around")
                    .text_size(OPTION_TEXT_SIZE)
                    .on_toggle(Message::AdvancedSearchWrapAroundToggled),
            ]
            .spacing(5)
            .into(),
        )
        .height(Length::Fixed(104.0))
        .width(FillPortion(1)),
    ]
    .spacing(10)
    .into()
}

fn mode_checkbox<'a>(
    active: SearchMode,
    mode: SearchMode,
    label: &'static str,
) -> Element<'a, Message> {
    checkbox(active == mode)
        .label(label)
        .text_size(OPTION_TEXT_SIZE)
        .on_toggle(move |_| Message::AdvancedSearchModeSelected(mode))
        .into()
}

fn command_column(dialog: &SearchDialogState) -> Element<'_, Message> {
    match dialog.active_tab {
        AdvancedSearchTab::Find => column![
            action_button("Find Next", Message::AdvancedFindNextRun, true),
            action_button("Count", Message::AdvancedCountRun, true),
            action_button(
                "Find All in Current Document",
                Message::AdvancedFindAllCurrentRun,
                true
            ),
            action_button(
                "Find All in Open Documents",
                Message::AdvancedFindAllOpenRun,
                true
            ),
            space::vertical(),
            action_button("Close", Message::AdvancedSearchClosed, true),
        ],
        AdvancedSearchTab::Replace => column![
            action_button("Find Next", Message::AdvancedFindNextRun, true),
            action_button("Replace", Message::AdvancedReplaceRun, true),
            action_button(
                "Replace All in Current Document",
                Message::AdvancedReplaceAllCurrentRun,
                true
            ),
            action_button(
                "Replace All in Open Documents",
                Message::AdvancedReplaceAllOpenRun,
                true
            ),
            space::vertical(),
            action_button("Close", Message::AdvancedSearchClosed, true),
        ],
        AdvancedSearchTab::FindInFiles => column![
            action_button("Find All", Message::AdvancedFindAllOpenRun, true),
            action_button("Count", Message::AdvancedCountRun, true),
            action_button("Find Next", Message::AdvancedFindNextRun, true),
            space::vertical(),
            scope_hint(dialog),
            action_button("Close", Message::AdvancedSearchClosed, true),
        ],
        AdvancedSearchTab::ReplaceInFiles => column![
            action_button("Find All", Message::AdvancedFindAllOpenRun, true),
            action_button("Replace", Message::AdvancedReplaceRun, true),
            action_button("Replace All", Message::AdvancedReplaceAllOpenRun, true),
            space::vertical(),
            scope_hint(dialog),
            action_button("Close", Message::AdvancedSearchClosed, true),
        ],
    }
    .spacing(7)
    .height(Fill)
    .into()
}

fn status_line(dialog: &SearchDialogState) -> Element<'_, Message> {
    row![
        text(&dialog.status).size(SECONDARY_TEXT_SIZE),
        space::horizontal(),
        text(scope_label(dialog.active_tab)).size(SECONDARY_TEXT_SIZE),
    ]
    .align_y(Center)
    .into()
}

fn results(dialog: &SearchDialogState) -> Element<'_, Message> {
    let result_list = dialog
        .results
        .iter()
        .fold(column![].spacing(1), |column, result| {
            column.push(result_row(result))
        });

    container(
        column![
            row![
                text("Search results").size(BODY_TEXT_SIZE),
                text(&dialog.status).size(SECONDARY_TEXT_SIZE),
                space::horizontal(),
            ]
            .spacing(10)
            .align_y(Center)
            .padding([8, 12]),
            scrollable(result_list.padding([0, 8])).height(Fill),
        ]
        .height(Fill),
    )
    .height(Fill)
    .width(Fill)
    .style(styles::settings_content)
    .into()
}

fn result_row(result: &SearchResult) -> Element<'_, Message> {
    button(
        row![
            text(format!(
                "{}:{}:{}",
                result.document_title,
                result.selection.range().start.line + 1,
                result.selection.range().start.column + 1
            ))
            .size(SECONDARY_TEXT_SIZE)
            .width(Length::Fixed(230.0)),
            text(&result.preview).size(SECONDARY_TEXT_SIZE).width(Fill),
        ]
        .spacing(12)
        .align_y(Center),
    )
    .padding([6, 8])
    .width(Fill)
    .style(styles::menu_dropdown_item)
    .on_press(Message::AdvancedSearchResultSelected(
        result.document_id,
        result.selection,
    ))
    .into()
}

fn field_row<'a>(label: &'static str, control: Element<'a, Message>) -> Element<'a, Message> {
    row![text(label).size(BODY_TEXT_SIZE).width(108), control]
        .spacing(10)
        .align_y(Center)
        .height(Length::Fixed(34.0))
        .into()
}

fn group_box<'a>(
    title: &'static str,
    content: Element<'a, Message>,
) -> container::Container<'a, Message> {
    container(column![text(title).size(SECONDARY_TEXT_SIZE), content].spacing(7))
        .padding([8, 10])
        .style(styles::find_status)
}

fn action_button<'a>(label: &'static str, message: Message, enabled: bool) -> Element<'a, Message> {
    button(centered_fill_button_label(label, SECONDARY_TEXT_SIZE))
        .padding([7, 10])
        .width(Fill)
        .style(styles::command_button)
        .on_press_maybe(enabled.then_some(message))
        .into()
}

fn scope_hint(dialog: &SearchDialogState) -> Element<'_, Message> {
    let filter = dialog.include_pattern.trim();
    let label = if filter.is_empty() {
        "Scope: open documents"
    } else {
        "Scope: filtered open documents"
    };

    container(text(label).size(SECONDARY_TEXT_SIZE))
        .padding([6, 8])
        .width(Fill)
        .style(styles::find_status)
        .into()
}

const fn needs_replace(tab: AdvancedSearchTab) -> bool {
    matches!(
        tab,
        AdvancedSearchTab::Replace | AdvancedSearchTab::ReplaceInFiles
    )
}

const fn needs_open_document_scope(tab: AdvancedSearchTab) -> bool {
    matches!(
        tab,
        AdvancedSearchTab::FindInFiles | AdvancedSearchTab::ReplaceInFiles
    )
}

const fn scope_label(tab: AdvancedSearchTab) -> &'static str {
    match tab {
        AdvancedSearchTab::Find | AdvancedSearchTab::Replace => "Current document",
        AdvancedSearchTab::FindInFiles | AdvancedSearchTab::ReplaceInFiles => "Open documents",
    }
}
