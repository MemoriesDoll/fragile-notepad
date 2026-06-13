use iced::highlighter;
use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, space, text, toggler,
};
use iced::{Center, Element, Fill, Length};

use crate::core::{
    AppearanceMode, EditorSettings, HardwareAccelerationMode, IndentationMode, KeyBinding,
    ShortcutCommand, ShortcutConflict, ShortcutDisplayPart, ShortcutGroup, ShortcutModifierIcon,
};
use crate::message::{Message, SettingsCategory};
use crate::settings_dialog::SettingsDialogState;
use crate::ui::icons::hero::{self, HeroIcon, IconTone};
use crate::ui::icons::shortcut::{self, ShortcutIcon};
use crate::ui::{centered_fill_button_label, controls, styles};

const APPEARANCE_OPTIONS: &[AppearanceMode] = &[
    AppearanceMode::System,
    AppearanceMode::Light,
    AppearanceMode::Dark,
];
const INDENTATION_OPTIONS: &[IndentationMode] = &[
    IndentationMode::Tabs,
    IndentationMode::Spaces(2),
    IndentationMode::Spaces(4),
    IndentationMode::Spaces(8),
];
const BODY_TEXT_SIZE: u32 = 14;
const SECONDARY_TEXT_SIZE: u32 = 13;
const TITLE_TEXT_SIZE: u32 = 19;

pub fn view(dialog: &SettingsDialogState) -> Element<'_, Message> {
    let categories = column![
        category_button(
            "General",
            SettingsCategory::General,
            dialog.category == SettingsCategory::General,
        ),
        category_button(
            "Appearance",
            SettingsCategory::Appearance,
            dialog.category == SettingsCategory::Appearance,
        ),
        category_button(
            "Editor",
            SettingsCategory::Editor,
            dialog.category == SettingsCategory::Editor,
        ),
        category_button(
            "Shortcuts",
            SettingsCategory::Shortcuts,
            dialog.category == SettingsCategory::Shortcuts,
        ),
    ]
    .spacing(1)
    .padding([8, 0])
    .width(160);

    let pane = match dialog.category {
        SettingsCategory::General => general_pane(&dialog.draft),
        SettingsCategory::Appearance => appearance_pane(&dialog.draft),
        SettingsCategory::Editor => editor_pane(&dialog.draft),
        SettingsCategory::Shortcuts => shortcuts_pane(
            &dialog.draft,
            dialog.shortcut_group,
            dialog.capturing_shortcut,
            dialog.shortcut_conflict,
        ),
    };

    container(
        column![
            row![
                container(categories)
                    .height(Fill)
                    .style(styles::settings_category_list),
                rule::vertical(1),
                container(pane)
                    .padding([18, 22])
                    .width(Fill)
                    .height(Fill)
                    .style(styles::settings_content),
            ]
            .height(Fill),
            rule::horizontal(1),
            row![
                space::horizontal(),
                button_bar_primary("Save", Message::SaveSettings),
                button_bar_command("Apply", Message::ApplySettings),
                button_bar_command("Cancel", Message::CancelSettings),
            ]
            .spacing(8)
            .align_y(Center)
            .padding([10, 14])
            .width(Fill),
        ]
        .height(Fill)
        .width(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(styles::settings_panel)
    .into()
}

fn general_pane(settings: &EditorSettings) -> Element<'_, Message> {
    column![
        pane_title("General"),
        option_row(
            "Startup appearance",
            pick_list(
                Some(settings.appearance),
                APPEARANCE_OPTIONS,
                appearance_label,
            )
            .on_select(Message::DraftAppearanceSelected)
            .placeholder("Appearance")
            .width(220)
            .into(),
        ),
        option_row(
            "Default syntax theme",
            pick_list(
                Some(settings.syntax_theme),
                highlighter::Theme::ALL,
                highlighter::Theme::to_string,
            )
            .on_select(Message::DraftThemeSelected)
            .placeholder("Syntax theme")
            .width(220)
            .into(),
        ),
        option_row(
            "Hardware acceleration",
            pick_list(
                Some(settings.hardware_acceleration),
                HardwareAccelerationMode::ALL,
                hardware_acceleration_label,
            )
            .on_select(Message::DraftHardwareAccelerationSelected)
            .placeholder("Hardware acceleration")
            .width(220)
            .into(),
        ),
    ]
    .spacing(14)
    .into()
}

fn appearance_pane(settings: &EditorSettings) -> Element<'_, Message> {
    column![
        pane_title("Appearance"),
        option_row(
            "Color mode",
            pick_list(
                Some(settings.appearance),
                APPEARANCE_OPTIONS,
                appearance_label,
            )
            .on_select(Message::DraftAppearanceSelected)
            .placeholder("Appearance")
            .width(220)
            .into(),
        ),
        row![
            text("Editor zoom").size(BODY_TEXT_SIZE).width(180),
            controls::icon_command_button(hero_icon(HeroIcon::Minus, 16), Message::SettingsZoomOut),
            controls::value_pill(
                format!("{:.0}%", settings.zoom * 100.0),
                BODY_TEXT_SIZE,
                56.0,
            ),
            controls::icon_command_button(hero_icon(HeroIcon::Plus, 16), Message::SettingsZoomIn),
            controls::compact_command_button(
                "Reset",
                SECONDARY_TEXT_SIZE,
                Message::SettingsZoomReset,
            ),
        ]
        .spacing(8)
        .align_y(Center),
    ]
    .spacing(14)
    .into()
}

