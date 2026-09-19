use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub async fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_with_permissions(path, contents, false).await
}

pub async fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_with_permissions(path, contents, true).await
}

async fn write_with_permissions(path: &Path, contents: &[u8], _private: bool) -> io::Result<()> {
    let temp_path = temp_path(path);

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if _private {
        options.mode(0o600);
    }
    let mut file = options.open(&temp_path).await?;
    let write_result = async {
        file.write_all(contents).await?;
        file.sync_all().await?;
        drop(file);

        replace_file(&temp_path, path).await
    }
    .await;

    if write_result.is_err() {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }

    write_result
}

#[cfg(windows)]
async fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from = wide_null(temp_path.as_os_str());
    let to = wide_null(path.as_os_str());
    let result = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
async fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    tokio::fs::rename(temp_path, path).await?;
    sync_parent_dir(path).await
}

#[cfg(not(windows))]
async fn sync_parent_dir(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let dir = tokio::fs::File::open(parent).await?;
    dir.sync_all().await
}

#[cfg(windows)]
fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn temp_path(path: &Path) -> PathBuf {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("fragile-notepad");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    parent.join(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        unique,
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}
