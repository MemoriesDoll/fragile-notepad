//! Small-file raw byte cache.

use crate::platform::paths;

use std::io;
use std::path::{Path, PathBuf};

pub const SMALL_FILE_CACHE_LIMIT: u64 = 1024 * 1024;

const FILE_CACHE_DIR: &str = "file-cache";
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmallFileCacheUpdate {
    Cached { cache_path: PathBuf, bytes: u64 },
    SkippedLarge { bytes: u64 },
    SkippedNotFile,
    Unavailable,
}

pub async fn update_small_file_cache(path: PathBuf) -> io::Result<SmallFileCacheUpdate> {
    let Some(cache_dir) = paths::cache_dir().map(|path| path.join(FILE_CACHE_DIR)) else {
        return Ok(SmallFileCacheUpdate::Unavailable);
    };

    update_small_file_cache_in(cache_dir, path, SMALL_FILE_CACHE_LIMIT).await
}

pub async fn update_small_file_cache_in(
    cache_dir: PathBuf,
    source_path: PathBuf,
    max_bytes: u64,
) -> io::Result<SmallFileCacheUpdate> {
    let metadata = tokio::fs::metadata(&source_path).await?;
    let cache_path = cache_file_path(&cache_dir, &source_path);

    if !metadata.is_file() {
        remove_stale_cache_file(&cache_path).await?;
        return Ok(SmallFileCacheUpdate::SkippedNotFile);
    }

    if metadata.len() > max_bytes {
        remove_stale_cache_file(&cache_path).await?;
        return Ok(SmallFileCacheUpdate::SkippedLarge {
            bytes: metadata.len(),
        });
    }

    let bytes = tokio::fs::read(&source_path).await?;
    if bytes.len() as u64 > max_bytes {
        remove_stale_cache_file(&cache_path).await?;
        return Ok(SmallFileCacheUpdate::SkippedLarge {
            bytes: bytes.len() as u64,
        });
    }

    super::atomic_write::write(&cache_path, &bytes).await?;

    Ok(SmallFileCacheUpdate::Cached {
        cache_path,
        bytes: bytes.len() as u64,
    })
}

pub fn cache_file_path(cache_dir: &Path, source_path: &Path) -> PathBuf {
    let key = cache_key(source_path);

    cache_dir.join(&key[..2]).join(format!("{key}.bin"))
}

pub fn cache_key(source_path: &Path) -> String {
    let mut hash = FNV_OFFSET;
    for byte in source_path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{hash:016x}")
}

async fn remove_stale_cache_file(path: &Path) -> io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable_for_same_path() {
        let path = Path::new("C:\\notes\\draft.txt");

        assert_eq!(cache_key(path), cache_key(path));
        assert_eq!(cache_key(path).len(), 16);
    }

    #[test]
    fn cache_file_path_uses_injected_cache_dir_and_stable_key() {
        let cache_dir = Path::new("cache-root");
        let source_path = Path::new("notes/draft.txt");
        let key = cache_key(source_path);

        assert_eq!(
            cache_file_path(cache_dir, source_path),
            cache_dir.join(&key[..2]).join(format!("{key}.bin"))
        );
    }
}
