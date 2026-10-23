# Portable Windows Toast Notification Specification

[中文版](./portable-toast-guide.md)

This guide defines DeskFoundry's Windows toast capability: the `desktop-notify` crate and the
AppUserModelID (AUMID) registration mechanism behind it. It targets **portable ZIP /
installer-less** Windows desktop apps; MSI, NSIS, macOS, and Linux are out of scope (on
non-Windows platforms the crate compiles as a stub).

- The SDK is independent of Tauri, egui, and any UI framework; it reads no config and writes
  no logs. The app owns notification policy, wording, and threading.
- Windows toasts must carry an AUMID. Installers register it via a Start Menu shortcut;
  portable apps instead write `HKCU\Software\Classes\AppUserModelId\<AUMID>` with
  `DisplayName` and `IconUri` values — the same mechanism Firefox, Chrome, and BurntToast
  use. Without it, toasts fail or land under the wrong identity.
- An AUMID must be at most 128 characters, contain no spaces, and **never change once
  released**: Windows notification settings and per-app mute state hang off it. Dev builds
  should use a separate AUMID (for example a `.dev` suffix) and display name so they group
  separately and can be muted independently.
- The brand icon must be materialized to a stable on-disk file for `IconUri`; compare bytes
  (not just length) before rewriting, and treat icon failure as non-fatal — register and
  send without `IconUri` instead.
- `show` initializes a COM apartment (`CoInitializeEx(MTA)`, tolerating
  `RPC_E_CHANGED_MODE`) on the calling thread; the underlying toast object is not `Send`,
  so create and send on the same thread. `show` re-ensures registration idempotently.
- When the user turns notifications off, remove the registration key tree and the
  materialized icon (`remove_registration`); a missing key counts as success.
- Beware cross-platform notification libraries that fall back to PowerShell's AUMID when no
  app id is set (notify-rust does this): toasts then appear to come from PowerShell. Always
  register and pass your own AUMID.

See the Chinese guide for the full integration example, troubleshooting matrix (Focus
Assist, muted apps, changed AUMIDs, stale icons), and testing requirements. The public API
is `validate_app_user_model_id`, `ensure_registration`, `show`, and `remove_registration`
over the `ToastApp` / `ToastIcon` / `ToastMessage` types.
