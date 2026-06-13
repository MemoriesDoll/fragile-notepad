use super::{Signal, SingleInstanceConfig};

use std::ffi::OsStr;
use std::io;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindowVisible, IsZoomed,
    SW_MAXIMIZE, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

const FOREGROUND_RETRY_TIMEOUT: Duration = Duration::from_secs(2);
const FOREGROUND_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const APP_TITLE: &str = "Fragile Notepad";
const TITLE_SUFFIX: &str = " - Fragile Notepad";

#[derive(Debug)]
pub enum Startup {
    Primary(PrimaryInstance),
    Secondary,
}

#[derive(Debug)]
pub struct PrimaryInstance {
    _mutex: Handle,
}

impl PrimaryInstance {
    pub fn accept_signal(&self) -> io::Result<Signal> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows foreground handoff is handled by the secondary instance",
        ))
    }

    pub fn supports_signals(&self) -> bool {
        false
    }
}

pub fn claim_or_signal(config: &SingleInstanceConfig) -> io::Result<Startup> {
    match claim_instance_lock(config)? {
        InstanceLock::Primary(mutex) => Ok(Startup::Primary(PrimaryInstance { _mutex: mutex })),
        InstanceLock::Secondary => {
            foreground_existing_instance()?;
            Ok(Startup::Secondary)
        }
    }
}

pub fn runtime_dir() -> PathBuf {
    std::env::temp_dir()
}

fn claim_instance_lock(config: &SingleInstanceConfig) -> io::Result<InstanceLock> {
    let mutex_name = wide_null(format!(
        "Local\\{}.single-instance",
        config.sanitized_app_id()
    ));
    let mutex = unsafe { CreateMutexW(ptr::null_mut(), 1, mutex_name.as_ptr()) };

    if mutex.is_null() {
        if unsafe { GetLastError() } == ERROR_ACCESS_DENIED {
            return Ok(InstanceLock::Secondary);
        }

        return Err(io::Error::last_os_error());
    }

    let mutex = Handle(mutex);

    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        Ok(InstanceLock::Secondary)
    } else {
        Ok(InstanceLock::Primary(mutex))
    }
}

enum InstanceLock {
    Primary(Handle),
    Secondary,
}

fn foreground_existing_instance() -> io::Result<()> {
    let deadline = Instant::now() + FOREGROUND_RETRY_TIMEOUT;

    loop {
        if let Some(window) = AppWindow::find_existing() {
            window.restore_and_foreground();
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "existing Fragile Notepad window was not found",
            ));
        }

        std::thread::sleep(FOREGROUND_RETRY_INTERVAL);
    }
}

#[derive(Clone, Copy)]
struct AppWindow(HWND);

impl AppWindow {
    fn find_existing() -> Option<Self> {
        let mut window: HWND = ptr::null_mut();

        unsafe {
            EnumWindows(
                Some(find_fragile_notepad_window),
                &mut window as *mut HWND as LPARAM,
            );
        }

        (!window.is_null()).then_some(Self(window))
    }

    fn restore_and_foreground(self) {
        unsafe {
            if IsZoomed(self.0) != 0 {
                ShowWindow(self.0, SW_MAXIMIZE);
            } else if IsIconic(self.0) != 0 {
                ShowWindow(self.0, SW_RESTORE);
            }

            SetForegroundWindow(self.0);
        }
    }
}

unsafe extern "system" fn find_fragile_notepad_window(hwnd: HWND, lparam: LPARAM) -> i32 {
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }

    let Some(title) = window_title(hwnd) else {
        return 1;
    };

    if is_app_window_title(&title) {
        unsafe {
            *(lparam as *mut HWND) = hwnd;
        }
        return 0;
    }

    1
}

fn window_title(hwnd: HWND) -> Option<String> {
    let title_length = unsafe { GetWindowTextLengthW(hwnd) };
    if title_length <= 0 {
        return None;
    }

    let mut title = vec![0; title_length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) };
    if copied <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&title[..copied as usize]))
}

fn is_app_window_title(title: &str) -> bool {
    title == APP_TITLE || title.ends_with(TITLE_SUFFIX)
}

#[derive(Debug)]
struct Handle(HANDLE);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn wide_null(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::is_app_window_title;

    #[test]
    fn title_matcher_accepts_app_window_titles() {
        assert!(is_app_window_title("Fragile Notepad"));
        assert!(is_app_window_title("Untitled 1 - Fragile Notepad"));
        assert!(is_app_window_title("notes.txt - Fragile Notepad"));
        assert!(is_app_window_title("Settings - Fragile Notepad"));
    }

    #[test]
    fn title_matcher_rejects_unrelated_titles() {
        assert!(!is_app_window_title("Fragile Notepad Backup"));
        assert!(!is_app_window_title("Settings"));
        assert!(!is_app_window_title(""));
    }
}
