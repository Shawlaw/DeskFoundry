# desktop-egui

[中文 README](./README.md)

`desktop-egui` is a small helper crate for desktop applications built with `egui/eframe`.

It currently focuses on one narrow but common issue:  
**built-in Chinese UI strings often render incorrectly because the default font stack does not cover CJK glyphs.**

## Version

- Current version: **0.1.0**

## API overview

### `install_cjk_fallback_fonts`

```rust
pub fn install_cjk_fallback_fonts(ctx: &egui::Context) -> Result<Option<String>, String>
```

Installs a system CJK font fallback into `egui`.

- returns `Ok(Some(font_name))` when a usable system font is found
- returns `Ok(None)` when no known CJK font is available
- returns `Err(...)` when the selected font file cannot be read

## Current font detection strategy

The crate tries common system fonts per platform:

- Windows: `Microsoft YaHei` / `SimHei` / `SimSun` / `DengXian`
- macOS: `PingFang SC` / `STHeiti` / `Hiragino Sans GB`
- Linux: `Noto Sans CJK` / `WenQuanYi Zen Hei` / `AR PL UKai`

## Scope

This crate only handles:

- `egui` font fallback setup
- discovery of common system CJK fonts

It does **not** handle:

- app-specific i18n strings
- font downloading or installation
- generic theming systems
