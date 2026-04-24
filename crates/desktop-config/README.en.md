# desktop-config

`desktop-config` is a small reusable crate for Windows-first Rust desktop apps.

It focuses on the low-level parts that multiple apps can share safely:

- resolving executable directory
- portable vs AppData config path selection
- JSON config read/write helpers
- plain text config migration helpers
- writable-directory checks
- Windows-style absolute path detection

## Version

- current version: **0.1.0**

## API overview

### `PortableAppPaths`

```rust
pub struct PortableAppPaths {
    pub exe_dir: PathBuf,
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
    pub app_log_path: PathBuf,
    pub portable_mode: bool,
}
```

### `current_exe_dir`

```rust
pub fn current_exe_dir() -> Result<PathBuf, String>
```

Returns the executable directory of the current process.

### `resolve_portable_app_paths`

```rust
pub fn resolve_portable_app_paths(
    qualifier: &str,
    organization: &str,
    application: &str,
    config_file_name: &str,
    app_log_file_name: &str,
) -> Result<PortableAppPaths, String>
```

Resolves a path strategy with:

- executable-directory-first portable mode
- automatic AppData fallback when the executable directory is not writable

### `load_json`

```rust
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, String>
```

Loads typed JSON from disk.

### `save_pretty_json`

```rust
pub fn save_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String>
```

Writes pretty-formatted JSON and creates parent directories if needed.

### `read_to_string` / `write_string`

Useful when an app has custom migration logic and wants to operate on raw config text before parsing.

### `is_dir_writable`

```rust
pub fn is_dir_writable(dir: &Path) -> bool
```

Checks whether a directory can be used for portable config/log storage.

### `is_windows_absolute`

```rust
pub fn is_windows_absolute(path: &str) -> bool
```

Recognizes:

- `C:\...`
- `D:/...`
- `\\server\share\...`

## Example

```rust
let paths = desktop_config::resolve_portable_app_paths(
    "com",
    "ExampleOrg",
    "ExampleApp",
    "config.json",
    ".example.log",
)?;
```

## Scope

This crate intentionally does **not** define your app's config schema.

It is meant to provide the reusable infrastructure layer under each app-specific `AppConfig`.

