//! Projects application state into window views and captions.

use super::{
    App, rendering,
    windowing::{ManagedWindow, Title},
};
use crate::{message::Message, ui};
use iced::{Element, Theme, window};

impl App {
    pub fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        let perf_span = crate::perf_trace::span("app_view", format_args!("window={window_id:?}"));

        crate::startup::report_first_view_ready();

        let element = if let Some(settings_window) = &self.settings_window
            && settings_window.is(window_id)
        {
            settings_window.view(&self.settings_dialog)
        } else if let Some(search_window) = &self.advanced_search_window
            && search_window.is(window_id)
        {
            search_window.view(&self.search_dialog)
        } else {
            ui::view(ui::WorkbenchView {
                workspace: &self.workspace,
                find: &self.find,
                settings: &self.settings,
                is_find_visible: self.is_find_visible,
                is_inline_replace_visible: self.is_inline_replace_visible,
                is_function_list_visible: self.is_function_list_visible,
                function_list_query: &self.function_list_query,
                chrome_animation: self.chrome_animation_info(),
                active_menu: self.menu.active(),
                active_menu_path: self.menu.path(),
                window_menu_state: self.window_menu_state(),
                dragged_tab: self.dragged_tab,
                hovered_drop_tab: self.hovered_drop_tab,
                dirty_close_document: self
                    .close_prompt
                    .document()
                    .and_then(|id| self.workspace.document(id)),
                about_tab: self
                    .chrome_animation
                    .about
                    .rendered_visible()
                    .then_some(self.about_tab),
                rendering_debug_info: ui::about_dialog::RenderingDebugInfo {
                    title_bar_style: self.title_bar_style,
                    current_renderer: self.rendering.label(),
                    rendering_policy: rendering::current_policy_label(&self.settings),
                },
                window_list_entries: self
                    .is_window_list_visible
                    .then(|| self.window_list_entries()),
                file_status: self.file_status.as_deref(),
                active_outline_state: self.active_outline_state(),
            })
        };
        let element = if self.main_window_id == Some(window_id)
            && let Some(prompt) = &self.go_to_line_prompt
        {
            iced::widget::stack![
                ui::motion::fade(element, 1.0, ui::styles::editor_background, false),
                ui::go_to_line_prompt::view(
                    &prompt.input,
                    prompt.error.as_deref(),
                    prompt.animation.progress(),
                    prompt.animation.target_visible(),
                ),
            ]
            .into()
        } else {
            element
        };
        if let Some(span) = perf_span {
            span.end_with("");
        }

        if ui::title_bar::SUPPORTED {
            ui::title_bar::frame(
                element,
                window_id,
                self.title(window_id),
                self.title_bar_style,
                self.focused_window_id == Some(window_id),
                self.maximized_windows
                    .get(&window_id)
                    .copied()
                    .unwrap_or(false),
            )
        } else {
            element
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        let windows: [Option<&dyn ManagedWindow>; 2] = [
            self.settings_window.as_ref().map(|window| window as _),
            self.advanced_search_window
                .as_ref()
                .map(|window| window as _),
        ];

        windows
            .into_iter()
            .flatten()
            .find(|window| window.is(window_id))
            .map_or_else(|| self.workspace.title(), Title::title)
    }

    pub fn theme(&self, _window_id: window::Id) -> Option<Theme> {
        ui::styles::modern_theme(self.settings.appearance)
    }

    pub(super) fn chrome_animation_info(&self) -> ui::ChromeAnimationInfo {
        let animation = self.chrome_animation;
        let find = animation.find;
        let inline_replace = animation.inline_replace;
        let function_list = animation.function_list;
        let about = animation.about;

        ui::ChromeAnimationInfo {
            find_rendered_visible: find.rendered_visible(),
            find_progress: find.progress(),
            inline_replace_rendered_visible: inline_replace.rendered_visible(),
            inline_replace_progress: inline_replace.progress(),
            function_list_rendered_visible: function_list.rendered_visible(),
            function_list_progress: function_list.progress(),
            about_rendered_visible: about.rendered_visible(),
            about_progress: about.progress(),
            about_interactive: about.target_visible(),
            dirty_close_progress: self.close_prompt.progress(),
            dirty_close_interactive: self.close_prompt.interactive(),
        }
    }
}
