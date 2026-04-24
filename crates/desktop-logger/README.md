# desktop-logger

`desktop-logger` is a reusable Rust crate for Windows-first desktop applications that need:

- file + console dual logging
- log rotation by size
- panic hook persistence
- human-readable runtime timestamps
- filesystem-friendly filename timestamps

## Version

- current version: **0.1.0**

## What it provides

### `init`

```rust
pub fn init(log_path: &Path, console_mode: bool, max_log_size_mb: u32) -> Result<(), String>
```

Initializes the global logger.

- `log_path`: target log file path
- `console_mode`: whether to also mirror logs to stdout/stderr
- `max_log_size_mb`: max file size before rotation

### `set_panic_hook`

```rust
pub fn set_panic_hook(log_path: &Path)
```

Registers a panic hook that writes panic information and backtrace to the same log file.

### `now_timestamp`

```rust
pub fn now_timestamp() -> String
```

Returns a timestamp like:

```text
2026-04-24 21:30:15.123
```

### `filename_timestamp`

```rust
pub fn filename_timestamp() -> String
```

Returns a timestamp safe for filenames, for example:

```text
20260424_213015123
```

## Example

```rust
use std::path::Path;

fn main() {
    desktop_logger::init(Path::new("app.log"), false, 2).unwrap();
    desktop_logger::set_panic_hook(Path::new("app.log"));

    log::info!("application started");
}
```

## Notes

- this crate installs the **global** Rust logger via `log::set_boxed_logger`
- call `init` only once per process
- intended for small desktop tools rather than complex multi-sink logging pipelines
