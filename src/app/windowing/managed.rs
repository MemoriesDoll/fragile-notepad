use iced::{Element, Size, Task, window};

use crate::core::EditorSettings;
use crate::message::Message;
use crate::search_dialog::SearchDialogState;
use crate::settings_dialog::SettingsDialogState;
use crate::ui;

pub(crate) trait ManagedWindow {
    fn id(&self) -> window::Id;

    fn is(&self, id: window::Id) -> bool {
        self.id() == id
    }

    fn title(&self) -> String;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SettingsWindow {
    id: window::Id,
}

impl SettingsWindow {
    pub(crate) const TITLE: &'static str = "Settings - Fragile Notepad";

    pub(crate) const fn new(id: window::Id) -> Self {
        Self { id }
    }

    pub(crate) fn settings() -> window::Settings {
        window::Settings {
            size: Size::new(820.0, 560.0),
            min_size: Some(Size::new(720.0, 460.0)),
            resizable: true,
            exit_on_close_request: false,
            ..window::Settings::default()
        }
    }
}

impl ManagedWindow for SettingsWindow {
    fn id(&self) -> window::Id {
        self.id
    }

    fn title(&self) -> String {
        Self::TITLE.to_owned()
    }
}

impl SettingsWindow {
    pub(crate) fn view<'a>(
        &self,
        settings_dialog: &'a SettingsDialogState,
    ) -> Element<'a, Message> {
        ui::settings_panel::view(settings_dialog)
    }

    pub(crate) fn close_requested(
        &self,
        settings_dialog: &mut SettingsDialogState,
        settings: &EditorSettings,
    ) -> Task<Message> {
        settings_dialog.reset_from(settings);
        window::close(self.id())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AdvancedSearchWindow {
    id: window::Id,
}

impl AdvancedSearchWindow {
    pub(crate) const TITLE: &'static str = "Find and Replace - Fragile Notepad";

    pub(crate) const fn new(id: window::Id) -> Self {
        Self { id }
    }

    pub(crate) fn settings() -> window::Settings {
        window::Settings {
            size: Size::new(760.0, 560.0),
            min_size: Some(Size::new(680.0, 500.0)),
            resizable: true,
            exit_on_close_request: false,
            ..window::Settings::default()
        }
    }

    pub(crate) fn view<'a>(&self, search_dialog: &'a SearchDialogState) -> Element<'a, Message> {
        ui::advanced_search_panel::view(search_dialog)
    }

    pub(crate) fn close_requested(&self) -> Task<Message> {
        window::close(self.id())
    }
}

impl ManagedWindow for AdvancedSearchWindow {
    fn id(&self) -> window::Id {
        self.id
    }

    fn title(&self) -> String {
        Self::TITLE.to_owned()
    }
}
