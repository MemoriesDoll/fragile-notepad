use iced::advanced::text::highlighter::Highlighter as _;
use iced::highlighter;
use iced::widget::{
    button, column, container, keyed_column, rich_text, row, rule, scrollable, space, span, text,
    toggler,
};
use iced::{Center, Color, Element, Fill, Font};

use crate::core::{
    AppearanceMode, EditorSettings, HardwareAccelerationMode, IndentationMode, KeyBinding,
    ShortcutCommand, ShortcutDisplayPart, ShortcutGroup, ShortcutModifierIcon,
};
use crate::message::{Message, SettingsCategory};
use crate::settings_dialog::{SettingsDialogState, ShortcutNoticeKind};
use crate::ui::dropdown::dropdown;
use crate::ui::icons::hero::{self, HeroIcon, IconTone};
use crate::ui::icons::shortcut::{self, ShortcutIcon};
use crate::ui::{controls, motion, styles, utility};

const INDENTATION_OPTIONS: &[IndentationMode] = &[
    IndentationMode::Tabs,
    IndentationMode::Spaces(2),
    IndentationMode::Spaces(4),
    IndentationMode::Spaces(8),
];
const SHORTCUT_STATUS_HEIGHT: f32 = 42.0;

pub fn view(dialog: &SettingsDialogState) -> Element<'_, Message> {
    let title = match dialog.category {
        SettingsCategory::General => "General",
        SettingsCategory::Appearance => "Appearance",
        SettingsCategory::Editor => "Editor",
        SettingsCategory::Shortcuts => "Keyboard shortcuts",
    };
    let pane = match dialog.category {
        SettingsCategory::General => general_pane(&dialog.draft),
        SettingsCategory::Appearance => appearance_pane(&dialog.draft),
        SettingsCategory::Editor => editor_pane(&dialog.draft),
        SettingsCategory::Shortcuts => shortcuts_pane(dialog),
    };
    let pane: Element<'_, Message> = if dialog.category == SettingsCategory::Shortcuts {
        pane
    } else {
        scrollable(pane)
            .smooth_scroll(true)
            .spacing(6)
            .height(Fill)
            .into()
    };
    let sidebar = container(
        column![
            category("General", SettingsCategory::General, dialog.category),
            category("Appearance", SettingsCategory::Appearance, dialog.category),
            category("Editor", SettingsCategory::Editor, dialog.category),
            category("Shortcuts", SettingsCategory::Shortcuts, dialog.category),
            space::vertical(),
        ]
        .spacing(2)
        .height(Fill),
    )
    .padding([16, 10])
    .width(156)
    .height(Fill)
    .style(styles::settings_category_list);

    container(
        column![
            row![
                sidebar,
                container(
                    column![
                        utility::heading(title),
                        // A new category starts at the top; redraws within a page retain scrolling.
                        keyed_column![(dialog.category, pane)]
                            .height(Fill)
                            .width(Fill),
                    ]
                    .spacing(14)
                    .height(Fill)
                )
                .padding(18)
                .width(Fill)
                .height(Fill)
            ]
            .height(Fill),
            rule::horizontal(1).style(styles::utility_rule),
            row![
                space::horizontal(),
                footer_button("Cancel", Message::CancelSettings, false),
                footer_button("Apply", Message::ApplySettings, false),
                footer_button("Save", Message::SaveSettings, true),
            ]
            .spacing(8)
            .align_y(Center)
            .padding([8, 16]),
        ]
        .width(Fill)
        .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(styles::settings_panel)
    .into()
}

fn category(
    label: &'static str,
    category: SettingsCategory,
    active: SettingsCategory,
) -> Element<'static, Message> {
    utility::navigation(
        label,
        category == active,
        Message::SettingsCategorySelected(category),
    )
}

