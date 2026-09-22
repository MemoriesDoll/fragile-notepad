use iced::widget::{
    button, checkbox, column, container, row, rule, scrollable, space, text, text_input,
};
use iced::{Center, Element, Fill, FillPortion, Font};

use crate::core::SearchMode;
use crate::message::{AdvancedSearchTab, Message};
use crate::search_dialog::{SearchDialogState, SearchResult};
use crate::ui::{styles, utility};

pub fn view(dialog: &SearchDialogState) -> Element<'_, Message> {
    let go_to = dialog.active_tab == AdvancedSearchTab::GoToLine;
    let title = match dialog.active_tab {
        AdvancedSearchTab::Find | AdvancedSearchTab::FindInFiles => "Find text",
        AdvancedSearchTab::Replace | AdvancedSearchTab::ReplaceInFiles => "Replace text",
        AdvancedSearchTab::GoToLine => "Go to line",
    };
    let header = row![
        utility::heading(title),
        space::horizontal(),
        utility::badge(scope_label(dialog.active_tab)),
    ]
    .align_y(Center)
    .spacing(12);

    let body: Element<'_, Message> = if go_to {
        column![
            header,
            container(
                column![
                    field(
                        "Line number",
                        text_input("e.g. 120", &dialog.go_to_line)
                            .on_input(Message::AdvancedSearchQueryChanged)
                            .on_submit(Message::AdvancedFindNextRun)
                            .padding([10, 12])
                            .size(15)
                            .style(styles::input)
                            .into()
                    ),
                    row![
                        space::horizontal(),
                        action(
                            "Go to line",
                            Message::AdvancedFindNextRun,
                            true,
                            !dialog.go_to_line.trim().is_empty()
                        )
                    ],
                ]
                .spacing(16)
            )
            .padding(20)
            .width(Fill)
            .style(styles::utility_card),
            space::vertical(),
        ]
        .spacing(22)
        .into()
    } else {
        column![
            header,
            container(
                column![
                    scrollable(container(search_form(dialog)).padding(16))
                        .smooth_scroll(true)
                        .spacing(8)
                        .height(Fill),
                    container(commands(dialog)).padding([12, 16]).width(Fill),
                ]
                .height(Fill)
            )
            .style(styles::utility_card)
            .height(FillPortion(3))
            .width(Fill),
            results(dialog),
        ]
        .spacing(18)
        .height(Fill)
        .into()
    };

    container(
        column![
            row![
                navigation(dialog.active_tab),
                container(body).padding(24).width(Fill).height(Fill),
            ]
            .height(Fill),
            rule::horizontal(1),
            row![
                container(text(status_label(dialog)).size(12))
                    .style(styles::info_muted)
                    .width(Fill),
                action("Close", Message::AdvancedSearchClosed, false, true),
            ]
            .spacing(16)
            .align_y(Center)
            .padding([12, 20]),
        ]
        .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(styles::settings_panel)
    .into()
}

fn navigation(active: AdvancedSearchTab) -> Element<'static, Message> {
    let nav = |label, tab| {
        utility::navigation(
            label,
            active == tab,
            Message::AdvancedSearchTabSelected(tab),
        )
    };
    container(
        column![
            utility::eyebrow("CURRENT DOCUMENT"),
            nav("Find", AdvancedSearchTab::Find),
            nav("Replace", AdvancedSearchTab::Replace),
            space::vertical().height(14),
            utility::eyebrow("OPEN DOCUMENTS"),
            nav("Find all", AdvancedSearchTab::FindInFiles),
            nav("Replace all", AdvancedSearchTab::ReplaceInFiles),
            space::vertical().height(14),
            nav("Go to line", AdvancedSearchTab::GoToLine),
            space::vertical(),
        ]
        .spacing(6)
        .height(Fill),
    )
    .padding([24, 14])
    .width(174)
    .height(Fill)
    .style(styles::settings_category_list)
    .into()
}

