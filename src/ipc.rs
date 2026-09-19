use std::io;
use std::path::PathBuf;

pub const SHOW_SIGNAL: &[u8] = b"show\n";
const FRAME_MAGIC: &[u8; 4] = b"FN03";
pub(super) const MAX_SIGNAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_FILES: usize = 4096;
const MAX_FIELD_BYTES: usize = 128 * 1024;

fn encode_signal(files: &[PathBuf], request: &ActivationRequest) -> io::Result<Vec<u8>> {
    if files.len() > MAX_FILES {
        return Err(invalid_signal("too many file paths"));
    }
    let mut body = Vec::new();
    body.extend_from_slice(&(files.len() as u32).to_le_bytes());
    push_field(
        &mut body,
        request
            .xdg_activation_token
            .as_deref()
            .unwrap_or("")
            .as_bytes(),
    )?;
    push_field(
        &mut body,
        request
            .desktop_startup_id
            .as_deref()
            .unwrap_or("")
            .as_bytes(),
    )?;
    for file in files {
        if !file.is_absolute() {
            return Err(invalid_signal("forwarded file paths must be absolute"));
        }
        push_field(&mut body, &path_bytes(file))?;
    }
    let mut frame = Vec::with_capacity(body.len() + 8);
    frame.extend_from_slice(FRAME_MAGIC);
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend(body);
    Ok(frame)
}

fn push_field(body: &mut Vec<u8>, value: &[u8]) -> io::Result<()> {
    if value.len() > MAX_FIELD_BYTES || body.len() + 4 + value.len() > MAX_SIGNAL_BYTES - 8 {
        return Err(invalid_signal("IPC payload exceeds size limit"));
    }
    body.extend_from_slice(&(value.len() as u32).to_le_bytes());
    body.extend_from_slice(value);
    Ok(())
}

fn decode_signal(frame: &[u8]) -> io::Result<Signal> {
    if frame.len() < 8 || &frame[..4] != FRAME_MAGIC || frame.len() > MAX_SIGNAL_BYTES {
        return Err(invalid_signal("invalid IPC header"));
    }
    let length = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
    if frame.len() != length + 8 {
        return Err(invalid_signal("invalid IPC frame length"));
    }
    let mut remaining = &frame[8..];
    let count = take_u32(&mut remaining)? as usize;
    if count > MAX_FILES {
        return Err(invalid_signal("too many file paths"));
    }
    let mut token = || -> io::Result<Option<String>> {
        let field = take_field(&mut remaining)?;
        if field.is_empty() {
            return Ok(None);
        }
        String::from_utf8(field.to_vec())
            .map(Some)
            .map_err(|_| invalid_signal("invalid activation token"))
    };
    let request = ActivationRequest {
        xdg_activation_token: token()?,
        desktop_startup_id: token()?,
    };
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let path = path_from_bytes(take_field(&mut remaining)?)?;
        if !path.is_absolute() {
            return Err(invalid_signal("forwarded file paths must be absolute"));
        }
        files.push(path);
    }
    if !remaining.is_empty() {
        return Err(invalid_signal("unexpected IPC trailing bytes"));
    }
    Ok(if files.is_empty() {
        Signal::Show(request)
    } else {
        Signal::OpenFiles(files, request)
    })
}

fn take_u32(bytes: &mut &[u8]) -> io::Result<u32> {
    let head = bytes
        .get(..4)
        .ok_or_else(|| invalid_signal("truncated IPC field"))?;
    let value = u32::from_le_bytes(head.try_into().unwrap());
    *bytes = &bytes[4..];
    Ok(value)
}

fn take_field<'a>(bytes: &mut &'a [u8]) -> io::Result<&'a [u8]> {
    let length = take_u32(bytes)? as usize;
    if length > MAX_FIELD_BYTES {
        return Err(invalid_signal("IPC field exceeds size limit"));
    }
    let field = bytes
        .get(..length)
        .ok_or_else(|| invalid_signal("truncated IPC field"))?;
    *bytes = &bytes[length..];
    Ok(field)
}

fn invalid_signal(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(windows)]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    if !bytes.len().is_multiple_of(2) {
        return Err(invalid_signal("invalid UTF-16 path length"));
    }
    let wide = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    if wide.contains(&0) {
        return Err(invalid_signal("NUL in file path"));
    }
    Ok(std::ffi::OsString::from_wide(&wide).into())
}

#[cfg(unix)]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    if bytes.contains(&0) {
        return Err(invalid_signal("NUL in file path"));
    }
    Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
}

#[cfg(not(any(windows, unix)))]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(not(any(windows, unix)))]
fn path_from_bytes(bytes: &[u8]) -> io::Result<PathBuf> {
    std::str::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|_| invalid_signal("invalid path"))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivationRequest {
    pub xdg_activation_token: Option<String>,
    pub desktop_startup_id: Option<String>,
}

/// A one-shot decision made by the UI before a forwarding client is acknowledged.
#[derive(Debug, Clone)]
pub struct AdmissionReceipt {
    state: std::sync::Arc<(std::sync::Mutex<Option<bool>>, std::sync::Condvar)>,
    deadline: std::time::Instant,
}

impl Default for AdmissionReceipt {
    fn default() -> Self {
        Self {
            state: Default::default(),
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        }
    }
}