fn general_pane(settings: &EditorSettings) -> Element<'_, Message> {
    let modes = [
        (
            HardwareAccelerationMode::Off,
            "Software",
            "Render without graphics acceleration.",
        ),
        (
            HardwareAccelerationMode::Lazy,
            "Hybrid",
            "Use graphics hardware when available.",
        ),
        (
            HardwareAccelerationMode::Diagnostic,
            "Diagnostic",
            "Request hardware rendering for troubleshooting.",
        ),
    ]
    .into_iter()
    .fold(row![].spacing(8), |row, (mode, title, hint)| {
        row.push(
            button(
                column![
                    text(title).size(14).font(utility::semibold()),
                    utility::description(hint),
                    space::vertical(),
                    text(if settings.hardware_acceleration == mode {
                        "Selected"
                    } else {
                        ""
                    })
                    .size(11),
                ]
                .spacing(6)
                .height(76),
            )
            .padding(10)
            .width(Fill)
            .style(styles::utility_selection(
                settings.hardware_acceleration == mode,
            ))
            .on_press(Message::DraftHardwareAccelerationSelected(mode)),
        )
    });
    column![
        section("Rendering", column![
            modes,
            utility::description("If hardware rendering is already active, switching to Software takes effect after restarting."),
        ].spacing(10).into()),
        section("Scrolling", setting_row(
            "Editor scroll speed",
            stepper(format!("{:.2}×", settings.scroll_speed), Message::SettingsScrollSpeedDecrease,
                Message::SettingsScrollSpeedIncrease, Message::SettingsScrollSpeedReset,
                settings.scroll_speed > EditorSettings::MIN_SCROLL_SPEED, settings.scroll_speed < EditorSettings::MAX_SCROLL_SPEED),
        )),
    ].spacing(14).into()
}

fn appearance_pane(settings: &EditorSettings) -> Element<'_, Message> {
    let modes = [
        AppearanceMode::System,
        AppearanceMode::Light,
        AppearanceMode::Dark,
    ]
    .into_iter()
    .fold(row![].spacing(8), |row, mode| {
        row.push(appearance_choice(mode, settings.appearance == mode))
    });
    column![
        section("Color mode", modes.into()),
        section(
            "Text & syntax",
            column![
                setting_row(
                    "Syntax theme",
                    dropdown(
                        Some(settings.syntax_theme),
                        highlighter::Theme::ALL,
                        highlighter::Theme::to_string,
                        Message::DraftThemeSelected,
                    )
                    .width(210)
                    .into()
                ),
                rule::horizontal(1).style(styles::utility_rule),
                setting_row(
                    "Editor zoom",
                    stepper(
                        format!("{:.0}%", settings.zoom * 100.0),
                        Message::SettingsZoomOut,
                        Message::SettingsZoomIn,
                        Message::SettingsZoomReset,
                        settings.zoom > EditorSettings::MIN_ZOOM,
                        settings.zoom < EditorSettings::MAX_ZOOM,
                    )
                ),
                syntax_preview(settings),
            ]
            .spacing(10)
            .into()
        ),
    ]
    .spacing(14)
    .into()
}

fn appearance_choice(mode: AppearanceMode, selected: bool) -> Element<'static, Message> {
    let label = match mode {
        AppearanceMode::System => "System",
        AppearanceMode::Light => "Light",
        AppearanceMode::Dark => "Dark",
    };
    let preview: Element<'static, Message> = if mode == AppearanceMode::System {
        row![miniature(false), miniature(true)].spacing(1).into()
    } else {
        miniature(mode == AppearanceMode::Dark)
    };
    button(
        column![
            preview,
            row![
                text(label).size(13).font(utility::semibold()),
                space::horizontal(),
                text(if selected { "Selected" } else { "" }).size(10)
            ]
            .align_y(Center),
        ]
        .spacing(8),
    )
    .padding(8)
    .width(Fill)
    .style(styles::utility_selection(selected))
    .on_press(Message::DraftAppearanceSelected(mode))
    .into()
}

