//! Declarative UI modules.

pub mod about_dialog;
pub mod advanced_search_panel;
pub mod controls;
pub mod dirty_close_dialog;
pub mod dropdown;
pub mod editor;
pub mod find_panel;
pub mod function_list_panel;
pub mod icons;
pub mod menu;
pub mod motion;
pub mod settings_panel;
pub mod status_bar;
pub mod styles;
pub mod tabs;
pub mod toolbar;
pub mod window_list_dialog;

use iced::widget::{column, container, row, stack, text};
use iced::{Element, Fill, Length};

use crate::core::{Document, DocumentId, EditorSettings, FindState, Workspace};
use crate::editor::OutlineState;
use crate::message::{AboutTab, Menu, Message};
use crate::ui::toolbar::WindowMenuState;
use crate::ui::window_list_dialog::WindowListEntry;

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

pub fn view<'a>(
    workspace: &'a Workspace,
    find: &'a FindState,
    settings: &'a EditorSettings,
    is_find_visible: bool,
    is_inline_replace_visible: bool,
    is_function_list_visible: bool,
    chrome_animation: ChromeAnimationInfo,
    active_menu: Option<Menu>,
    active_menu_path: &'a [String],
    window_menu_state: WindowMenuState,
    dragged_tab: Option<DocumentId>,
    hovered_drop_tab: Option<DocumentId>,
    dirty_close_document: Option<&'a Document>,
    about_tab: Option<AboutTab>,
    rendering_debug_info: about_dialog::RenderingDebugInfo,
    window_list_entries: Option<Vec<WindowListEntry>>,
    file_status: Option<&'a str>,
    active_outline_state: Option<&'a OutlineState>,
) -> Element<'a, Message> {
    let active_document = workspace.active_document();
    let editor = if let Some(document) = active_document {
        editor::view(document, settings)
    } else {
        editor::empty()
    };

    let mut workbench = column![
        toolbar::menu_bar(active_menu),
        toolbar::tool_bar(),
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
                settings
            ))
            .width(Fill)
            .height(Fill),
        ]
        .into()
    } else {
        shell.into()
    };

    let with_dialogs = if let Some(document) = dirty_close_document {
        stack![with_menu, dirty_close_dialog::view(document)].into()
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
            about_dialog::view(tab, rendering_debug_info)
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
