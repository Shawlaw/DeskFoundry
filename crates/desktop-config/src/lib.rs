use directories::ProjectDirs;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct PortableAppPaths {
    pub exe_dir: PathBuf,
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
    pub app_log_path: PathBuf,
    pub portable_mode: bool,
}

pub fn current_exe_dir() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe()
        .map_err(|err| format!("Failed to resolve current exe path: {err}"))?;
    exe_path.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "Executable path has no parent directory: {}",
            exe_path.display()
        )
    })
}

pub fn resolve_portable_app_paths(
    qualifier: &str,
    organization: &str,
    application: &str,
    config_file_name: &str,
    app_log_file_name: &str,
) -> Result<PortableAppPaths, String> {
    let exe_dir = current_exe_dir()?;
    let portable_config_path = exe_dir.join(config_file_name);

    let (config_dir, portable_mode) = if portable_config_path.exists() || is_dir_writable(&exe_dir)
    {
        (exe_dir.clone(), true)
    } else {
        let dirs = ProjectDirs::from(qualifier, organization, application).ok_or_else(|| {
            "Unsupported platform: cannot resolve app config directory".to_owned()
        })?;
        (dirs.config_dir().to_path_buf(), false)
    };

    Ok(PortableAppPaths {
        exe_dir,
        app_log_path: config_dir.join(app_log_file_name),
        config_path: config_dir.join(config_file_name),
        config_dir,
        portable_mode,
    })
}

pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    serde_json::from_slice::<T>(&bytes)
        .map_err(|err| format!("Failed to parse {}: {err}", path.display()))
}

pub fn save_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let content = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("Failed to serialize config: {err}"))?;
    fs::write(path, content).map_err(|err| format!("Failed to write {}: {err}", path.display()))
}

pub fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("Failed to read {}: {err}", path.display()))
}

pub fn write_string(path: &Path, contents: &str) -> Result<(), String> {
    ensure_parent_dir(path)?;
    fs::write(path, contents).map_err(|err| format!("Failed to write {}: {err}", path.display()))
}

pub fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Path has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("Failed to create directory {}: {err}", parent.display()))
}

pub fn is_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(".write-test.tmp");
    match fs::write(&probe, b"probe") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn is_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
        return true;
    }
    bytes.starts_with(b"\\\\")
}

#[cfg(test)]
mod tests {
    use super::{is_windows_absolute, save_pretty_json};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct DemoConfig {
        value: String,
    }

    #[test]
    fn save_pretty_json_writes_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");
        save_pretty_json(
            &path,
            &DemoConfig {
                value: "ok".to_owned(),
            },
        )
        .expect("save");
        let text = std::fs::read_to_string(path).expect("read");
        assert!(text.contains("\"value\": \"ok\""));
    }

    #[test]
    fn windows_absolute_detection_handles_drive_and_unc() {
        assert!(is_windows_absolute(r"C:\demo"));
        assert!(is_windows_absolute(r"\\server\share"));
        assert!(!is_windows_absolute("relative/path"));
    }
}