/// A small, code-native window illustration; both color modes remain visible in any theme.
fn miniature(dark: bool) -> Element<'static, Message> {
    let surface = if dark {
        Color::from_rgb8(27, 31, 38)
    } else {
        Color::from_rgb8(250, 251, 253)
    };
    let chrome = if dark {
        Color::from_rgb8(51, 58, 70)
    } else {
        Color::from_rgb8(225, 231, 240)
    };
    let ink = if dark {
        Color::from_rgb8(123, 167, 217)
    } else {
        Color::from_rgb8(101, 142, 191)
    };
    let line = move |width| {
        container(space::horizontal())
            .width(width)
            .height(3)
            .style(move |_| container::Style {
                background: Some(ink.into()),
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
    };
    container(column![
        container(space::horizontal())
            .height(11)
            .width(Fill)
            .style(move |_| container::Style {
                background: Some(chrome.into()),
                ..Default::default()
            }),
        row![
            container(space::horizontal())
                .width(14)
                .height(Fill)
                .style(move |_| container::Style {
                    background: Some(chrome.into()),
                    ..Default::default()
                }),
            column![line(30), line(20), line(27)]
                .spacing(6)
                .padding(10)
                .width(Fill),
        ]
        .height(Fill),
    ])
    .height(52)
    .width(Fill)
    .clip(true)
    .style(move |_| container::Style {
        background: Some(surface.into()),
        border: iced::Border {
            color: chrome,
            width: 1.0,
            radius: 5.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn syntax_preview(settings: &EditorSettings) -> Element<'_, Message> {
    let mut highlighter = highlighter::Highlighter::new(&highlighter::Settings {
        token: "rs".into(),
        theme: settings.syntax_theme,
    });
    let mut lines = column![].spacing(3);
    for (index, line) in [
        "fn main() {",
        "    let message = \"Hello, world!\";",
        "    println!(\"{message}\");",
        "}",
    ]
    .into_iter()
    .enumerate()
    {
        let spans: Vec<iced::widget::text::Span<'_, (), Font>> = highlighter
            .highlight_line(line)
            .map(|(range, highlight)| {
                let mut part = span(&line[range]).font(highlight.font().unwrap_or(Font::MONOSPACE));
                if let Some(color) = highlight.color() {
                    part = part.color(color);
                }
                part
            })
            .collect();
        let mut line_row = row![].spacing(16).align_y(Center);
        if settings.decorations.show_line_numbers {
            line_row = line_row.push(
                container(text((index + 1).to_string()).size(13).font(Font::MONOSPACE))
                    .width(20)
                    .style(styles::info_muted),
            );
        }
        lines = lines.push(
            line_row.push(
                rich_text(spans)
                    .font(Font::MONOSPACE)
                    .size(16.0 * settings.zoom)
                    .wrapping(text::Wrapping::None),
            ),
        );
    }
    let preview = container(column![
        container(utility::description("Preview")).padding([8, 12]),
        rule::horizontal(1).style(styles::utility_rule),
        scrollable(container(lines).padding(10))
            .direction(scrollable::Direction::Both {
                vertical: scrollable::Scrollbar::default(),
                horizontal: scrollable::Scrollbar::default(),
            })
            .height(112)
            .width(Fill),
    ])
    .width(Fill)
    .style(styles::utility_card);
    iced::widget::themer(styles::modern_theme(settings.appearance), preview).into()
}

fn editor_pane(settings: &EditorSettings) -> Element<'_, Message> {
    column![
        section(
            "Typing & layout",
            column![
                setting_row(
                    "Indentation",
                    dropdown(
                        Some(settings.indentation),
                        INDENTATION_OPTIONS,
                        indentation_label,
                        Message::DraftIndentationSelected
                    )
                    .width(190)
                    .into()
                ),
                rule::horizontal(1).style(styles::utility_rule),
                column![
                    toggle_row(
                        "Word wrap",
                        settings.word_wrap,
                        Message::DraftWordWrapToggled
                    ),
                    utility::description("Wrap at the window edge without inserting line breaks."),
                ]
                .spacing(5),
            ]
            .spacing(10)
            .into()
        ),
        section(
            "Gutter & structure",
            column![
                toggle_row(
                    "Line numbers",
                    settings.decorations.show_line_numbers,
                    Message::DraftLineNumbersToggled
                ),
                rule::horizontal(1).style(styles::utility_rule),
                toggle_row(
                    "Indentation guides",
                    settings.decorations.show_indentation_guides,
                    Message::DraftIndentationGuidesToggled
                ),
                rule::horizontal(1).style(styles::utility_rule),
                toggle_row(
                    "Folding controls",
                    settings.decorations.show_folding_controls,
                    Message::DraftFoldingControlsToggled
                ),
            ]
            .spacing(10)
            .into()
        ),
        section(
            "Whitespace",
            column![
                utility::description("Show markers without changing file contents."),
                toggle_row(
                    "Spaces",
                    settings.decorations.show_spaces,
                    Message::DraftVisibleSpacesToggled
                ),
                rule::horizontal(1).style(styles::utility_rule),
                toggle_row(
                    "Tabs",
                    settings.decorations.show_tabs,
                    Message::DraftVisibleTabsToggled
                ),
                rule::horizontal(1).style(styles::utility_rule),
                toggle_row(
                    "Line endings",
                    settings.decorations.show_end_of_line_markers,
                    Message::DraftEolMarkersToggled
                ),
            ]
            .spacing(10)
            .into()
        ),
    ]
    .spacing(14)
    .into()
}

fn shortcuts_pane(dialog: &SettingsDialogState) -> Element<'_, Message> {
    let groups = ShortcutGroup::ALL
        .into_iter()
        .fold(row![].spacing(4), |row, group| {
            row.push(
                button(text(group.label()).size(13))
                    .padding([6, 11])
                    .style(styles::settings_category_button(
                        group == dialog.shortcut_group,
                    ))
                    .on_press(Message::ShortcutGroupSelected(group)),
            )
        });
    let commands: Vec<_> = ShortcutCommand::ALL
        .into_iter()
        .filter(|command| command.group() == dialog.shortcut_group)
        .collect();
    let pane = column![
        shortcut_status(dialog),
        row![
            groups,
            space::horizontal(),
            button(text("Restore all defaults").size(12))
                .padding([6, 9])
                .style(styles::command_button)
                .on_press(Message::ShortcutsResetToDefaults)
        ]
        .spacing(8)
        .align_y(Center),
    ]
    .spacing(10);
    let mut rows = column![].spacing(0);
    for (index, command) in commands.into_iter().enumerate() {
        if index > 0 {
            rows = rows.push(rule::horizontal(1).style(styles::utility_rule));
        }
        rows = rows.push(shortcut_row(
            &dialog.draft,
            command,
            dialog.capturing_shortcut,
            dialog.shortcut_notice_animation.pulse(),
        ));
    }
    pane.push(
        keyed_column![(
            dialog.shortcut_group,
            scrollable(container(rows).style(styles::utility_card))
                .spacing(6)
                .smooth_scroll(true)
                .height(Fill)
        )]
        .height(Fill),
    )
    .height(Fill)
    .into()
}

fn shortcut_status(dialog: &SettingsDialogState) -> Element<'_, Message> {
    let animation = dialog.shortcut_notice_animation;
    let fallback_current = animation.rendered() == ShortcutNoticeKind::None
        && (dialog.capturing_shortcut.is_some() || dialog.shortcut_conflict.is_some());
    let kind = match animation.rendered() {
        ShortcutNoticeKind::None if dialog.capturing_shortcut.is_some() => {
            ShortcutNoticeKind::Listening(dialog.capturing_shortcut.unwrap())
        }
        ShortcutNoticeKind::None if dialog.shortcut_conflict.is_some() => {
            ShortcutNoticeKind::Conflict(dialog.shortcut_conflict.unwrap())
        }
        kind => kind,
    };

    let content: Element<'_, Message> = match kind {
        ShortcutNoticeKind::None => container(utility::description(
            "Click a binding, then press the new shortcut.",
        ))
        .width(Fill)
        .center_y(SHORTCUT_STATUS_HEIGHT)
        .into(),
        ShortcutNoticeKind::Listening(command) => container(
            row![
                text("Listening").size(12).font(utility::semibold()),
                text(format!("Press a shortcut for {}", command.label()))
                    .size(12)
                    .width(Fill),
                button(text("Cancel").size(12))
                    .padding([6, 9])
                    .style(styles::command_button)
                    .on_press(Message::ShortcutGroupSelected(dialog.shortcut_group)),
            ]
            .spacing(10)
            .align_y(Center),
        )
        .padding([8, 10])
        .width(Fill)
        .height(SHORTCUT_STATUS_HEIGHT)
        .style(styles::listening_notice)
        .into(),
        ShortcutNoticeKind::Conflict(conflict) => container(
            row![
                text("Conflict").size(12).font(utility::semibold()),
                text(format!(
                    "{} is already assigned to {}.",
                    conflict.binding.display(),
                    conflict.command.label()
                ))
                .size(12)
                .width(Fill),
                button(text("Dismiss").size(12))
                    .padding([6, 9])
                    .style(styles::command_button)
                    .on_press(Message::ShortcutConflictDismissed),
            ]
            .spacing(10)
            .align_y(Center),
        )
        .padding([8, 10])
        .width(Fill)
        .height(SHORTCUT_STATUS_HEIGHT)
        .style(styles::utility_notice)
        .into(),
    };

    let opacity = if fallback_current {
        1.0
    } else {
        animation.opacity()
    };
    let content = if kind == ShortcutNoticeKind::None {
        content
    } else {
        // Keep the notice in its fixed slot while its paint fades as a group.
        // The slot itself remains mounted, so the table never moves.
        motion::fade(content, opacity, styles::settings_panel_background, true)
    };

    container(content)
        .width(Fill)
        .height(SHORTCUT_STATUS_HEIGHT)
        .clip(true)
        .into()
}

