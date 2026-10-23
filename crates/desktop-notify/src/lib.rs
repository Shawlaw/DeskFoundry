//! Portable Windows toast notifications with explicit AppUserModelID
//! registration.
//!
//! Windows toasts must carry an AppUserModelID (AUMID) the shell already
//! knows: installer-based apps get one through a Start Menu shortcut, while
//! portable, installer-less apps must register it themselves under
//! `HKCU\Software\Classes\AppUserModelId\<AUMID>`. This crate encapsulates
//! that mechanism plus the WinRT send path:
//!
//! - AUMID validation (at most 128 UTF-16 code units; no whitespace,
//!   control characters, or path separators — a separator would escape the
//!   AppUserModelId registry branch) and idempotent HKCU registration with
//!   `DisplayName` and an optional `IconUri`.
//! - Materializing an embedded brand-icon PNG into a caller-chosen cache
//!   directory, rewriting only when the bytes actually change.
//! - Sending toasts with COM apartment handling on the calling thread.
//! - Removing the registration key and icon again, treating a missing key
//!   as success.
//!
//! The crate is framework-agnostic (no Tauri, egui, config, or logging
//! coupling) and policy-free: callers decide what to notify, the wording,
//! and which thread sends. On non-Windows targets the registration and
//! removal functions are `Ok(())` stubs and [`show`] returns an error, so
//! cross-platform code compiles unchanged.
//!
//! See `docs/portable-toast-guide.md` in the repository for the background
//! and integration checklist.

use std::path::PathBuf;

/// Windows requires AppUserModelIDs of at most 128 UTF-16 code units.
pub const MAX_APP_USER_MODEL_ID_LEN: usize = 128;

/// File name the brand icon is materialized to inside
/// [`ToastIcon::cache_dir`].
pub const TOAST_ICON_FILE_NAME: &str = "toast-icon.png";

/// One build's toast identity.
///
/// The AppUserModelID must stay stable for the product's lifetime: Windows
/// notification settings group and mute by it, so changing it after release
/// appears to users as a brand-new (unmuted) app. Dev builds should carry
/// their own AppUserModelID and display name so their toasts group
/// separately from the release app.
#[derive(Debug, Clone)]
pub struct ToastApp {
    pub app_user_model_id: String,
    pub display_name: String,
    pub icon: Option<ToastIcon>,
}

/// An embedded brand icon materialized to a caller-owned cache directory.
///
/// The crate does not assume any specific directory layout; callers
/// typically pass a per-app folder under `%LOCALAPPDATA%`.
#[derive(Debug, Clone)]
pub struct ToastIcon {
    /// PNG bytes, usually via `include_bytes!`.
    pub png: &'static [u8],
    /// Directory the icon file is written to; created when missing.
    pub cache_dir: PathBuf,
}

/// The content of one toast. `title` is typically the app display name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastMessage<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub secondary: Option<&'a str>,
}

/// Validates an AppUserModelID against the Windows constraints: non-empty,
/// at most [`MAX_APP_USER_MODEL_ID_LEN`] UTF-16 code units, and free of
/// whitespace, control characters, and path separators. The separator and
/// control-character rules are stricter than Microsoft's "no spaces"
/// wording, because this crate derives registry key paths from the id: a
/// `\` or `/` would address a nested branch, and an embedded NUL truncates
/// the path handed to the registry API.
pub fn validate_app_user_model_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("AppUserModelID must not be empty".to_string());
    }
    // The registry and the shell compare UTF-16, so a non-BMP character
    // counts as two units; chars().count() would understate the length.
    if id.encode_utf16().count() > MAX_APP_USER_MODEL_ID_LEN {
        return Err(format!(
            "AppUserModelID must be at most {MAX_APP_USER_MODEL_ID_LEN} UTF-16 code units"
        ));
    }
    if id
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\\' | '/'))
    {
        return Err(
            "AppUserModelID must not contain whitespace, control characters, or path separators"
                .to_string(),
        );
    }
    Ok(())
}

fn icon_file_path(icon: &ToastIcon) -> PathBuf {
    icon.cache_dir.join(TOAST_ICON_FILE_NAME)
}

