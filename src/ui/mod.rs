//! Declarative UI modules.

pub mod about_dialog;
pub mod advanced_search_panel;
pub mod controls;
pub mod dirty_close_dialog;
pub mod dropdown;
pub mod editor;
mod editor_context_menu;
pub mod find_panel;
pub mod function_list_panel;
pub mod icons;
pub mod info_vfx;
pub mod menu;
pub mod motion;
pub mod settings_panel;
pub mod status_bar;
pub mod styles;
pub mod tabs;
pub mod title_bar;
pub mod toolbar;
mod view_model;
pub mod window_list_dialog;
pub use view_model::WorkbenchView;

use iced::widget::{column, container, row, stack, text};
use iced::{Element, Fill, Length};

use crate::message::Message;

const FIND_PANEL_COLLAPSED_HEIGHT: f32 = 46.0;
const FIND_PANEL_EXPANDED_HEIGHT: f32 = 86.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeAnimationInfo {
    pub find_rendered_visible: bool,
    pub find_progress: f32,
    pub inline_replace_rendered_visible: bool,
    pub inline_replace_progress: f32,
    pub function_list_rendered_visible: bool,
    pub function_list_progress: f32,
    pub about_rendered_visible: bool,
    pub about_progress: f32,
    pub about_interactive: bool,
    pub dirty_close_progress: f32,
    pub dirty_close_interactive: bool,
}

pub fn centered_button_content<'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content).center(Fill).into()
}

pub fn centered_button_label<'a>(label: &'a str, size: u32) -> Element<'a, Message> {
    text(label).size(size).into()
}

pub fn centered_fill_button_label<'a>(label: &'a str, size: u32) -> Element<'a, Message> {
    container(text(label).size(size)).center_x(Fill).into()
}

pub fn view<'a>(model: WorkbenchView<'a>) -> Element<'a, Message> {
    let WorkbenchView {
        workspace,
        find,
        settings,
        is_find_visible,
        is_inline_replace_visible,
        is_function_list_visible,
        chrome_animation,
        active_menu,
        active_menu_path,
        window_menu_state,
        dragged_tab,
        hovered_drop_tab,
        dirty_close_document,
        about_tab,
        rendering_debug_info,
        window_list_entries,
        file_status,
        active_outline_state,
    } = model;
    let active_document = workspace.active_document();
    let editor = if let Some(document) = active_document {
        editor::view(document, settings)
    } else {
        editor::empty()
    };

    let mut workbench = column![
        toolbar::menu_bar(active_menu),
        toolbar::tool_bar(active_document),
        tabs::view(workspace, dragged_tab, hovered_drop_tab),
    ];

    if chrome_animation.find_rendered_visible || is_find_visible {
        let find_height = animated_find_height(chrome_animation);

        workbench = workbench.push(
            container(motion::fade(
                find_panel::view(
                    find,
                    is_inline_replace_visible,
                    chrome_animation.inline_replace_rendered_visible,
                    chrome_animation.inline_replace_progress,
                ),
                chrome_animation.find_progress,
                styles::utility_bar_background,
                is_find_visible,
            ))
            .height(Length::Fixed(find_height))
            .width(Fill)
            .clip(true),
        );
    }

    let editor_surface = container(editor)
        .height(Fill)
        .width(Fill)
        .style(styles::editor_surface);

    let main_area: Element<'a, Message> =
        if chrome_animation.function_list_rendered_visible || is_function_list_visible {
            if let Some(document) = active_document {
                let function_list_width = function_list_panel::FUNCTION_LIST_PANEL_WIDTH
                    * chrome_animation.function_list_progress.clamp(0.0, 1.0);

                row![
                    editor_surface,
                    container(motion::fade(
                        function_list_panel::view(document, active_outline_state),
                        chrome_animation.function_list_progress,
                        styles::editor_background,
                        is_function_list_visible,
                    ))
                    .width(Length::Fixed(function_list_width))
                    .height(Fill)
                    .clip(true),
                ]
                .height(Fill)
                .width(Fill)
                .into()
            } else {
                editor_surface.into()
            }
        } else {
            editor_surface.into()
        };

    let shell = container(workbench.push(main_area).push(status_bar::view(
        active_document,
        settings,
        file_status,
    )))
    .height(Fill)
    .width(Fill)
    .style(styles::app_shell);

    let with_menu: Element<'a, Message> = if active_menu.is_some() {
        stack![
            shell,
            container(toolbar::menu_overlay(
                active_menu,
                active_menu_path,
                window_menu_state,
                settings,
                active_document
            ))
            .width(Fill)
            .height(Fill),
        ]
        .into()
    } else {
        shell.into()
    };

    let with_dialogs = if let Some(document) = dirty_close_document {
        stack![
            motion::fade(with_menu, 1.0, styles::editor_background, false),
            dirty_close_dialog::view(
                document,
                chrome_animation.dirty_close_progress,
                chrome_animation.dirty_close_interactive,
            )
        ]
        .into()
    } else {
        with_menu
    };

    let with_window_list = if let Some(entries) = window_list_entries {
        stack![with_dialogs, window_list_dialog::view(entries)].into()
    } else {
        with_dialogs
    };

    if let Some(tab) = about_tab {
        stack![
            with_window_list,
            about_dialog::view(
                tab,
                rendering_debug_info,
                chrome_animation.about_progress,
                chrome_animation.about_interactive,
            )
        ]
        .into()
    } else {
        with_window_list
    }
}

fn animated_find_height(animation: ChromeAnimationInfo) -> f32 {
    let find_progress = animation.find_progress.clamp(0.0, 1.0);
    let replace_progress = animation.inline_replace_progress.clamp(0.0, 1.0);
    let expanded_height = FIND_PANEL_COLLAPSED_HEIGHT
        + ((FIND_PANEL_EXPANDED_HEIGHT - FIND_PANEL_COLLAPSED_HEIGHT) * replace_progress);

    expanded_height * find_progress
}
