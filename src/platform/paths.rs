use std::env;
use std::path::PathBuf;

const APP_DIR_WINDOWS: &str = "FragileNotepad";
const APP_DIR_UNIX: &str = "fragile-notepad";

pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(appdata) = env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join(APP_DIR_WINDOWS));
    }

    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg_config_home).join(APP_DIR_UNIX));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join(APP_DIR_UNIX))
}

pub(crate) fn cache_dir() -> Option<PathBuf> {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local_app_data)
                .join(APP_DIR_WINDOWS)
                .join("Cache"),
        );
    }

    if let Some(appdata) = env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join(APP_DIR_WINDOWS).join("Cache"));
    }

    if let Some(xdg_cache_home) = env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(xdg_cache_home).join(APP_DIR_UNIX));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache").join(APP_DIR_UNIX))
}
