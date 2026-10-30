//! from keytools-rs crates/kit/src/config.rs

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// `path` if it exists, else the default TOML written to `path` first.
pub fn load_or_create(path: &Path, default_toml: &str) -> Result<String> {
    if !path.exists() {
        std::fs::write(path, default_toml)
            .with_context(|| format!("cannot create default {}", path.display()))?;
    }
    std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

pub fn exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot locate own executable")?;
    Ok(exe
        .parent()
        .context("executable has no parent directory")?
        .to_path_buf())
}

/// `<exe_dir>/<name>`; config and state stay next to the executable.
pub fn exe_dir_file(name: &str) -> Result<PathBuf> {
    Ok(exe_dir()?.join(name))
}

pub fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Replace a small TOML state file without exposing a partially written file.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).with_context(|| format!("cannot write {}", tmp.display()))?;
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        unsafe {
            MoveFileExW(
                &HSTRING::from(tmp.as_os_str()),
                &HSTRING::from(path.as_os_str()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .ok()
            .with_context(|| format!("cannot replace {}", path.display()))?;
        }
    }
    #[cfg(not(windows))]
    std::fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_default_then_loads() {
        let dir = std::env::temp_dir().join(format!("taskbar-kit-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.toml");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            load_or_create(&path, "answer = 42\n").unwrap(),
            "answer = 42\n"
        );
        std::fs::write(&path, "answer = 43\n").unwrap();
        assert_eq!(
            load_or_create(&path, "answer = 42\n").unwrap(),
            "answer = 43\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