/// Materializes the brand icon so the registry `IconUri` and the toast can
/// reference a stable on-disk file. The file is rewritten only when its
/// bytes differ from `icon.png` (a length check alone misses same-size icon
/// updates), and the rewrite goes through a temporary file plus rename so a
/// concurrent reader never observes a torn PNG. Any failure yields `None`:
/// registration then omits `IconUri` and toasts still send with the generic
/// Windows icon.
fn materialize_icon(icon: &ToastIcon) -> Option<PathBuf> {
    let path = icon_file_path(icon);
    let current = std::fs::read(&path).ok();
    if current.as_deref() == Some(icon.png) {
        return Some(path);
    }
    std::fs::create_dir_all(&icon.cache_dir).ok()?;
    let temp = icon
        .cache_dir
        .join(format!(".{TOAST_ICON_FILE_NAME}.tmp-{}", std::process::id()));
    let written = (|| -> Option<()> {
        std::fs::write(&temp, icon.png).ok()?;
        std::fs::rename(&temp, &path).ok()?;
        Some(())
    })();
    if written.is_none() {
        let _ = std::fs::remove_file(&temp);
        return None;
    }
    Some(path)
}

#[cfg(windows)]
pub use windows_impl::{ensure_registration, remove_registration, show};

#[cfg(not(windows))]
pub use other_platform::{ensure_registration, remove_registration, show};

#[cfg(windows)]
mod windows_impl {
    use super::{materialize_icon, validate_app_user_model_id, ToastApp, ToastMessage};

    // The HKCU subkey holding one build's AppUserModelID registration. Null-
    // terminated UTF-16 as the registry APIs expect.
    pub(crate) fn app_user_model_id_registry_subkey(app_user_model_id: &str) -> Vec<u16> {
        "Software\\Classes\\AppUserModelId\\"
            .encode_utf16()
            .chain(app_user_model_id.encode_utf16())
            .chain([0])
            .collect()
    }

    // A WinRT toast is only displayed when its AppUserModelID is known to the
    // shell. Bundled installers create a Start Menu shortcut for this; the
    // portable build instead registers the AppUserModelID key in HKCU (the
    // same mechanism Firefox/Chrome and PowerShell's BurntToast use),
    // optionally including an IconUri pointing at the extracted brand icon.
    fn ensure_registration_impl(
        icon_path: Option<&std::path::Path>,
        app_user_model_id: &str,
        display_name: &str,
    ) -> Result<(), String> {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE,
            REG_OPTION_NON_VOLATILE, REG_EXPAND_SZ,
        };

