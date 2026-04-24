# desktop-app-template

Starter scaffold for Windows-first Rust desktop applications built with `egui/eframe`.

This template keeps the release-oriented structure proven in real projects and wires in the shared DeskFoundry crates from the beginning.

## Included pieces

- GUI entry point without default Windows console popup
- runtime logger and panic hook
- portable/AppData config path bootstrap
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

