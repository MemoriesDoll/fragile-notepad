use iced::Task;

use super::App;
use crate::message::Message;
use crate::services;

impl App {
    pub(super) fn update_settings(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DraftThemeSelected(theme) => {
                self.settings_dialog.draft.set_syntax_theme(theme);
                Task::none()
            }
            Message::DraftWordWrapToggled(word_wrap) => {
                self.settings_dialog.draft.set_word_wrap(word_wrap);
                Task::none()
            }
            Message::DraftAppearanceSelected(appearance) => {
                self.settings_dialog.draft.set_appearance(appearance);
                Task::none()
            }
            Message::DraftHardwareAccelerationSelected(mode) => {
                self.settings_dialog.draft.set_hardware_acceleration(mode);
                Task::none()
            }
            Message::DraftIndentationSelected(indentation) => {
                self.settings_dialog.draft.set_indentation(indentation);
                Task::none()
            }
            Message::DraftLineNumbersToggled(show_line_numbers) => {
                self.settings_dialog
                    .draft
                    .set_show_line_numbers(show_line_numbers);
                Task::none()
            }
            Message::DraftVisibleSpacesToggled(show_spaces) => {
                self.settings_dialog.draft.set_show_spaces(show_spaces);
                Task::none()
            }
            Message::DraftVisibleTabsToggled(show_tabs) => {
                self.settings_dialog.draft.set_show_tabs(show_tabs);
                Task::none()
            }
            Message::DraftEolMarkersToggled(show_end_of_line_markers) => {
                self.settings_dialog
                    .draft
                    .set_show_end_of_line_markers(show_end_of_line_markers);
                Task::none()
            }
            Message::DraftIndentationGuidesToggled(show_indentation_guides) => {
                self.settings_dialog
                    .draft
                    .set_show_indentation_guides(show_indentation_guides);
                Task::none()
            }
            Message::DraftFoldingControlsToggled(show_folding_controls) => {
                self.settings_dialog
                    .draft
                    .set_show_folding_controls(show_folding_controls);
                Task::none()
            }
            Message::SettingsCategorySelected(category) => {
                self.settings_dialog.category = category;
                self.settings_dialog.capturing_shortcut = None;
                self.settings_dialog.shortcut_conflict = None;
                Task::none()
            }
            Message::ShortcutGroupSelected(group) => {
                self.settings_dialog.shortcut_group = group;
                self.settings_dialog.capturing_shortcut = None;
                self.settings_dialog.shortcut_conflict = None;
                Task::none()
            }
            Message::SettingsZoomIn => {
                self.settings_dialog.draft.zoom_in();
                Task::none()
            }
            Message::SettingsZoomOut => {
                self.settings_dialog.draft.zoom_out();
                Task::none()
            }
            Message::SettingsZoomReset => {
                self.settings_dialog.draft.reset_zoom();
                Task::none()
            }
            Message::SettingsScrollSpeedIncrease => {
                self.settings_dialog.draft.increase_scroll_speed();
                Task::none()
            }
            Message::SettingsScrollSpeedDecrease => {
                self.settings_dialog.draft.decrease_scroll_speed();
                Task::none()
            }
            Message::SettingsScrollSpeedReset => {
                self.settings_dialog.draft.reset_scroll_speed();
                Task::none()
            }
            Message::ApplySettings => {
                if self.apply_settings_dialog() {
                    self.request_gpu_boost()
                } else {
                    Task::none()
                }
            }
            Message::SaveSettings => {
                let boost_task = if self.apply_settings_dialog() {
                    self.request_gpu_boost()
                } else {
                    Task::none()
                };

                Task::batch([
                    self.persist_settings(),
                    self.close_settings_window(),
                    boost_task,
                ])
            }
            Message::SettingsLoaded(result) => {
                if let Ok(Some(settings)) = result {
                    self.settings = settings;
                    self.settings_dialog.reset_from(&self.settings);
                    self.apply_decorations();
                    self.prewarm_active_syntax_cache();

                    if super::rendering::startup_gpu_boost_requested(&self.settings) {
                        if self.main_window_opened {
                            return self.request_gpu_boost();
                        }

                        self.pending_startup_gpu_boost = true;
                    }
                }

                Task::none()
            }
            Message::SettingsPersisted(result) => {
                if let Err(error) = result {
                    self.file_status = Some(format!("Settings save failed: {}", error.summary()));
                }

                Task::none()
            }
            Message::CancelSettings => self.cancel_settings_dialog(),
            Message::ToggleSettingsPanel => self.toggle_settings_window(),
            Message::ShortcutCaptureStarted(command) => {
                self.settings_dialog.capturing_shortcut = Some(command);
                self.settings_dialog.shortcut_conflict = None;
                Task::none()
            }
            Message::ShortcutCaptured(command, binding) => {
                self.settings_dialog.capturing_shortcut = None;
                match self
                    .settings_dialog
                    .draft
                    .shortcuts
                    .set_binding(command, binding)
                {
                    Ok(()) => {
                        self.settings_dialog.shortcut_conflict = None;
                    }
                    Err(conflict) => {
                        self.settings_dialog.shortcut_conflict = Some(conflict);
                    }
                }
                Task::none()
            }
            Message::ShortcutCleared(command) => {
                self.settings_dialog.capturing_shortcut = None;
                self.settings_dialog.shortcut_conflict = None;
                self.settings_dialog.draft.shortcuts.clear(command);
                Task::none()
            }
            Message::ShortcutsResetToDefaults => {
                self.settings_dialog.capturing_shortcut = None;
                self.settings_dialog.shortcut_conflict = None;
                self.settings_dialog.draft.shortcuts.reset_to_defaults();
                Task::none()
            }
            Message::ShortcutConflictDismissed => {
                self.settings_dialog.shortcut_conflict = None;
                Task::none()
            }
            Message::ShortcutCaptureConflict(conflict) => {
                self.settings_dialog.capturing_shortcut = None;
                self.settings_dialog.shortcut_conflict = Some(conflict);
                Task::none()
            }
            Message::ZoomIn => {
                self.active_menu = None;
                self.settings.zoom_in();
                self.persist_settings()
            }
            Message::ZoomOut => {
                self.active_menu = None;
                self.settings.zoom_out();
                self.persist_settings()
            }
            Message::ZoomReset => {
                self.active_menu = None;
                self.settings.reset_zoom();
                self.persist_settings()
            }
            Message::ToggleWordWrap => {
                self.active_menu = None;
                self.settings.set_word_wrap(!self.settings.word_wrap);
                self.persist_settings()
            }
            Message::ToggleLineNumbers => {
                self.active_menu = None;
                self.settings
                    .set_show_line_numbers(!self.settings.decorations.show_line_numbers);
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleSpaceAndTab => {
                self.active_menu = None;
                let show_space_and_tab =
                    !(self.settings.decorations.show_spaces && self.settings.decorations.show_tabs);
                self.settings.set_show_spaces(show_space_and_tab);
                self.settings.set_show_tabs(show_space_and_tab);
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleVisibleSpaces => {
                self.active_menu = None;
                self.settings
                    .set_show_spaces(!self.settings.decorations.show_spaces);
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleVisibleTabs => {
                self.active_menu = None;
                self.settings
                    .set_show_tabs(!self.settings.decorations.show_tabs);
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleEolMarkers => {
                self.active_menu = None;
                self.settings.set_show_end_of_line_markers(
                    !self.settings.decorations.show_end_of_line_markers,
                );
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleAllCharacters => {
                self.active_menu = None;
                let show_all = !(self.settings.decorations.show_spaces
                    && self.settings.decorations.show_tabs
                    && self.settings.decorations.show_end_of_line_markers);
                self.settings.set_show_spaces(show_all);
                self.settings.set_show_tabs(show_all);
                self.settings.set_show_end_of_line_markers(show_all);
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleIndentationGuides => {
                self.active_menu = None;
                self.settings.set_show_indentation_guides(
                    !self.settings.decorations.show_indentation_guides,
                );
                self.apply_decorations();
                self.persist_settings()
            }
            Message::ToggleFoldingControls => {
                self.active_menu = None;
                self.settings
                    .set_show_folding_controls(!self.settings.decorations.show_folding_controls);
                self.apply_decorations();
                self.persist_settings()
            }
            _ => unreachable!("settings handler received non-settings message"),
        }
    }

    fn apply_settings_dialog(&mut self) -> bool {
        let old_syntax_theme = self.settings.syntax_theme;
        let old_hardware_acceleration = self.settings.hardware_acceleration;
        self.settings_dialog.apply_to(&mut self.settings);
        self.apply_decorations();

        if self.settings.syntax_theme != old_syntax_theme {
            self.prewarm_active_syntax_cache();
        }

        self.settings.hardware_acceleration != old_hardware_acceleration
            && super::rendering::startup_gpu_boost_requested(&self.settings)
    }

    fn cancel_settings_dialog(&mut self) -> Task<Message> {
        self.settings_dialog.reset_from(&self.settings);
        self.close_settings_window()
    }

    pub(super) fn persist_settings(&self) -> Task<Message> {
        Task::perform(
            services::save_settings(self.settings.clone()),
            Message::SettingsPersisted,
        )
    }

    fn apply_decorations(&mut self) {
        let decorations = self.settings.decoration_settings();

        for document in &mut self.workspace.documents {
            document.set_decoration_settings(decorations);
        }
    }
}