impl AdmissionReceipt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve(&self, accepted: bool) {
        let (state, wake) = &*self.state;
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        if state.is_none() {
            *state = Some(accepted && std::time::Instant::now() < self.deadline);
            wake.notify_all();
        }
    }

    /// Claims a pending request. A timed-out or previously handled request cannot open files.
    pub fn try_accept(&self) -> bool {
        let (state, wake) = &*self.state;
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        if state.is_some() {
            return false;
        }
        if std::time::Instant::now() >= self.deadline {
            *state = Some(false);
            wake.notify_all();
            return false;
        }
        *state = Some(true);
        wake.notify_all();
        true
    }

    pub fn wait_for_acceptance(&self) -> bool {
        self.wait_for(std::time::Duration::from_secs(5))
    }

    fn wait_for(&self, timeout: std::time::Duration) -> bool {
        let (state, wake) = &*self.state;
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        let (mut state, _) = wake
            .wait_timeout_while(
                state,
                timeout.min(
                    self.deadline
                        .saturating_duration_since(std::time::Instant::now()),
                ),
                |state| state.is_none(),
            )
            .unwrap_or_else(|error| error.into_inner());
        *state.get_or_insert(false)
    }
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
    OpenFiles(Vec<PathBuf>, ActivationRequest),
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
        self.accept_signal_with(|_| true)
    }

    pub fn accept_signal_with(&self, admit: impl FnOnce(&Signal) -> bool) -> io::Result<Signal> {
        self.platform.accept_signal_with(admit)
    }

    pub fn supports_signals(&self) -> bool {
        self.platform.supports_signals()
    }
}

pub fn claim_or_signal(config: &SingleInstanceConfig) -> io::Result<Startup> {
    claim_or_signal_with_files(config, &[])
}

pub fn claim_or_signal_with_files(
    config: &SingleInstanceConfig,
    files: &[PathBuf],
) -> io::Result<Startup> {
    platform::claim_or_signal(config, files).map(|startup| match startup {
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
        pub fn accept_signal_with(
            &self,
            _admit: impl FnOnce(&Signal) -> bool,
        ) -> io::Result<Signal> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "single-instance IPC is not supported on this platform",
            ))
        }

        pub fn supports_signals(&self) -> bool {
            false
        }
    }

    pub fn claim_or_signal(
        _config: &SingleInstanceConfig,
        _files: &[PathBuf],
    ) -> io::Result<Startup> {
        Ok(Startup::Primary(PrimaryInstance))
    }

    pub fn runtime_dir() -> PathBuf {
        std::env::temp_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::{SingleInstanceConfig, sanitize_app_id};
    use std::path::PathBuf;

    #[test]
    fn admission_timeout_permanently_rejects_late_ui_delivery() {
        let receipt = super::AdmissionReceipt::new();
        assert!(!receipt.wait_for(std::time::Duration::ZERO));
        assert!(!receipt.try_accept());
        receipt.resolve(true);
        assert!(!receipt.wait_for_acceptance());
    }

    #[test]
    fn admission_acceptance_is_single_use_and_wakes_waiter() {
        let receipt = super::AdmissionReceipt::new();
        let waiter = receipt.clone();
        let thread = std::thread::spawn(move || waiter.wait_for_acceptance());
        assert!(receipt.try_accept());
        assert!(!receipt.try_accept());
        assert!(thread.join().unwrap());
        receipt.resolve(false);
        assert!(receipt.wait_for_acceptance());
    }

    #[test]
    fn file_frame_round_trips_paths_and_activation() {
        let files = vec![
            std::env::temp_dir().join("a b.txt"),
            std::env::temp_dir().join("-second.txt"),
        ];
        let request = super::ActivationRequest {
            xdg_activation_token: Some("token\nwith-separator".into()),
            desktop_startup_id: Some("startup".into()),
        };
        let frame = super::encode_signal(&files, &request).unwrap();
        assert_eq!(
            super::decode_signal(&frame).unwrap(),
            super::Signal::OpenFiles(files, request)
        );
        for length in 0..frame.len() {
            assert!(super::decode_signal(&frame[..length]).is_err());
        }
        let mut bad_version = frame.clone();
        bad_version[3] = b'4';
        assert!(super::decode_signal(&bad_version).is_err());
        let mut trailing = frame;
        trailing.push(0);
        assert!(super::decode_signal(&trailing).is_err());
    }

    #[test]
    fn file_frame_rejects_excessive_and_relative_paths() {
        assert!(
            super::encode_signal(
                &[PathBuf::from("relative.txt")],
                &super::ActivationRequest::default()
            )
            .is_err()
        );
        assert!(
            super::encode_signal(
                &vec![std::env::temp_dir(); super::MAX_FILES + 1],
                &super::ActivationRequest::default()
            )
            .is_err()
        );
        let frame = [b"FN03".as_slice(), &(u32::MAX).to_le_bytes()].concat();
        assert!(super::decode_signal(&frame).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn file_frame_preserves_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/\xfffile".to_vec()));
        let frame =
            super::encode_signal(&[path.clone()], &super::ActivationRequest::default()).unwrap();
        assert_eq!(
            super::decode_signal(&frame).unwrap(),
            super::Signal::OpenFiles(vec![path], super::ActivationRequest::default())
        );
    }

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
