//! Persistence for application settings.

use crate::core::EditorSettings;
use crate::message::{SettingsError, SettingsLoadResult, SettingsSaveResult};
use crate::platform::paths;

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const SETTINGS_FILE: &str = "settings.xml";

pub async fn load_settings() -> SettingsLoadResult {
    let Some(path) = settings_path() else {
        return Err(SettingsError::Unavailable);
    };

    match tokio::fs::read_to_string(path).await {
        Ok(contents) => {
            let xml = roxmltree::Document::parse(&contents)
                .map_err(|_| SettingsError::Io(std::io::ErrorKind::InvalidData))?;
            if xml.root_element().tag_name().name() != "fragile-notepad-settings" {
                return Err(SettingsError::Io(std::io::ErrorKind::InvalidData));
            }
            Ok(Some(EditorSettings::from_xml_str(&contents)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SettingsError::Io(error.kind())),
    }
}

#[derive(Default)]
struct SettingsWriter {
    pending: Mutex<(u64, Option<EditorSettings>)>,
    writing: tokio::sync::Mutex<()>,
}

impl SettingsWriter {
    fn enqueue(&self, settings: EditorSettings) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        pending.0 = pending.0.wrapping_add(1);
        pending.1 = Some(settings);
    }

    async fn flush(&self, path: &std::path::Path) -> SettingsSaveResult {
        let _writing = self.writing.lock().await;
        loop {
            let snapshot = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            let (generation, Some(settings)) = snapshot else {
                return Ok(());
            };
            super::atomic_write::write_private(path, settings.to_xml_string().as_bytes())
                .await
                .map_err(|error| SettingsError::Io(error.kind()))?;
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if pending.0 == generation {
                pending.1 = None;
            }
        }
    }
}

fn writer() -> &'static SettingsWriter {
    static WRITER: OnceLock<SettingsWriter> = OnceLock::new();
    WRITER.get_or_init(SettingsWriter::default)
}

// Enqueue at call time: executor polling order must not reorder snapshots.
pub fn save_settings(settings: EditorSettings) -> impl Future<Output = SettingsSaveResult> {
    writer().enqueue(settings);
    flush_settings()
}

pub async fn flush_settings() -> SettingsSaveResult {
    let path = settings_path().ok_or(SettingsError::Unavailable)?;
    writer().flush(&path).await
}

fn settings_path() -> Option<PathBuf> {
    config_dir().map(|path| path.join(SETTINGS_FILE))
}

fn config_dir() -> Option<PathBuf> {
    paths::config_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_requested_settings_win_when_futures_are_polled_in_reverse_order() {
        let directory = std::env::temp_dir().join(format!(
            "fragile-settings-order-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("settings.xml");
        let writer = SettingsWriter::default();
        let mut old = EditorSettings::default();
        old.record_open_history_path("old.txt");
        writer.enqueue(old);
        let old_future = writer.flush(&path);
        let mut latest = EditorSettings::default();
        latest.record_open_history_path("latest.txt");
        writer.enqueue(latest.clone());
        let latest_future = writer.flush(&path);
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let (a, b) = futures::join!(latest_future, old_future);
            a.unwrap();
            b.unwrap();
            assert_eq!(
                tokio::fs::read_to_string(&path).await.unwrap(),
                latest.to_xml_string()
            );
        });
        std::fs::remove_file(path).unwrap();
        // Windows can briefly retain a delete-pending file after the handle closes.
        for attempt in 0..10 {
            match std::fs::remove_dir(&directory) {
                Ok(()) => return,
                Err(error)
                    if error.kind() == std::io::ErrorKind::DirectoryNotEmpty && attempt < 9 =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => panic!("remove test directory: {error}"),
            }
        }
    }
}
