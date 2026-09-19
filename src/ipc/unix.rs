use super::{ActivationRequest, SHOW_SIGNAL, Signal, SingleInstanceConfig};

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum Startup {
    Primary(PrimaryInstance),
    Secondary,
}

#[derive(Debug)]
pub struct PrimaryInstance {
    _lock: File,
    listener: UnixListener,
    socket_path: PathBuf,
}

impl Drop for PrimaryInstance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket_path);
    }
}

impl PrimaryInstance {
    pub fn accept_signal_with(&self, admit: impl FnOnce(&Signal) -> bool) -> io::Result<Signal> {
        loop {
            match self.listener.accept() {
                Ok((mut stream, _address)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                    let mut payload = Vec::new();
                    if (&mut stream)
                        .take(super::MAX_SIGNAL_BYTES as u64 + 1)
                        .read_to_end(&mut payload)
                        .is_err()
                    {
                        continue;
                    }

                    if let Ok(signal) = super::decode_signal(&payload) {
                        let accepted = admit(&signal);
                        let _ = stream.write_all(&[u8::from(accepted)]);
                        return Ok(signal);
                    }
                    if payload.len() <= MAX_SIGNAL_BYTES as usize
                        && let Some(request) = parse_signal_payload(&payload)
                    {
                        let signal = Signal::Show(request);
                        let _ = admit(&signal);
                        return Ok(signal);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    pub fn supports_signals(&self) -> bool {
        true
    }
}

pub fn claim_or_signal(config: &SingleInstanceConfig, files: &[PathBuf]) -> io::Result<Startup> {
    let paths = InstancePaths::new(config);
    paths.ensure_dir()?;

    match acquire_lock(&paths.lock_path)? {
        LockStatus::Acquired(lock) => {
            let _ = fs::remove_file(&paths.socket_path);
            let listener = UnixListener::bind(&paths.socket_path)?;

            Ok(Startup::Primary(PrimaryInstance {
                _lock: lock,
                listener,
                socket_path: paths.socket_path,
            }))
        }
        LockStatus::HeldByAnotherProcess => {
            signal_existing_instance(&paths.socket_path, files)?;
            Ok(Startup::Secondary)
        }
    }
}

pub fn runtime_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    base.join(format!("fragile-notepad-{}", effective_user_id()))
}

fn acquire_lock(path: &PathBuf) -> io::Result<LockStatus> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

    if result == 0 {
        Ok(LockStatus::Acquired(lock))
    } else {
        let error = io::Error::last_os_error();

        if matches!(
            error.raw_os_error(),
            Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN
        ) {
            Ok(LockStatus::HeldByAnotherProcess)
        } else {
            Err(error)
        }
    }
}

fn signal_existing_instance(socket_path: &PathBuf, files: &[PathBuf]) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let request = ActivationRequest::from_environment();
    let payload = if files.is_empty() {
        signal_payload(&request)
    } else {
        super::encode_signal(files, &request)?
    };

    loop {
        match UnixStream::connect(socket_path) {
            Ok(mut stream) => {
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                stream.write_all(&payload)?;
                if !files.is_empty() {
                    stream.shutdown(std::net::Shutdown::Write)?;
                    stream.set_read_timeout(Some(Duration::from_secs(7)))?;
                    let mut acknowledgement = [0];
                    stream.read_exact(&mut acknowledgement)?;
                    if acknowledgement != [1] {
                        return Err(io::Error::new(
                            io::ErrorKind::ConnectionAborted,
                            "The running application could not accept the files (it may be closing). Retry after it exits.",
                        ));
                    }
                }
                return Ok(());
            }
            Err(error) if is_transient_signal_error(&error) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_transient_signal_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

fn effective_user_id() -> u32 {
    unsafe { libc::geteuid() }
}

const MAX_SIGNAL_BYTES: u64 = 8 * 1024;
const SIGNAL_HEADER: &str = "show-v2\n";

fn signal_payload(request: &ActivationRequest) -> Vec<u8> {
    if request.is_empty() {
        return SHOW_SIGNAL.to_vec();
    }

    let mut payload = String::from(SIGNAL_HEADER);

    if let Some(token) = &request.xdg_activation_token {
        payload.push_str("xdg=");
        payload.push_str(&hex_encode(token.as_bytes()));
        payload.push('\n');
    }

    if let Some(startup_id) = &request.desktop_startup_id {
        payload.push_str("desktop=");
        payload.push_str(&hex_encode(startup_id.as_bytes()));
        payload.push('\n');
    }

    payload.into_bytes()
}

fn parse_signal_payload(payload: &[u8]) -> Option<ActivationRequest> {
    if payload == SHOW_SIGNAL {
        return Some(ActivationRequest::default());
    }

    let payload = std::str::from_utf8(payload).ok()?;
    let body = payload.strip_prefix(SIGNAL_HEADER)?;
    let mut request = ActivationRequest::default();

    for line in body.lines() {
        if let Some(value) = line.strip_prefix("xdg=") {
            request.xdg_activation_token = hex_decode(value);
        } else if let Some(value) = line.strip_prefix("desktop=") {
            request.desktop_startup_id = hex_decode(value);
        }
    }

    Some(request)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }

    encoded
}

fn hex_decode(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) {
        return None;
    }

    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Some((high << 4) | low)
        })
        .collect::<Option<Vec<_>>>()?;

    String::from_utf8(bytes).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

enum LockStatus {
    Acquired(File),
    HeldByAnotherProcess,
}

struct InstancePaths {
    dir: PathBuf,
    lock_path: PathBuf,
    socket_path: PathBuf,
}

impl InstancePaths {
    fn new(config: &SingleInstanceConfig) -> Self {
        let app_id = config.sanitized_app_id();
        let dir = runtime_dir();

        Self {
            dir: dir.clone(),
            lock_path: dir.join(format!("{app_id}.lock")),
            socket_path: bounded_socket_path(dir.join(format!("{app_id}.sock"))),
        }
    }

    fn ensure_dir(&self) -> io::Result<()> {
        ensure_private_dir(&self.dir)?;
        let socket_dir = self.socket_path.parent().expect("socket has a parent");
        if socket_dir != self.dir {
            ensure_private_dir(socket_dir)?;
        }
        Ok(())
    }
}

fn bounded_socket_path(path: PathBuf) -> PathBuf {
    // macOS sun_path is 104 bytes, including the terminating NUL. Its default
    // TMPDIR can already consume most of this budget before adding our name.
    if path.as_os_str().as_bytes().len() < 104 {
        return path;
    }

    // Stable FNV-1a over the full original path keeps runtime namespaces and
    // instance IDs distinct. Do not use DefaultHasher: its algorithm may change.
    let hash = path
        .as_os_str()
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    PathBuf::from("/tmp")
        .join(format!("fragile-notepad-{}", effective_user_id()))
        .join(format!("{hash:016x}.sock"))
}

fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let metadata = fs::symlink_metadata(dir)?;
    if !metadata.is_dir() || metadata.uid() != effective_user_id() || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC directory must be private and owned by the current user",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_signal_payload, signal_payload};
    use crate::ipc::{ActivationRequest, SHOW_SIGNAL};

    #[test]
    fn socket_path_fallback_respects_byte_limit_and_runtime_namespace() {
        use std::path::PathBuf;

        let fitting = PathBuf::from(format!("/tmp/{}.sock", "a".repeat(93)));
        assert_eq!(fitting.as_os_str().len(), 103);
        assert_eq!(super::bounded_socket_path(fitting.clone()), fitting);

        // Unicode characters consume multiple bytes in sun_path.
        let long = PathBuf::from(format!("/tmp/{}.sock", "é".repeat(47)));
        assert_eq!(long.as_os_str().len(), 104);
        let fallback = super::bounded_socket_path(long.clone());
        assert!(fallback.as_os_str().len() < 104);
        assert_ne!(fallback, long);
        assert_eq!(fallback, super::bounded_socket_path(long.clone()));
        assert_ne!(
            fallback,
            super::bounded_socket_path(
                PathBuf::from("/different").join(long.strip_prefix("/").unwrap())
            )
        );
        assert_ne!(
            fallback,
            super::bounded_socket_path(long.with_extension("other.sock"))
        );
    }

    #[test]
    fn long_runtime_directory_supports_single_instance_forwarding() {
        const CHILD_ENV: &str = "FRAGILE_IPC_LONG_RUNTIME_TEST";
        if std::env::var_os(CHILD_ENV).is_some() {
            let config = super::SingleInstanceConfig::new(format!(
                "fragile-notepad-startup-probe-{}",
                std::process::id()
            ));
            let super::Startup::Primary(primary) = super::claim_or_signal(&config, &[]).unwrap()
            else {
                panic!("expected primary instance");
            };
            // Stay below macOS's 104-byte sun_path even when tested on Linux.
            assert!(primary.socket_path.as_os_str().len() < 104);
            let socket_path = primary.socket_path.clone();
            let receiver = std::thread::spawn(move || primary.accept_signal_with(|_| true));
            let files = vec![std::env::temp_dir().join("forwarded file.txt")];
            assert!(matches!(
                super::claim_or_signal(&config, &files).unwrap(),
                super::Startup::Secondary
            ));
            let super::Signal::OpenFiles(received, _) = receiver.join().unwrap().unwrap() else {
                panic!("expected forwarded files");
            };
            assert_eq!(received, files);
            assert!(
                !socket_path.exists(),
                "primary must remove its socket on exit"
            );
            assert!(matches!(
                super::claim_or_signal(&config, &[]).unwrap(),
                super::Startup::Primary(_)
            ));
            return;
        }

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::PathBuf::from(format!(
            "/tmp/fragile-ipc-test-{}-{unique}",
            std::process::id()
        ));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ipc::unix::tests::long_runtime_directory_supports_single_instance_forwarding",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .env("XDG_RUNTIME_DIR", root.join("long-runtime-".repeat(12)))
            .output()
            .unwrap();
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        assert!(
            output.status.success(),
            "long-path IPC child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn legacy_show_signal_parses_as_empty_activation_request() {
        assert_eq!(
            parse_signal_payload(SHOW_SIGNAL),
            Some(ActivationRequest::default())
        );
    }

    #[test]
    fn activation_request_payload_round_trips_tokens() {
        let request = ActivationRequest {
            xdg_activation_token: Some("wayland-token/123".to_owned()),
            desktop_startup_id: Some("x11-token:456".to_owned()),
        };

        assert_eq!(
            parse_signal_payload(&signal_payload(&request)),
            Some(request)
        );
    }

    #[test]
    fn empty_activation_request_uses_legacy_show_signal() {
        assert_eq!(signal_payload(&ActivationRequest::default()), SHOW_SIGNAL);
    }

    #[test]
    fn malformed_activation_payload_is_ignored() {
        assert!(parse_signal_payload(b"show-v2\nxdg=not-hex\n").is_some());
        assert_eq!(
            parse_signal_payload(b"show-v2\nxdg=not-hex\n")
                .and_then(|request| request.xdg_activation_token),
            None
        );
    }
}
