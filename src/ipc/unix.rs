use super::{ActivationRequest, SHOW_SIGNAL, Signal, SingleInstanceConfig};

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
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
    pub fn accept_signal(&self) -> io::Result<Signal> {
        loop {
            match self.listener.accept() {
                Ok((stream, _address)) => {
                    let mut payload = Vec::new();
                    if stream
                        .take(MAX_SIGNAL_BYTES)
                        .read_to_end(&mut payload)
                        .is_err()
                    {
                        continue;
                    }

                    if let Some(request) = parse_signal_payload(&payload) {
                        return Ok(Signal::Show(request));
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

pub fn claim_or_signal(config: &SingleInstanceConfig) -> io::Result<Startup> {
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
            signal_existing_instance(&paths.socket_path)?;
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

fn signal_existing_instance(socket_path: &PathBuf) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let payload = signal_payload(&ActivationRequest::from_environment());

    loop {
        match UnixStream::connect(socket_path) {
            Ok(mut stream) => return stream.write_all(&payload),
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
            socket_path: dir.join(format!("{app_id}.sock")),
        }
    }

    fn ensure_dir(&self) -> io::Result<()> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_signal_payload, signal_payload};
    use crate::ipc::{ActivationRequest, SHOW_SIGNAL};

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
