use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

/// Atomically replace `path` with `bytes`.
///
/// Writes a sibling temporary file, flushes, fsyncs, and renames it over the
/// destination, so readers never observe a torn or half-written file and a
/// crash mid-write leaves the previous contents intact. Intended for small
/// critical state (configs, caches, manifests) — not for large payloads.
///
/// Notes:
/// - `rename` replaces an existing destination on both Unix and Windows
///   (std uses `MOVEFILE_REPLACE_EXISTING` there).
/// - Directory fsync after the rename is not portable on Windows and is
///   skipped; the file's own data is synced before the swap.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "state".to_owned());
    let temp = directory.join(format!(".{file_name}.tmp-{}", std::process::id()));

    (|| -> io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        Ok(())
    })()
    .inspect_err(|_| {
        // Never leave the scratch file behind on failure.
        let _ = std::fs::remove_file(&temp);
    })
}

pub fn sanitize_path_component(input: &str) -> String {
    let mut sanitized: String = input
        .chars()
        .map(|ch| match ch {
            '-' | '_' | '.' => ch,
            ch if ch.is_alphanumeric() => ch,
            _ => '_',
        })
        .collect();

    if sanitized.is_empty() {
        sanitized.push('_');
    }

    sanitized = sanitized.trim_end_matches([' ', '.']).to_owned();
    if sanitized.is_empty() {
        sanitized.push('_');
    }

    if is_windows_reserved_name(&sanitized) {
        sanitized.insert(0, '_');
    }

    sanitized
}

fn is_windows_reserved_name(component: &str) -> bool {
    let stem = component
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(component);
    let stem = stem.to_ascii_uppercase();

    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0usize;

    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{bytes} {}", UNITS[unit_idx])
    } else {
        format!("{value:.2} {}", UNITS[unit_idx])
    }
}

pub fn display_path(path: &Path) -> String {
    simplify_windows_path(&path.to_string_lossy())
}

pub fn display_path_string(path: &str) -> String {
    simplify_windows_path(path)
}

pub fn normalize_display_path(path: &Path) -> PathBuf {
    PathBuf::from(display_path(path))
}

pub fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut cmd = Command::new("explorer");
        cmd.arg(path);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut cmd = Command::new("open");
        cmd.arg(path);
        cmd
    };

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut cmd = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(path);
        cmd
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

fn simplify_windows_path(path: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", rest);
        }
        if let Some(rest) = path.strip_prefix(r"\\?\") {
            return rest.to_owned();
        }
    }

    path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{atomic_write, display_path_string, format_bytes, sanitize_path_component};
    use std::path::Path;

    #[test]
    fn atomic_write_creates_and_replaces_files() {
        let dir = tempfile_dir();
        let path = dir.join("state.json");

        atomic_write(&path, b"first").expect("initial write");
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        atomic_write(&path, b"second").expect("overwrite");
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn atomic_write_leaves_no_temp_files_behind() {
        let dir = tempfile_dir();
        let path = dir.join("cfg.json");
        atomic_write(&path, b"{}").expect("write");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "temp leftovers: {leftovers:?}");
    }

    #[test]
    fn atomic_write_failure_keeps_previous_contents() {
        let dir = tempfile_dir();
        let path = dir.join("keep.json");
        atomic_write(&path, b"good").expect("write");
        // A directory blocking the temp file name makes the write fail.
        let temp = dir
            .join(".keep.json.tmp-")
            .with_file_name(format!(".keep.json.tmp-{}", std::process::id()));
        std::fs::create_dir(&temp).expect("block temp path");
        let result = atomic_write(&path, b"bad");
        assert!(
            result.is_err(),
            "write should fail while temp path is a dir"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"good");
        std::fs::remove_dir(&temp).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "desktop-fs-atomic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        let _ = Path::new(&dir);
        dir
    }

    #[test]
    fn sanitize_path_component_replaces_unsafe_characters() {
        assert_eq!(sanitize_path_component("serial:1/usb"), "serial_1_usb");
    }

    #[test]
    fn sanitize_path_component_preserves_unicode_letters_and_numbers() {
        assert_eq!(sanitize_path_component("小米 14"), "小米_14");
        assert_eq!(sanitize_path_component("Pixel-éclair-2"), "Pixel-éclair-2");
    }

    #[test]
    fn sanitize_path_component_avoids_empty_and_windows_reserved_names() {
        assert_eq!(sanitize_path_component(""), "_");
        assert_eq!(sanitize_path_component("..."), "_");
        assert_eq!(sanitize_path_component("CON"), "_CON");
        assert_eq!(sanitize_path_component("nul.log"), "_nul.log");
        assert_eq!(sanitize_path_component("COM9"), "_COM9");
    }

    #[test]
    fn format_bytes_uses_readable_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.00 KB");
    }

    #[test]
    fn display_path_string_strips_windows_verbatim_prefix() {
        let raw = r"\\?\F:\logs\demo";
        let cleaned = display_path_string(raw);
        if cfg!(target_os = "windows") {
            assert_eq!(cleaned, r"F:\logs\demo");
        } else {
            assert_eq!(cleaned, raw);
        }
    }
}
