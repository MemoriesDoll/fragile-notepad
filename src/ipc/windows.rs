use super::{Signal, SingleInstanceConfig};

use std::ffi::OsStr;
use std::io;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_NO_DATA, ERROR_PIPE_BUSY,
    ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenSessionId, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile,
    WriteFile,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId,
    PIPE_NOWAIT, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    SetNamedPipeHandleState,
};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

#[derive(Debug)]
pub enum Startup {
    Primary(PrimaryInstance),
    Secondary,
}

#[derive(Debug)]
pub struct PrimaryInstance {
    _mutex: Handle,
    pipe: Handle,
}

impl PrimaryInstance {
    #[cfg(test)]
    pub fn accept_signal(&self) -> io::Result<Signal> {
        self.accept_signal_with(|_| true)
    }

    pub fn accept_signal_with(&self, admit: impl FnOnce(&Signal) -> bool) -> io::Result<Signal> {
        let mut admit = Some(admit);
        loop {
            if unsafe { ConnectNamedPipe(self.pipe.0, ptr::null_mut()) } == 0 {
                let error = unsafe { GetLastError() };
                if error == ERROR_PIPE_LISTENING {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
                if error != ERROR_PIPE_CONNECTED && error != ERROR_NO_DATA {
                    return Err(io::Error::from_raw_os_error(error as i32));
                }
            }
            let result = read_frame(self.pipe.0).and_then(|frame| super::decode_signal(&frame));
            if let Ok(signal) = &result {
                let accepted = admit.take().expect("one signal per admission")(signal);
                let _ = pipe_write(
                    self.pipe.0,
                    &[u8::from(accepted)],
                    Instant::now() + Duration::from_secs(2),
                );
                let mut acknowledged = [0];
                let _ = pipe_read_exact(
                    self.pipe.0,
                    &mut acknowledged,
                    Instant::now() + Duration::from_secs(2),
                );
            }
            unsafe {
                DisconnectNamedPipe(self.pipe.0);
            }
            if let Ok(signal) = result {
                return Ok(signal);
            }
        }
    }

    pub fn supports_signals(&self) -> bool {
        true
    }
}

pub fn claim_or_signal(config: &SingleInstanceConfig, files: &[PathBuf]) -> io::Result<Startup> {
    let (sid, session) = current_user_identity()?;
    let scoped = format!("{}.{}.{}", config.sanitized_app_id(), sid, session);
    let pipe_name = wide_null(format!(r"\\.\pipe\{scoped}.files"));
    match claim_instance_lock(&scoped)? {
        InstanceLock::Primary(mutex) => {
            let pipe = create_pipe(&pipe_name, &sid)?;
            Ok(Startup::Primary(PrimaryInstance {
                _mutex: mutex,
                pipe,
            }))
        }
        InstanceLock::Secondary => {
            send_files(&pipe_name, files)?;
            Ok(Startup::Secondary)
        }
    }
}

pub fn runtime_dir() -> PathBuf {
    std::env::temp_dir()
}

fn claim_instance_lock(scoped_name: &str) -> io::Result<InstanceLock> {
    let mutex_name = wide_null(format!("Local\\{}.single-instance", scoped_name));
    let mutex = unsafe { CreateMutexW(ptr::null_mut(), 1, mutex_name.as_ptr()) };

    if mutex.is_null() {
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

fn current_user_identity() -> io::Result<(String, u32)> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Handle(token);
    let mut session = 0u32;
    let mut length = 0;
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenSessionId,
            (&mut session as *mut u32).cast(),
            4,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut length = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut length);
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            storage.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut length = 0;
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
    }
    let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    unsafe {
        LocalFree(text.cast());
    }
    Ok((sid, session))
}

fn create_pipe(name: &[u16], sid: &str) -> io::Result<Handle> {
    let sddl = wide_null(format!("D:P(A;;GA;;;{sid})"));
    let mut descriptor = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let pipe = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            2000,
            &attributes,
        )
    };
    let error = io::Error::last_os_error();
    unsafe {
        LocalFree(descriptor);
    }
    if pipe == INVALID_HANDLE_VALUE {
        Err(error)
    } else {
        Ok(Handle(pipe))
    }
}