        fn set_registry_value(
            hkey: windows_sys::Win32::Foundation::HANDLE,
            name: &str,
            value: &str,
        ) -> Result<(), u32> {
            let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
            let value: Vec<u16> = value.encode_utf16().chain([0]).collect();
            let result = unsafe {
                RegSetValueExW(
                    hkey,
                    name.as_ptr(),
                    0,
                    REG_EXPAND_SZ,
                    value.as_ptr().cast(),
                    (value.len() * 2) as u32,
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err(result)
            }
        }

        let subkey = app_user_model_id_registry_subkey(app_user_model_id);

        let mut hkey = std::ptr::null_mut();
        let create_result = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut hkey,
                std::ptr::null_mut(),
            )
        };
        if create_result != 0 {
            return Err(format!(
                "failed to register AppUserModelID key: error {create_result}"
            ));
        }
        let set_results = [
            set_registry_value(hkey, "DisplayName", display_name),
            match icon_path {
                Some(icon_path) => {
                    set_registry_value(hkey, "IconUri", &icon_path.to_string_lossy())
                }
                None => Ok(()),
            },
        ];
        unsafe { RegCloseKey(hkey) };
        for result in set_results {
            if let Err(error) = result {
                return Err(format!("failed to set AppUserModelID value: error {error}"));
            }
        }
        Ok(())
    }

    /// Idempotently registers the app's toast identity in HKCU, updating
    /// `DisplayName` / `IconUri` when the key already exists. Icon
    /// materialization failures are tolerated: registration then omits
    /// `IconUri`.
    pub fn ensure_registration(app: &ToastApp) -> Result<(), String> {
        validate_app_user_model_id(&app.app_user_model_id)?;
        let icon_path = app.icon.as_ref().and_then(materialize_icon);
        ensure_registration_impl(
            icon_path.as_deref(),
            &app.app_user_model_id,
            &app.display_name,
        )
    }

    /// Sends one toast. Registration is ensured first, so callers need no
    /// separate setup step. The underlying WinRT toast object is not `Send`:
    /// create and send it on the same thread, which this function's
    /// signature guarantees.
    pub fn show(app: &ToastApp, message: &ToastMessage<'_>) -> Result<(), String> {
        use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

        // RPC_E_CHANGED_MODE as an unsigned HRESULT: the thread already joined a
        // different apartment model, which WinRT tolerates for toasts.
        const RPC_E_CHANGED_MODE: u32 = 0x8001_0106;

        // WinRT toast calls need an apartment on the calling thread. S_FALSE
        // (1) means the apartment already existed.
        let coinit = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
        let need_uninitialize = coinit == 0 || coinit == 1;
        if coinit < 0 && (coinit as u32) != RPC_E_CHANGED_MODE {
            let hresult = coinit as u32;
            return Err(format!("CoInitializeEx failed: HRESULT 0x{hresult:08X}"));
        }

        let result = (|| {
            validate_app_user_model_id(&app.app_user_model_id)?;
            ensure_registration(app)?;
            let mut toast =
                tauri_winrt_notification::Toast::new(&app.app_user_model_id).title(message.title);
            toast = toast.text1(message.text);
            if let Some(secondary) = message.secondary {
                toast = toast.text2(secondary);
            }
            toast
                .show()
                .map_err(|error| format!("toast failed: {error}"))
        })();

        if need_uninitialize {
            unsafe { CoUninitialize() };
        }
        result
    }

    /// Removes the HKCU AppUserModelID key and the materialized icon so a
    /// user who turns Windows notifications off leaves no registration
    /// footprint behind. A missing key counts as success. The id is
    /// validated first: deletion derives a registry path from it, and an
    /// id carrying a separator, control character, or trailing empty string
    /// must never reach `RegDeleteTreeW`.
    pub fn remove_registration(app: &ToastApp) -> Result<(), String> {
        use windows_sys::Win32::System::Registry::{RegDeleteTreeW, HKEY_CURRENT_USER};

        validate_app_user_model_id(&app.app_user_model_id)?;
        let subkey = app_user_model_id_registry_subkey(&app.app_user_model_id);
        const ERROR_FILE_NOT_FOUND: u32 = 2;
        let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, subkey.as_ptr()) };
        if let Some(icon) = app.icon.as_ref() {
            let _ = std::fs::remove_file(super::icon_file_path(icon));
        }
        match result {
            0 | ERROR_FILE_NOT_FOUND => Ok(()),
            code => Err(format!("RegDeleteTreeW failed: error {code}")),
        }
    }
}

// Nothing is registered outside Windows, so registration and removal
// trivially succeed and sending reports the platform limitation.
#[cfg(not(windows))]
mod other_platform {
    use super::{ToastApp, ToastMessage};

    pub fn ensure_registration(_app: &ToastApp) -> Result<(), String> {
        Ok(())
    }

    pub fn show(_app: &ToastApp, _message: &ToastMessage<'_>) -> Result<(), String> {
        Err("toast notifications are only supported on Windows".to_string())
    }

    pub fn remove_registration(_app: &ToastApp) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_user_model_ids() -> [String; 3] {
        [
            "com.example.app".to_string(),
            "com.example.app.dev".to_string(),
            "DeskFoundry.desktop-notify.test-missing".to_string(),
        ]
    }

