# desktop-fs

[English README](./README.en.md)

`desktop-fs` 是一个小型可复用 crate，聚焦桌面工具里常见的文件系统周边辅助能力。

它更关注用户可见的路径展示与系统壳层行为，而不是做完整的存储抽象。

## 版本

- 当前版本：**0.1.0**

## API 概览

### `sanitize_path_component`

```rust
pub fn sanitize_path_component(input: &str) -> String
```

把来自用户 / 设备 / 文件名的字符串转换成更安全的路径片段。

### `format_bytes`

```rust
pub fn format_bytes(bytes: u64) -> String
```

把字节数格式化成：

- `512 B`
- `2.00 KB`
- `15.32 MB`

### `display_path`

```rust
pub fn display_path(path: &Path) -> String
```

返回适合展示给用户的路径字符串。

### `display_path_string`

```rust
pub fn display_path_string(path: &str) -> String
```

适合路径本身已经以字符串形式保存的场景。

### `normalize_display_path`

```rust
pub fn normalize_display_path(path: &Path) -> PathBuf
```

返回一个适合“展示导向持久化”的规范化 `PathBuf`。

### `open_path`

```rust
pub fn open_path(path: &Path) -> Result<(), String>
```

使用当前平台默认壳层行为打开文件或目录：

- Windows：`explorer`
- macOS：`open`
- Linux：`xdg-open`

## Windows 行为

这个 crate 也会把 Windows 的 verbatim path 规范化成更适合展示的形式，例如：

- `\\?\F:\logs\demo` -> `F:\logs\demo`
- `\\?\UNC\server\share` -> `\\server\share`

## 示例

```rust
let safe = desktop_fs::sanitize_path_component("serial:1/usb");
assert_eq!(safe, "serial_1_usb");
```