fn send_files(name: &[u16], files: &[PathBuf]) -> io::Result<()> {
    let frame = super::encode_signal(files, &super::ActivationRequest::default())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let pipe = loop {
        let pipe = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if pipe != INVALID_HANDLE_VALUE {
            break Handle(pipe);
        }
        let error = unsafe { GetLastError() };
        if !matches!(error, ERROR_FILE_NOT_FOUND | ERROR_PIPE_BUSY) || Instant::now() >= deadline {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mode = PIPE_READMODE_BYTE | PIPE_NOWAIT;
    if unsafe { SetNamedPipeHandleState(pipe.0, &mode, ptr::null(), ptr::null()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut pid = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.0, &mut pid) } != 0 {
        unsafe {
            AllowSetForegroundWindow(pid);
        }
    }
    pipe_write(pipe.0, &frame, deadline)?;
    let mut acknowledgement = [0];
    let admission_deadline = Instant::now() + Duration::from_secs(7);
    pipe_read_exact(pipe.0, &mut acknowledgement, admission_deadline)?;
    pipe_write(pipe.0, &[2], admission_deadline)?;
    if acknowledgement != [1] {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "The running application could not accept the files (it may be closing). Retry after it exits.",
        ));
    }
    Ok(())
}

fn read_frame(pipe: HANDLE) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut header = [0; 8];
    pipe_read_exact(pipe, &mut header, deadline)?;
    if &header[..4] != super::FRAME_MAGIC {
        return Err(super::invalid_signal("invalid IPC header"));
    }
    let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if length > super::MAX_SIGNAL_BYTES - 8 {
        return Err(super::invalid_signal("IPC frame exceeds size limit"));
    }
    let mut frame = Vec::from(header);
    frame.resize(length + 8, 0);
    pipe_read_exact(pipe, &mut frame[8..], deadline)?;
    Ok(frame)
}

fn pipe_read_exact(pipe: HANDLE, mut bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        let mut read = 0;
        let success = unsafe {
            ReadFile(
                pipe,
                bytes.as_mut_ptr(),
                bytes.len().min(64 * 1024) as u32,
                &mut read,
                ptr::null_mut(),
            )
        };
        if success != 0 && read > 0 {
            bytes = &mut bytes[read as usize..];
            continue;
        }
        let error = unsafe { GetLastError() };
        if success == 0 && error != ERROR_NO_DATA {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC read timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn pipe_write(pipe: HANDLE, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        let mut written = 0;
        let success = unsafe {
            WriteFile(
                pipe,
                bytes.as_ptr(),
                bytes.len().min(64 * 1024) as u32,
                &mut written,
                ptr::null_mut(),
            )
        };
        if success != 0 && written > 0 {
            bytes = &bytes[written as usize..];
            continue;
        }
        let error = unsafe { GetLastError() };
        if success == 0 && !matches!(error, ERROR_NO_DATA | ERROR_PIPE_BUSY) {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "IPC write timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
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
    use super::*;

    #[test]
    fn named_pipe_delivers_multiple_native_file_requests() {
        use std::os::windows::ffi::OsStringExt;
        let config = SingleInstanceConfig::new(format!("ipc-native-test-{}", std::process::id()));
        let Startup::Primary(primary) = claim_or_signal(&config, &[]).unwrap() else {
            panic!("primary");
        };
        let file = PathBuf::from(std::ffi::OsString::from_wide(&[
            67, 58, 92, 0xd800, 46, 116, 120, 116,
        ]));
        let mut files = vec![file];
        files
            .extend((0..100).map(|index| {
                std::env::temp_dir().join(format!("{index}-{}.txt", "x".repeat(800)))
            }));
        let expected = files.clone();
        let receiver = std::thread::spawn(move || {
            assert_eq!(
                primary.accept_signal().unwrap(),
                Signal::OpenFiles(expected, super::super::ActivationRequest::default())
            );
            assert_eq!(
                primary.accept_signal().unwrap(),
                Signal::Show(super::super::ActivationRequest::default())
            );
        });
        assert!(matches!(
            claim_or_signal(&config, &files).unwrap(),
            Startup::Secondary
        ));
        assert!(matches!(
            claim_or_signal(&config, &[]).unwrap(),
            Startup::Secondary
        ));
        receiver.join().unwrap();
    }
}