fn search_form(dialog: &SearchDialogState) -> Element<'_, Message> {
    let submit = if open_scope(dialog.active_tab) {
        Message::AdvancedFindAllOpenRun
    } else {
        Message::AdvancedFindNextRun
    };
    let find = field(
        "Find",
        text_input("Enter text or a pattern", &dialog.query)
            .on_input(Message::AdvancedSearchQueryChanged)
            .on_submit(submit)
            .padding([10, 12])
            .size(15)
            .width(Fill)
            .style(styles::input)
            .into(),
    );
    let input_fields: Element<'_, Message> = if replace_mode(dialog.active_tab) {
        row![
            container(find).width(Fill),
            container(field(
                "Replace with",
                text_input("Replacement text", &dialog.replacement)
                    .on_input(Message::AdvancedSearchReplacementChanged)
                    .padding([10, 12])
                    .size(15)
                    .width(Fill)
                    .style(styles::input)
                    .into()
            ))
            .width(Fill),
        ]
        .spacing(14)
        .into()
    } else {
        find
    };
    let mut fields = column![input_fields].spacing(14);
    if open_scope(dialog.active_tab) {
        fields = fields.push(
            row![
                text("File names").size(12).width(76),
                text_input(
                    "All open documents · e.g. *.rs;*.txt",
                    &dialog.include_pattern
                )
                .on_input(Message::AdvancedSearchIncludeChanged)
                .padding([8, 10])
                .size(13)
                .width(Fill)
                .style(styles::input),
            ]
            .spacing(12)
            .align_y(Center),
        );
    }
    column![fields, options(dialog)].spacing(16).into()
}

fn options(dialog: &SearchDialogState) -> Element<'_, Message> {
    let modes = [
        (SearchMode::Normal, "Plain text"),
        (SearchMode::Extended, "Escapes"),
        (SearchMode::Regex, "Regex"),
    ]
    .into_iter()
    .fold(row![].spacing(4), |row, (mode, label)| {
        row.push(
            button(text(label).size(12))
                .padding([6, 12])
                .style(styles::settings_category_button(dialog.mode == mode))
                .on_press(Message::AdvancedSearchModeSelected(mode)),
        )
    });
    let hint = match dialog.mode {
        SearchMode::Extended => Some(r"Escapes: \n, \t, \r."),
        SearchMode::Regex if replace_mode(dialog.active_tab) => {
            Some("Use $1, $2, … for captured groups.")
        }
        _ => None,
    };
    let mut flags = row![
        checkbox(dialog.case_sensitive)
            .label("Match case")
            .text_size(12)
            .size(16)
            .on_toggle(Message::AdvancedSearchCaseSensitiveToggled),
        checkbox(dialog.whole_word)
            .label("Whole words")
            .text_size(12)
            .size(16)
            .on_toggle(Message::AdvancedSearchWholeWordToggled),
    ]
    .spacing(18)
    .align_y(Center);
    if !open_scope(dialog.active_tab) {
        flags = flags.push(
            checkbox(dialog.wrap_around)
                .label("Wrap around")
                .text_size(12)
                .size(16)
                .on_toggle(Message::AdvancedSearchWrapAroundToggled),
        );
    }
    let mut options = column![modes, flags].spacing(10);
    if let Some(hint) = hint {
        options = options.push(utility::description(hint));
    }
    options.into()
}

fn commands(dialog: &SearchDialogState) -> Element<'_, Message> {
    let enabled = !dialog.query.is_empty();
    let is_open = open_scope(dialog.active_tab);
    let find_all = if is_open {
        Message::AdvancedFindAllOpenRun
    } else {
        Message::AdvancedFindAllCurrentRun
    };
    let replace_all = if is_open {
        Message::AdvancedReplaceAllOpenRun
    } else {
        Message::AdvancedReplaceAllCurrentRun
    };
    let mut actions = row![].spacing(8);
    if !is_open {
        actions = actions.push(action(
            "Find next",
            Message::AdvancedFindNextRun,
            true,
            enabled,
        ));
    }
    actions = actions
        .push(action("Find all", find_all, is_open, enabled))
        .push(action("Count", Message::AdvancedCountRun, false, enabled));
    if replace_mode(dialog.active_tab) {
        if !is_open {
            actions = actions.push(action(
                "Replace",
                Message::AdvancedReplaceRun,
                false,
                enabled,
            ));
        }
        actions = actions.push(action("Replace all", replace_all, false, enabled));
    }
    actions.into()
}

