# desktop-app-template

[中文 README](./README.md)

Starter scaffold for Windows-first Rust desktop applications built with `egui/eframe`.

This template keeps the release-oriented structure proven in real projects and wires in the shared DeskFoundry crates from the beginning.

## Included pieces

- GUI entry point without default Windows console popup
- runtime logger and panic hook
- portable/AppData config path bootstrap
- `egui` CJK font fallback wiring
- build metadata embedding via `build.rs`
- Windows packaging script
- GitHub Actions release workflow

## Expected customization

Before shipping a real app, replace:

- app name
- executable name
- icons
- README content
- release artifact names
- metadata in `build.rs`

## Build

```bash
cargo xwin build --target x86_64-pc-windows-msvc --release
```

## Package

```bash
./scripts/package_windows_release.sh
```
## Included DeskFoundry crates

- `desktop-config`
- `desktop-egui`
- `desktop-fs`
- `desktop-i18n`
- `desktop-logger`
