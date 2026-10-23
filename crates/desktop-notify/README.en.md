# desktop-notify

[中文 README](./README.md)

`desktop-notify` provides Windows toast notifications for **portable /
installer-less** Windows desktop apps: explicit AppUserModelID (AUMID)
registration, brand-icon materialization, the WinRT send path, and removal
cleanup.

It exists because portable apps ship no Start Menu shortcut, so Windows does
not know their AUMID and toasts fail or group under the wrong identity (see
[`docs/portable-toast-guide.en.md`](../../docs/portable-toast-guide.en.md) for
the background).

## Version

- Current version: **0.1.0**

## Design boundaries

- No Tauri, egui, or UI-framework coupling; reads no config, writes no logs,
  and knows nothing about event models.
- Notification policy (when to notify, wording, filtering, threading) stays
  with the caller.
- Non-Windows targets compile as stubs: `ensure_registration` /
  `remove_registration` return `Ok(())`, `show` returns an error.

## API overview

```rust
pub struct ToastApp {
    pub app_user_model_id: String,
    pub display_name: String,
    pub icon: Option<ToastIcon>,
}

pub struct ToastIcon {
    pub png: &'static [u8],      // usually via include_bytes!
    pub cache_dir: PathBuf,      // caller-chosen icon cache directory
}

pub struct ToastMessage<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub secondary: Option<&'a str>,
}

pub fn validate_app_user_model_id(id: &str) -> Result<(), String>;
pub fn ensure_registration(app: &ToastApp) -> Result<(), String>;
pub fn show(app: &ToastApp, message: &ToastMessage<'_>) -> Result<(), String>;
pub fn remove_registration(app: &ToastApp) -> Result<(), String>;
```

The AUMID must stay stable for the product's lifetime (Windows notification
settings group and mute by it); dev builds should carry their own AUMID and
display name.

`show` ensures registration first and handles the COM apartment
(`CoInitializeEx(MTA)`, tolerating `RPC_E_CHANGED_MODE`) on the calling
thread; the underlying toast object is not `Send`, so create and send happen
on one thread. `remove_registration` deletes the HKCU key and the
materialized icon, treating a missing key as success — call it when the user
turns notifications off.

## Example

```rust
use desktop_notify::{show, ToastApp, ToastIcon, ToastMessage};
use std::path::PathBuf;

let icon = std::env::var_os("LOCALAPPDATA").map(|dir| ToastIcon {
    png: include_bytes!("../icons/128x128.png"),
    cache_dir: PathBuf::from(dir).join("MyApp"),
});
let app = ToastApp {
    app_user_model_id: "com.example.myapp".to_string(),
    display_name: "MyApp".to_string(),
    icon,
};
show(
    &app,
    &ToastMessage {
        title: &app.display_name,
        text: "Capture finished",
        secondary: None,
    },
)?;
```

## Dependencies

On Windows the crate uses
[`tauri-winrt-notification`](https://docs.rs/tauri-winrt-notification) 0.7.x
(the Tauri team's maintained fork of `winrt-notification`; it does not depend
on Tauri — the name only marks its origin). Registry and COM calls use
`windows-sys`.

## Changelog

- v0.1.0: initial public capability — AUMID validation (at most 128 UTF-16
  code units; rejects whitespace, control characters, and path separators),
  HKCU registration,
  icon materialization with byte-compare updates via write-then-rename,
  WinRT toast send with
  built-in COM apartment handling, removal cleanup, non-Windows stubs.
