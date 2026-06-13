use crate::core::{EditorSettings, ShortcutCommand, ShortcutConflict, ShortcutGroup};
use crate::message::SettingsCategory;

#[derive(Debug, Clone)]
pub struct SettingsDialogState {
    pub draft: EditorSettings,
    pub category: SettingsCategory,
    pub shortcut_group: ShortcutGroup,
    pub capturing_shortcut: Option<ShortcutCommand>,
    pub shortcut_conflict: Option<ShortcutConflict>,
}

impl SettingsDialogState {
    pub(crate) fn new(settings: &EditorSettings) -> Self {
        Self {
            draft: settings.clone(),
            category: SettingsCategory::General,
            shortcut_group: ShortcutGroup::File,
            capturing_shortcut: None,
            shortcut_conflict: None,
        }
    }

    pub(crate) fn reset_from(&mut self, settings: &EditorSettings) {
        self.draft = settings.clone();
        self.category = SettingsCategory::General;
        self.shortcut_group = ShortcutGroup::File;
        self.capturing_shortcut = None;
        self.shortcut_conflict = None;
    }

    pub(crate) fn apply_to(&self, settings: &mut EditorSettings) {
        *settings = self.draft.clone();
    }
}
