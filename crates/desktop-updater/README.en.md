# desktop-updater

`desktop-updater` provides signed portable-ZIP updates for Windows-first desktop applications: it checks a GitHub Raw manifest, verifies an Ed25519 signature and SHA-256, downloads the asset, replaces declared release files through a helper process, and retains rollback copies until the new app acknowledges startup.

It is independent of Tauri, egui, and UI frameworks. An application owns UI state, calls `acknowledge_if_requested` at startup, and ships a tiny updater-helper binary.

Version 1 supports Windows x64 portable ZIPs only.
