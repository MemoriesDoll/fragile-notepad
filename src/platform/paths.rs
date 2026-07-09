use std::env;
use std::path::PathBuf;

#[cfg(windows)]
const APP_DIR_WINDOWS: &str = "FragileNotepad";
#[cfg(unix)]
const APP_DIR_UNIX: &str = "fragile-notepad";

pub(crate) fn config_dir() -> Option<PathBuf> {
    config_dir_platform()
}

pub(crate) fn cache_dir() -> Option<PathBuf> {
    cache_dir_platform()
}

#[cfg(windows)]
fn config_dir_platform() -> Option<PathBuf> {
    if let Some(appdata) = env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join(APP_DIR_WINDOWS));
    }

    None
}

#[cfg(unix)]
fn config_dir_platform() -> Option<PathBuf> {
    if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg_config_home).join(APP_DIR_UNIX));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join(APP_DIR_UNIX))
}

#[cfg(not(any(windows, unix)))]
fn config_dir_platform() -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn cache_dir_platform() -> Option<PathBuf> {
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

    None
}

#[cfg(unix)]
fn cache_dir_platform() -> Option<PathBuf> {
    if let Some(xdg_cache_home) = env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(xdg_cache_home).join(APP_DIR_UNIX));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache").join(APP_DIR_UNIX))
}

#[cfg(not(any(windows, unix)))]
fn cache_dir_platform() -> Option<PathBuf> {
    None
}
