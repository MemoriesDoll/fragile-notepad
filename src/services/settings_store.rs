//! Persistence for application settings.

use crate::core::EditorSettings;
use crate::message::{SettingsError, SettingsLoadResult, SettingsSaveResult};
use crate::platform::paths;

use std::path::PathBuf;

const SETTINGS_FILE: &str = "settings.xml";

pub async fn load_settings() -> SettingsLoadResult {
    let Some(path) = settings_path() else {
        return Err(SettingsError::Unavailable);
    };

    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(Some(EditorSettings::from_xml_str(&contents))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SettingsError::Io(error.kind())),
    }
}

pub async fn save_settings(settings: EditorSettings) -> SettingsSaveResult {
    let Some(path) = settings_path() else {
        return Err(SettingsError::Unavailable);
    };

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| SettingsError::Io(error.kind()))?;
    }

    super::atomic_write::write(&path, settings.to_xml_string().as_bytes())
        .await
        .map_err(|error| SettingsError::Io(error.kind()))
}

fn settings_path() -> Option<PathBuf> {
    config_dir().map(|path| path.join(SETTINGS_FILE))
}

fn config_dir() -> Option<PathBuf> {
    paths::config_dir()
}
