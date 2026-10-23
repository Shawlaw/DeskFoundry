# desktop-notify

[English README](./README.en.md)

`desktop-notify` 为 **portable / 无安装器** 的 Windows 桌面应用提供 Windows toast
通知：显式的 AppUserModelID（AUMID）注册、品牌图标物化、WinRT 发送与注销清理。

它解决的是 portable 应用没有开始菜单快捷方式、Windows 不认识其 AUMID、toast 无法
正常弹出或归组的问题（机制背景见
[`docs/portable-toast-guide.md`](../../docs/portable-toast-guide.md)）。

## 版本

- 当前版本：**0.1.0**

## 设计边界

- 不绑定 Tauri、egui 或任何 UI 框架；不读取配置、不写日志、不感知事件模型。
- 通知策略（何时通知、文案、过滤、线程选择）全部由调用方决定。
- 非 Windows 平台提供可编译的 stub：`ensure_registration` / `remove_registration`
  返回 `Ok(())`，`show` 返回错误。

## API 概览

### `ToastApp` / `ToastIcon` / `ToastMessage`

```rust
pub struct ToastApp {
    pub app_user_model_id: String,
    pub display_name: String,
    pub icon: Option<ToastIcon>,
}

pub struct ToastIcon {
    pub png: &'static [u8],      // 通常来自 include_bytes!
    pub cache_dir: PathBuf,      // 图标物化目录，由调用方决定
}

pub struct ToastMessage<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub secondary: Option<&'a str>,
}
```

AUMID 在产品生命周期内必须保持稳定（Windows 通知设置按它分组与静音）；开发构建应
使用独立 AUMID 与显示名。

### `validate_app_user_model_id`

```rust
pub fn validate_app_user_model_id(id: &str) -> Result<(), String>;
```

校验 Windows 约束：非空、不超过 128 个 UTF-16 字符，且不含空白、控制字符与路径分隔符（`\`、`/` 会逃逸出 AppUserModelId 注册表分支，控制字符会截断注册表路径，因此比微软"不含空格"的措辞更严格）。

### `ensure_registration`

```rust
pub fn ensure_registration(app: &ToastApp) -> Result<(), String>;
```

幂等地在 `HKCU\Software\Classes\AppUserModelId\<AUMID>` 写入 `DisplayName` 与可选的
`IconUri`；图标物化失败时跳过 `IconUri` 而不是失败。

### `show`

```rust
pub fn show(app: &ToastApp, message: &ToastMessage<'_>) -> Result<(), String>;
```

发送一条 toast，内部先确保注册。底层 `Toast` 对象不是 `Send`：本函数在调用线程内
完成创建与发送；COM 套间初始化（`CoInitializeEx(MTA)`，容忍
`RPC_E_CHANGED_MODE`）已内置。

### `remove_registration`

```rust
pub fn remove_registration(app: &ToastApp) -> Result<(), String>;
```

删除 AUMID 注册键与物化图标；键不存在视为成功。适合在用户关闭通知通道时调用。

## 示例

```rust
use desktop_notify::{show, ToastApp, ToastIcon, ToastMessage};
use std::path::PathBuf;

fn toast_app() -> ToastApp {
    let icon = std::env::var_os("LOCALAPPDATA").map(|dir| ToastIcon {
        png: include_bytes!("../icons/128x128.png"),
        cache_dir: PathBuf::from(dir).join("MyApp"),
    });
    ToastApp {
        app_user_model_id: "com.example.myapp".to_string(),
        display_name: "MyApp".to_string(),
        icon,
    }
}

let app = toast_app();
show(
    &app,
    &ToastMessage {
        title: &app.display_name,
        text: "采集已完成",
        secondary: None,
    },
)?;
```

## 依赖说明

Windows 侧使用 [`tauri-winrt-notification`](https://docs.rs/tauri-winrt-notification)
0.7.x（Tauri 团队维护的 `winrt-notification` 分支）。它本身不依赖 Tauri，名称中的
"tauri" 只是来源标记。注册表与 COM 调用使用 `windows-sys`。

## 更新记录

- v0.1.0：初始公开能力 —— AUMID 校验（≤128 个 UTF-16 字符；拒绝空白、控制字符与路径分隔符）、HKCU 注册、图标物化（按字节比较更新、临时文件 + rename 原子替换）、
  WinRT toast 发送（内置 COM 套间处理）、注销清理、非 Windows stub。