fn results(dialog: &SearchDialogState) -> Element<'_, Message> {
    let count = dialog.results.len();
    let header = row![
        text("Results").size(14).font(utility::semibold()),
        utility::badge(count.to_string()),
    ]
    .spacing(10)
    .align_y(Center);
    let body: Element<'_, Message> = if count == 0 {
        let label =
            if dialog.status.starts_with("No matches") || dialog.status.starts_with("0 matches") {
                "No matches"
            } else {
                "No results yet"
            };
        container(utility::description(label))
            .padding(16)
            .center(Fill)
            .into()
    } else {
        let mut rows = column![].spacing(2);
        let mut previous = None;
        for result in &dialog.results {
            if previous != Some(result.document_id) {
                rows = rows.push(
                    container(
                        text(&result.document_title)
                            .size(12)
                            .font(utility::semibold())
                            .wrapping(text::Wrapping::None),
                    )
                    .padding([8, 10])
                    .width(Fill)
                    .clip(true)
                    .style(styles::find_status),
                );
                previous = Some(result.document_id);
            }
            rows = rows.push(result_row(result));
        }
        scrollable(rows)
            .spacing(8)
            .smooth_scroll(true)
            .height(Fill)
            .into()
    };
    container(column![header, body].spacing(12).height(Fill))
        .padding(16)
        .height(FillPortion(2))
        .width(Fill)
        .style(styles::utility_card)
        .into()
}

fn result_row(result: &SearchResult) -> Element<'_, Message> {
    let start = result.selection.range().start;
    button(
        row![
            container(
                text(format!("{}:{}", start.line + 1, start.column + 1))
                    .size(12)
                    .font(Font::MONOSPACE)
            )
            .width(76)
            .style(styles::info_muted),
            container(
                text(&result.preview)
                    .size(13)
                    .font(Font::MONOSPACE)
                    .wrapping(text::Wrapping::None)
            )
            .width(Fill)
            .clip(true),
        ]
        .spacing(12)
        .align_y(Center),
    )
    .padding([8, 10])
    .width(Fill)
    .style(styles::menu_dropdown_item)
    .on_press(Message::AdvancedSearchResultSelected(
        result.document_id,
        result.selection,
    ))
    .into()
}

fn field<'a>(label: &'static str, control: Element<'a, Message>) -> Element<'a, Message> {
    column![text(label).size(13).font(utility::semibold()), control]
        .spacing(7)
        .into()
}

fn action<'a>(
    label: &'static str,
    message: Message,
    primary: bool,
    enabled: bool,
) -> Element<'a, Message> {
    button(text(label).size(13))
        .padding([8, 14])
        .style(if primary {
            styles::primary_command_button
        } else {
            styles::command_button
        })
        .on_press_maybe(enabled.then_some(message))
        .into()
}

fn status_label(dialog: &SearchDialogState) -> &str {
    if dialog.status == "No query" {
        "Ready to search"
    } else {
        &dialog.status
    }
}

const fn replace_mode(tab: AdvancedSearchTab) -> bool {
    matches!(
        tab,
        AdvancedSearchTab::Replace | AdvancedSearchTab::ReplaceInFiles
    )
}

const fn open_scope(tab: AdvancedSearchTab) -> bool {
    matches!(
        tab,
        AdvancedSearchTab::FindInFiles | AdvancedSearchTab::ReplaceInFiles
    )
}

const fn scope_label(tab: AdvancedSearchTab) -> &'static str {
    if open_scope(tab) {
        "Open documents"
    } else {
        "Current document"
    }
}
