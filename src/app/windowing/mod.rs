use iced::{Task, window};

use super::App;
use crate::ipc::ActivationRequest;
use crate::message::{Message, WindowTarget};
use crate::ui::toolbar::WindowMenuState;
use crate::ui::window_list_dialog::WindowListEntry;

mod managed;
mod platform_activation;

pub(super) use managed::{AdvancedSearchWindow, ManagedWindow, SettingsWindow};

impl App {
    pub(super) fn update_window(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowOpened(_id) => Task::none(),
            Message::WindowCloseRequested(id) => {
                if let Some(settings_window) = &self.settings_window
                    && settings_window.is(id)
                {
                    settings_window.close_requested(&mut self.settings_dialog, &self.settings)
                } else if let Some(search_window) = &self.advanced_search_window
                    && search_window.is(id)
                {
                    search_window.close_requested()
                } else if self.main_window_id == Some(id) {
                    self.exit_request()
                } else {
                    Task::none()
                }
            }
            Message::WindowClosed(id) => {
                if self
                    .settings_window
                    .as_ref()
                    .is_some_and(|settings_window| settings_window.is(id))
                {
                    self.settings_window = None;
                    self.settings_dialog.reset_from(&self.settings);
                    Task::none()
                } else if self
                    .advanced_search_window
                    .as_ref()
                    .is_some_and(|search_window| search_window.is(id))
                {
                    self.advanced_search_window = None;
                    Task::none()
                } else if self.main_window_id == Some(id) {
                    self.focused_window_id = None;
                    iced::exit()
                } else {
                    Task::none()
                }
            }
            _ => unreachable!("window handler received non-window message"),
        }
    }

    pub(super) fn show_main_window(&mut self, request: ActivationRequest) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();

        let Some(main_window_id) = self.main_window_id else {
            return Task::none();
        };

        platform_activation::prepare_for_presentation(&request);
        window::minimize(main_window_id, false)
            .chain(window::gain_focus(main_window_id))
            .chain(window::request_user_attention(
                main_window_id,
                Some(window::UserAttention::Informational),
            ))
    }

    pub(super) fn toggle_settings_window(&mut self) -> Task<Message> {
        self.active_menu = None;

        if let Some(settings_window) = self.settings_window {
            return iced::window::gain_focus(settings_window.id());
        }

        self.settings_dialog.reset_from(&self.settings);
        let (id, open) = iced::window::open(SettingsWindow::settings());
        self.settings_window = Some(SettingsWindow::new(id));

        open.map(Message::WindowOpened)
    }

    pub(super) fn close_settings_window(&mut self) -> Task<Message> {
        self.settings_window
            .map(|settings_window| iced::window::close(settings_window.id()))
            .unwrap_or_else(Task::none)
    }

    pub(super) fn open_advanced_search_window(&mut self) -> Task<Message> {
        if let Some(search_window) = self.advanced_search_window {
            return iced::window::gain_focus(search_window.id());
        }

        let (id, open) = iced::window::open(AdvancedSearchWindow::settings());
        self.advanced_search_window = Some(AdvancedSearchWindow::new(id));

        open.map(Message::WindowOpened)
    }

    pub(super) fn close_advanced_search_window(&mut self) -> Task<Message> {
        self.advanced_search_window
            .map(|search_window| iced::window::close(search_window.id()))
            .unwrap_or_else(Task::none)
    }

    pub(super) fn window_menu_state(&self) -> WindowMenuState {
        WindowMenuState {
            open_window_count: self.open_window_targets().len(),
        }
    }

    pub(super) fn window_list_entries(&self) -> Vec<WindowListEntry> {
        let focused = self.focused_window_target();

        self.open_window_targets()
            .into_iter()
            .map(|target| WindowListEntry {
                target,
                title: self.window_title(target),
                is_focused: focused == Some(target),
            })
            .collect()
    }

    pub(super) fn focus_window(&mut self, target: WindowTarget) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();
        self.is_window_list_visible = false;

        let Some(id) = self.window_id(target) else {
            return Task::none();
        };

        self.focused_window_id = Some(id);
        iced::window::gain_focus(id)
    }

    pub(super) fn focus_adjacent_window(&mut self, direction: isize) -> Task<Message> {
        self.active_menu = None;
        self.active_menu_path.clear();

        let targets = self.open_window_targets();
        if targets.len() < 2 {
            return Task::none();
        }

        let current = self
            .focused_window_target()
            .and_then(|target| targets.iter().position(|candidate| *candidate == target))
            .unwrap_or(0);
        let next = if direction.is_negative() {
            current
                .checked_sub(direction.unsigned_abs())
                .unwrap_or_else(|| targets.len() - 1)
        } else {
            (current + direction as usize) % targets.len()
        };

        self.focus_window(targets[next])
    }

    fn open_window_targets(&self) -> Vec<WindowTarget> {
        let mut targets = Vec::new();

        if self.main_window_id.is_some() {
            targets.push(WindowTarget::Main);
        }
        if self.settings_window.is_some() {
            targets.push(WindowTarget::Settings);
        }
        if self.advanced_search_window.is_some() {
            targets.push(WindowTarget::AdvancedSearch);
        }

        targets
    }

    fn focused_window_target(&self) -> Option<WindowTarget> {
        let id = self.focused_window_id?;

        self.open_window_targets()
            .into_iter()
            .find(|target| self.window_id(*target) == Some(id))
    }

    fn window_id(&self, target: WindowTarget) -> Option<iced::window::Id> {
        match target {
            WindowTarget::Main => self.main_window_id,
            WindowTarget::Settings => self.settings_window.map(|window| window.id()),
            WindowTarget::AdvancedSearch => self.advanced_search_window.map(|window| window.id()),
        }
    }

    fn window_title(&self, target: WindowTarget) -> String {
        match target {
            WindowTarget::Main => self
                .workspace
                .active_document()
                .map(|document| format!("{} - Fragile Notepad", document.title()))
                .unwrap_or_else(|| String::from("Fragile Notepad")),
            WindowTarget::Settings => SettingsWindow::TITLE.to_owned(),
            WindowTarget::AdvancedSearch => AdvancedSearchWindow::TITLE.to_owned(),
        }
    }
}
