use std::io;
use std::path::PathBuf;

pub const SHOW_SIGNAL: &[u8] = b"show\n";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivationRequest {
    pub xdg_activation_token: Option<String>,
    pub desktop_startup_id: Option<String>,
}

impl ActivationRequest {
    #[cfg(unix)]
    pub fn from_environment() -> Self {
        Self {
            xdg_activation_token: env_value("XDG_ACTIVATION_TOKEN"),
            desktop_startup_id: env_value("DESKTOP_STARTUP_ID"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.xdg_activation_token.is_none() && self.desktop_startup_id.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Show(ActivationRequest),
}

#[cfg(unix)]
fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleInstanceConfig {
    pub app_id: String,
}

impl SingleInstanceConfig {
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
        }
    }

    pub fn sanitized_app_id(&self) -> String {
        sanitize_app_id(&self.app_id)
    }
}

#[derive(Debug)]
pub enum Startup {
    Primary(PrimaryInstance),
    Secondary,
}

#[derive(Debug)]
pub struct PrimaryInstance {
    platform: platform::PrimaryInstance,
}

impl PrimaryInstance {
    pub fn accept_signal(&self) -> io::Result<Signal> {
        self.platform.accept_signal()
    }

    pub fn supports_signals(&self) -> bool {
        self.platform.supports_signals()
    }
}

pub fn claim_or_signal(config: &SingleInstanceConfig) -> io::Result<Startup> {
    platform::claim_or_signal(config).map(|startup| match startup {
        platform::Startup::Primary(platform) => Startup::Primary(PrimaryInstance { platform }),
        platform::Startup::Secondary => Startup::Secondary,
    })
}

pub fn runtime_dir() -> PathBuf {
    platform::runtime_dir()
}

pub fn sanitize_app_id(app_id: &str) -> String {
    let mut sanitized = String::with_capacity(app_id.len());

    for ch in app_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    if sanitized.is_empty() {
        String::from("app")
    } else {
        sanitized
    }
}

#[cfg(windows)]
#[path = "ipc/windows.rs"]
mod windows;

#[cfg(windows)]
mod platform {
    pub use super::windows::{PrimaryInstance, Startup, claim_or_signal, runtime_dir};
}

#[cfg(unix)]
#[path = "ipc/unix.rs"]
mod unix;

#[cfg(unix)]
mod platform {
    pub use super::unix::{PrimaryInstance, Startup, claim_or_signal, runtime_dir};
}

#[cfg(not(any(windows, unix)))]
mod platform {
    use super::{Signal, SingleInstanceConfig};

    use std::io;
    use std::path::PathBuf;

    #[derive(Debug)]
    pub enum Startup {
        Primary(PrimaryInstance),
        Secondary,
    }

    #[derive(Debug)]
    pub struct PrimaryInstance;

    impl PrimaryInstance {
        pub fn accept_signal(&self) -> io::Result<Signal> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "single-instance IPC is not supported on this platform",
            ))
        }

        pub fn supports_signals(&self) -> bool {
            false
        }
    }

    pub fn claim_or_signal(_config: &SingleInstanceConfig) -> io::Result<Startup> {
        Ok(Startup::Primary(PrimaryInstance))
    }

    pub fn runtime_dir() -> PathBuf {
        std::env::temp_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::{SingleInstanceConfig, sanitize_app_id};

    #[test]
    fn app_id_sanitizer_preserves_simple_cross_platform_names() {
        assert_eq!(
            sanitize_app_id("fragile-notepad_user-1"),
            "fragile-notepad_user-1"
        );
    }

    #[test]
    fn app_id_sanitizer_replaces_path_and_namespace_separators() {
        assert_eq!(
            sanitize_app_id("Fragile Notepad/org.example\\main"),
            "Fragile_Notepad_org_example_main"
        );
    }

    #[test]
    fn config_exposes_sanitized_app_id() {
        let config = SingleInstanceConfig::new("fragile notepad");

        assert_eq!(config.sanitized_app_id(), "fragile_notepad");
    }

    #[cfg(unix)]
    #[test]
    fn runtime_dir_includes_app_specific_leaf() {
        let dir = super::runtime_dir();
        let leaf = dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("runtime dir should have a UTF-8 leaf");

        assert!(
            leaf.starts_with("fragile-notepad-"),
            "runtime dir should be app and user scoped, got {dir:?}"
        );
    }
}