fn editor_pane(settings: &EditorSettings) -> Element<'_, Message> {
    column![
        pane_title("Editor"),
        option_row(
            "Indentation",
            pick_list(
                Some(settings.indentation),
                INDENTATION_OPTIONS,
                indentation_label,
            )
            .on_select(Message::DraftIndentationSelected)
            .placeholder("Indentation")
            .width(220)
            .into(),
        ),
        row![
            text("Word wrap").size(BODY_TEXT_SIZE).width(180),
            toggler(settings.word_wrap)
                .label("")
                .on_toggle(Message::DraftWordWrapToggled),
        ]
        .spacing(8)
        .align_y(Center),
        row![
            text("Wheel scroll speed").size(BODY_TEXT_SIZE).width(180),
            controls::icon_command_button(
                hero_icon(HeroIcon::Minus, 16),
                Message::SettingsScrollSpeedDecrease,
            ),
            controls::value_pill(
                format!("{:.2}x", settings.scroll_speed),
                BODY_TEXT_SIZE,
                56.0,
            ),
            controls::icon_command_button(
                hero_icon(HeroIcon::Plus, 16),
                Message::SettingsScrollSpeedIncrease,
            ),
            controls::compact_command_button(
                "Reset",
                SECONDARY_TEXT_SIZE,
                Message::SettingsScrollSpeedReset,
            ),
        ]
        .spacing(8)
        .align_y(Center),
        toggle_row(
            "Line numbers",
            settings.decorations.show_line_numbers,
            Message::DraftLineNumbersToggled,
        ),
        toggle_row(
            "Visible spaces",
            settings.decorations.show_spaces,
            Message::DraftVisibleSpacesToggled,
        ),
        toggle_row(
            "Visible tabs",
            settings.decorations.show_tabs,
            Message::DraftVisibleTabsToggled,
        ),
        toggle_row(
            "End of line markers",
            settings.decorations.show_end_of_line_markers,
            Message::DraftEolMarkersToggled,
        ),
        toggle_row(
            "Indentation guides",
            settings.decorations.show_indentation_guides,
            Message::DraftIndentationGuidesToggled,
        ),
        toggle_row(
            "Folding controls",
            settings.decorations.show_folding_controls,
            Message::DraftFoldingControlsToggled,
        ),
    ]
    .spacing(14)
    .into()
}

fn shortcuts_pane(
    settings: &EditorSettings,
    active_group: ShortcutGroup,
    capturing: Option<ShortcutCommand>,
    conflict: Option<ShortcutConflict>,
) -> Element<'_, Message> {
    let mut content = column![
        row![
            pane_title("Shortcuts"),
            space::horizontal(),
            controls::compact_command_button(
                "Reset",
                SECONDARY_TEXT_SIZE,
                Message::ShortcutsResetToDefaults,
            ),
        ]
        .align_y(Center)
    ]
    .spacing(12);

    let group_tabs =
        ShortcutGroup::ALL
            .iter()
            .copied()
            .fold(row![].spacing(6).width(Fill), |row, group| {
                row.push(
                    button(crate::ui::centered_button_label(
                        group.label(),
                        SECONDARY_TEXT_SIZE,
                    ))
                    .padding([5, 10])
                    .style(styles::settings_category_button(group == active_group))
                    .on_press(Message::ShortcutGroupSelected(group)),
                )
            });

    content = content.push(group_tabs);

    if let Some(conflict) = conflict {
        content = content.push(
            container(
                row![
                    text(format!(
                        "{} is already assigned to {}",
                        conflict.binding.display(),
                        conflict.command.label()
                    ))
                    .size(SECONDARY_TEXT_SIZE)
                    .width(Fill),
                    controls::command_button(
                        "Dismiss",
                        SECONDARY_TEXT_SIZE,
                        Message::ShortcutConflictDismissed,
                    ),
                ]
                .spacing(8)
                .align_y(Center),
            )
            .padding([8, 10])
            .style(styles::settings_category_list),
        );
    }

    let rows = ShortcutCommand::ALL
        .iter()
        .copied()
        .filter(|command| command.group() == active_group)
        .fold(column![].spacing(6), |column, command| {
            column.push(shortcut_row(settings, command, capturing))
        });

    content = content.push(rows);

    scrollable(content.spacing(16)).height(Fill).into()
}