    #[test]
    fn validate_accepts_normal_ids_and_rejects_invalid_ones() {
        for id in app_user_model_ids() {
            validate_app_user_model_id(&id).expect("normal id must pass");
        }
        assert!(validate_app_user_model_id("").is_err());
        assert!(validate_app_user_model_id("com example app").is_err());
        // Whitespace beyond the plain space, control characters (including
        // the NUL that would truncate the registry path), and separators
        // that would escape the AppUserModelId branch.
        for id in ["com\texample", "com\nexample", "com.example\0app", "com\\example", "com/example"] {
            assert!(validate_app_user_model_id(id).is_err(), "must reject {id:?}");
        }
        let too_long = "a".repeat(MAX_APP_USER_MODEL_ID_LEN + 1);
        assert!(validate_app_user_model_id(&too_long).is_err());
        let boundary = "a".repeat(MAX_APP_USER_MODEL_ID_LEN);
        assert!(validate_app_user_model_id(&boundary).is_ok());
        // Length is measured in UTF-16 code units: 65 surrogate pairs are
        // 130 units even though they are only 65 chars.
        assert!(validate_app_user_model_id(&"😀".repeat(65)).is_err());
        assert!(validate_app_user_model_id(&"😀".repeat(64)).is_ok());
    }

    #[test]
    fn icon_file_path_joins_the_cache_dir_and_fixed_file_name() {
        let icon = ToastIcon {
            png: b"png",
            cache_dir: std::path::Path::new("C:/cache").to_path_buf(),
        };
        assert_eq!(icon_file_path(&icon), std::path::Path::new("C:/cache/toast-icon.png"));
    }

    #[test]
    fn materialize_icon_creates_rewrites_and_skips_identical_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let icon = ToastIcon {
            png: b"brand-icon-bytes",
            cache_dir: dir.path().to_path_buf(),
        };

        // Missing file: created with the exact bytes.
        let path = materialize_icon(&icon).expect("materialize missing");
        assert_eq!(std::fs::read(&path).unwrap(), b"brand-icon-bytes");

        // Stale content (for example a same-length icon update): rewritten.
        std::fs::write(&path, b"stale-icon-bytes").unwrap();
        materialize_icon(&icon).expect("materialize stale");
        assert_eq!(std::fs::read(&path).unwrap(), b"brand-icon-bytes");

        // Identical content: still succeeds and leaves the file in place.
        materialize_icon(&icon).expect("materialize identical");
        assert_eq!(std::fs::read(&path).unwrap(), b"brand-icon-bytes");

        // The rewrite goes through a temporary file; nothing may linger.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temporary file residue");
    }

    #[test]
    fn materialize_icon_tolerates_an_unusable_cache_dir() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A file occupying the cache-dir path makes create_dir_all fail.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"").unwrap();
        let icon = ToastIcon {
            png: b"png",
            cache_dir: blocker,
        };
        assert!(materialize_icon(&icon).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn registry_subkey_places_each_identity_under_the_aumid_branch() {
        let subkey = |id: &str| {
            let encoded = windows_impl::app_user_model_id_registry_subkey(id);
            String::from_utf16_lossy(&encoded[..encoded.len() - 1])
        };
        let ids = app_user_model_ids();
        assert_eq!(
            subkey(&ids[0]),
            r"Software\Classes\AppUserModelId\com.example.app"
        );
        assert_eq!(
            subkey(&ids[1]),
            r"Software\Classes\AppUserModelId\com.example.app.dev"
        );
        assert_ne!(subkey(&ids[0]), subkey(&ids[1]));
    }

    #[cfg(windows)]
    #[test]
    fn remove_registration_treats_a_missing_key_as_success() {
        let app = ToastApp {
            app_user_model_id: "DeskFoundry.desktop-notify.test-missing".to_string(),
            display_name: "desktop-notify test".to_string(),
            icon: None,
        };
        remove_registration(&app).expect("missing key must not be an error");
    }

    #[cfg(windows)]
    #[test]
    fn remove_registration_rejects_ids_that_would_escape_the_registry_branch() {
        for id in ["", "com.example\\nested", "com.example\0app"] {
            let app = ToastApp {
                app_user_model_id: id.to_string(),
                display_name: "desktop-notify test".to_string(),
                icon: None,
            };
            assert!(
                remove_registration(&app).is_err(),
                "deletion must refuse the id {id:?} without touching the registry"
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_stubs_register_nothing_and_refuse_to_send() {
        let app = ToastApp {
            app_user_model_id: "com.example.app".to_string(),
            display_name: "Example".to_string(),
            icon: None,
        };
        assert!(ensure_registration(&app).is_ok());
        assert!(remove_registration(&app).is_ok());
        assert!(show(&app, &ToastMessage { title: "t", text: "x", secondary: None }).is_err());
    }
}
