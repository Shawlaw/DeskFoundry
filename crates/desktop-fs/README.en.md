# desktop-fs

`desktop-fs` is a small reusable crate for filesystem-adjacent helpers commonly needed by desktop tools.

It focuses on user-facing path and shell behavior rather than full storage abstractions.

## Version

- current version: **0.1.1**

## API overview

### `sanitize_path_component`

```rust
pub fn sanitize_path_component(input: &str) -> String
```

Converts a user/device/file-derived string into a filesystem-friendly path segment. The function preserves Unicode letters and numbers, and replaces whitespace, path separators, control characters, and other unsuitable path-component characters with `_`.

### `format_bytes`

```rust
pub fn format_bytes(bytes: u64) -> String
```

Formats values like:

- `512 B`
- `2.00 KB`
- `15.32 MB`

### `display_path`

```rust
pub fn display_path(path: &Path) -> String
```

Returns a user-friendly display string for a path.

### `display_path_string`

```rust
pub fn display_path_string(path: &str) -> String
```

Useful when a path is already stored as a string.

### `normalize_display_path`

```rust
pub fn normalize_display_path(path: &Path) -> PathBuf
```

Returns a normalized `PathBuf` suitable for display-oriented persistence.

### `open_path`

```rust
pub fn open_path(path: &Path) -> Result<(), String>
```

Opens a file or directory with the platform default shell behavior:

- Windows: `explorer`
- macOS: `open`
- Linux: `xdg-open`

## Windows behavior

This crate also normalizes Windows verbatim paths for display, for example:

- `\\?\F:\logs\demo` -> `F:\logs\demo`
- `\\?\UNC\server\share` -> `\\server\share`

## Example

```rust
let safe = desktop_fs::sanitize_path_component("serial:1/usb");
assert_eq!(safe, "serial_1_usb");

let safe = desktop_fs::sanitize_path_component("小米 14");
assert_eq!(safe, "小米_14");
```