fn shortcut_row<'a>(
    settings: &'a EditorSettings,
    command: ShortcutCommand,
    capturing: Option<ShortcutCommand>,
) -> Element<'a, Message> {
    let is_capturing = capturing == Some(command);
    let binding = settings.shortcuts.binding(command);

    let action_label = if is_capturing { "..." } else { "Set" };

    row![
        text(command.label()).size(BODY_TEXT_SIZE).width(Fill),
        container(shortcut_binding_view(binding, is_capturing))
            .width(Length::Fixed(138.0))
            .padding([5, 8])
            .style(styles::settings_panel),
        shortcut_set_button(
            action_label,
            is_capturing,
            Message::ShortcutCaptureStarted(command),
        ),
        controls::fixed_fill_command_button(
            "Clr",
            SECONDARY_TEXT_SIZE,
            44.0,
            Message::ShortcutCleared(command),
        ),
    ]
    .spacing(6)
    .align_y(Center)
    .height(Length::Fixed(32.0))
    .into()
}

fn shortcut_binding_view<'a>(
    binding: Option<KeyBinding>,
    is_capturing: bool,
) -> Element<'a, Message> {
    if is_capturing {
        return text("Press shortcut").size(SECONDARY_TEXT_SIZE).into();
    }

    let Some(binding) = binding else {
        return text("Unassigned").size(SECONDARY_TEXT_SIZE).into();
    };

    let display = binding.display_parts();
    let mut parts = row![].spacing(3).align_y(Center);

    for modifier in display.modifiers {
        parts = parts.push(shortcut_display_part(modifier));
    }

    parts = parts.push(text(display.key).size(SECONDARY_TEXT_SIZE));
    parts.into()
}

fn shortcut_display_part<'a>(part: ShortcutDisplayPart) -> Element<'a, Message> {
    match part {
        ShortcutDisplayPart::Text(label) => text(label).size(SECONDARY_TEXT_SIZE).into(),
        ShortcutDisplayPart::Icon(icon) => shortcut_modifier_icon(icon),
    }
}

fn shortcut_modifier_icon<'a>(icon: ShortcutModifierIcon) -> Element<'a, Message> {
    let icon = match icon {
        ShortcutModifierIcon::Command => ShortcutIcon::Command,
        ShortcutModifierIcon::Option => ShortcutIcon::Option,
        ShortcutModifierIcon::Shift => ShortcutIcon::Shift,
        ShortcutModifierIcon::Windows => ShortcutIcon::Windows,
    };

    shortcut::icon_with_color(icon, 14, styles::shortcut_text_color)
}

fn category_button<'a>(
    label: &'static str,
    category: SettingsCategory,
    is_active: bool,
) -> Element<'a, Message> {
    controls::category_button(
        label,
        BODY_TEXT_SIZE,
        is_active,
        Message::SettingsCategorySelected(category),
    )
}

fn pane_title<'a>(label: &'static str) -> Element<'a, Message> {
    text(label).size(TITLE_TEXT_SIZE).width(Fill).into()
}

fn option_row<'a>(label: &'static str, control: Element<'a, Message>) -> Element<'a, Message> {
    row![text(label).size(BODY_TEXT_SIZE).width(180), control]
        .spacing(8)
        .align_y(Center)
        .height(Length::Fixed(32.0))
        .into()
}

fn toggle_row<'a>(
    label: &'static str,
    enabled: bool,
    message: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).size(BODY_TEXT_SIZE).width(180),
        toggler(enabled).label("").on_toggle(message),
    ]
    .spacing(8)
    .align_y(Center)
    .height(Length::Fixed(32.0))
    .into()
}

fn hero_icon<'a>(icon: HeroIcon, size: u32) -> Element<'a, Message> {
    hero::icon(icon, size, IconTone::Text)
}

fn button_bar_primary<'a>(label: &'static str, message: Message) -> Element<'a, Message> {
    button(crate::ui::centered_button_label(label, SECONDARY_TEXT_SIZE))
        .padding([6, 18])
        .style(styles::primary_command_button)
        .on_press(message)
        .into()
}

fn button_bar_command<'a>(label: &'static str, message: Message) -> Element<'a, Message> {
    button(crate::ui::centered_button_label(label, SECONDARY_TEXT_SIZE))
        .padding([6, 18])
        .style(styles::command_button)
        .on_press(message)
        .into()
}

fn shortcut_set_button<'a>(
    label: &'static str,
    is_capturing: bool,
    message: Message,
) -> Element<'a, Message> {
    button(centered_fill_button_label(label, SECONDARY_TEXT_SIZE))
        .width(44)
        .padding([5, 0])
        .style(if is_capturing {
            styles::primary_command_button
        } else {
            styles::command_button
        })
        .on_press(message)
        .into()
}

fn appearance_label(appearance: &AppearanceMode) -> String {
    match appearance {
        AppearanceMode::System => String::from("System"),
        AppearanceMode::Light => String::from("Light"),
        AppearanceMode::Dark => String::from("Dark"),
    }
}

fn hardware_acceleration_label(mode: &HardwareAccelerationMode) -> String {
    mode.label().to_owned()
}

fn indentation_label(indentation: &IndentationMode) -> String {
    match indentation {
        IndentationMode::Tabs => String::from("Tabs"),
        IndentationMode::Spaces(width) => format!("{} spaces", width),
    }
}