fn shortcut_row(
    settings: &EditorSettings,
    command: ShortcutCommand,
    capturing: Option<ShortcutCommand>,
    pulse: f32,
) -> Element<'_, Message> {
    let recording = capturing == Some(command);
    let binding = settings.shortcuts.binding(command);
    let binding_view: Element<'_, Message> = if recording {
        text("Press keys…").size(12).into()
    } else {
        shortcut_binding_view(binding)
    };
    let binding_button: Element<'_, Message> = if recording {
        button(container(binding_view).center_x(Fill))
            .padding([6, 9])
            .width(174)
            .style(styles::listening_command_button(pulse))
            .on_press(Message::ShortcutCaptureStarted(command))
            .into()
    } else {
        button(container(binding_view).center_x(Fill))
            .padding([6, 9])
            .width(174)
            .style(styles::command_button)
            .on_press(Message::ShortcutCaptureStarted(command))
            .into()
    };
    container(
        row![
            text(command.label()).size(13).width(Fill),
            binding_button,
            button(text("Clear").size(12))
                .padding([6, 5])
                .style(styles::text_button)
                .on_press_maybe(binding.map(|_| Message::ShortcutCleared(command))),
        ]
        .spacing(8)
        .align_y(Center),
    )
    .padding([8, 12])
    .width(Fill)
    .into()
}

