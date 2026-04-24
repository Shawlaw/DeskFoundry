# desktop-app-template

`desktop-app-template` is a starter project for Windows-first Rust desktop applications.

It is not meant to be published directly as a product.  
Instead, copy it when creating a new app and then rename the product-specific fields.

## What the template already includes

- `#![windows_subsystem = "windows"]` GUI startup behavior
- logger + panic hook wiring
- portable/AppData config path strategy
- Windows resource metadata via `build.rs`
- icon directory structure
- packaging script
- GitHub Actions tag release workflow
- README / CHANGELOG / LICENSE / config example layout

## Rename checklist

When creating a new app from this template, replace:

- Cargo package name
- product name / executable name
- company / app identity in `build.rs`
- icon files
- README text
- release zip naming
- GitHub workflow release artifact names

## Included SDKs

The template uses DeskFoundry crates via local path dependencies:

- `desktop-logger`
- `desktop-config`
- `desktop-fs`
- `desktop-i18n`

## Build

```bash
cargo xwin build --target x86_64-pc-windows-msvc --release
```

## Package

```bash
./scripts/package_windows_release.sh
```

