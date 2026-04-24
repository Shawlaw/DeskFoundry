# desktop-logger

[English README](./README.en.md)

`desktop-logger` 是一个面向 Windows-first 桌面应用的可复用 Rust crate，适合承载这类通用能力：

- 文件 + 控制台双写日志
- 按大小轮转日志
- 持久化 panic hook
- 人类可读的运行时时间戳
- 适合文件名使用的时间戳

## 版本

- 当前版本：**0.1.0**

## 提供的能力

### `init`

```rust
pub fn init(log_path: &Path, console_mode: bool, max_log_size_mb: u32) -> Result<(), String>
```

初始化全局 logger。

- `log_path`：日志文件路径
- `console_mode`：是否同时镜像输出到 stdout/stderr
- `max_log_size_mb`：轮转前的单文件最大大小

### `set_panic_hook`

```rust
pub fn set_panic_hook(log_path: &Path)
```

注册 panic hook，把 panic 信息和 backtrace 写入同一个日志文件。

### `now_timestamp`

```rust
pub fn now_timestamp() -> String
```

返回这样的时间戳：

```text
2026-04-24 21:30:15.123
```

### `filename_timestamp`

```rust
pub fn filename_timestamp() -> String
```

返回适合文件名使用的时间戳，例如：

```text
20260424_213015123
```

## 示例

```rust
use std::path::Path;

fn main() {
    desktop_logger::init(Path::new("app.log"), false, 2).unwrap();
    desktop_logger::set_panic_hook(Path::new("app.log"));

    log::info!("application started");
}
```

## 说明

- 这个 crate 会通过 `log::set_boxed_logger` 安装 **全局** Rust logger
- `init` 在单个进程内只能调用一次
- 目标场景是小型桌面工具，而不是复杂的多 sink 日志系统