fn shortcut_binding_view(binding: Option<KeyBinding>) -> Element<'static, Message> {
    let Some(binding) = binding else {
        return utility::description("Assign shortcut");
    };
    let display = binding.display_parts();
    let mut parts = row![].spacing(4).align_y(Center);
    for modifier in display.modifiers {
        let part: Element<'_, Message> = match modifier {
            ShortcutDisplayPart::Text(label) => text(label).size(12).into(),
            ShortcutDisplayPart::Icon(icon) => shortcut::icon_with_color(
                match icon {
                    ShortcutModifierIcon::Command => ShortcutIcon::Command,
                    ShortcutModifierIcon::Option => ShortcutIcon::Option,
                    ShortcutModifierIcon::Shift => ShortcutIcon::Shift,
                    ShortcutModifierIcon::Windows => ShortcutIcon::Windows,
                },
                14,
                styles::shortcut_text_color,
            ),
        };
        parts = parts.push(part);
    }
    parts
        .push(text(display.key).size(12).font(utility::semibold()))
        .into()
}

fn section<'a>(title: &'static str, content: Element<'a, Message>) -> Element<'a, Message> {
    container(column![text(title).size(15).font(utility::semibold()), content,].spacing(12))
        .padding(12)
        .width(Fill)
        .style(styles::utility_card)
        .into()
}

fn setting_row<'a>(title: &'static str, control: Element<'a, Message>) -> Element<'a, Message> {
    row![
        text(title).size(13).font(utility::semibold()).width(Fill),
        control
    ]
    .spacing(12)
    .align_y(Center)
    .into()
}

fn toggle_row<'a>(
    title: &'static str,
    enabled: bool,
    message: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    setting_row(title, toggler(enabled).size(20).on_toggle(message).into())
}

fn stepper<'a>(
    value: String,
    decrease: Message,
    increase: Message,
    reset: Message,
    can_decrease: bool,
    can_increase: bool,
) -> Element<'a, Message> {
    let icon = |icon| hero::icon(icon, 14, IconTone::Text);
    row![
        button(icon(HeroIcon::Minus))
            .padding(6)
            .style(styles::command_button)
            .on_press_maybe(can_decrease.then_some(decrease)),
        container(text(value).size(13).font(utility::semibold())).center_x(58),
        button(icon(HeroIcon::Plus))
            .padding(6)
            .style(styles::command_button)
            .on_press_maybe(can_increase.then_some(increase)),
        controls::compact_command_button("Reset", 12, reset),
    ]
    .spacing(4)
    .align_y(Center)
    .into()
}

fn footer_button(
    label: &'static str,
    message: Message,
    primary: bool,
) -> Element<'static, Message> {
    button(container(text(label).size(13)).center_x(54))
        .padding([7, 10])
        .style(if primary {
            styles::primary_command_button
        } else {
            styles::command_button
        })
        .on_press(message)
        .into()
}

fn indentation_label(indentation: &IndentationMode) -> String {
    match indentation {
        IndentationMode::Tabs => "Tabs".into(),
        IndentationMode::Spaces(width) => format!("{width} spaces"),
    }
}
