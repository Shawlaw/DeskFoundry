# Portable Windows Toast 通知接入规范

[English version](./portable-toast-guide.en.md)

本文定义 DeskFoundry 的 Windows toast 通知能力：`desktop-notify` crate 与它背后的
AppUserModelID（AUMID）注册机制。它面向 **portable ZIP / 无安装器** 的 Windows 桌面
应用；MSI、NSIS、macOS 与 Linux 不在此协议范围内（非 Windows 平台 crate 提供可编译的
stub）。

## 设计边界

- `desktop-notify` 不依赖 Tauri、egui 或任何 UI 框架，不读取配置文件，不写日志。
- 应用负责决定何时通知、通知文案、在哪个线程发送；crate 只负责身份注册与投递。
- 事件模型、通知过滤、通知历史、模板渲染都不属于本 crate。

## 背景：portable 应用为什么弹不出 toast

Windows 的 toast 必须携带一个 AUMID，shell 用它在操作中心为通知分组、展示应用名，
并在「通知设置」里提供每应用开关。安装器会通过开始菜单快捷方式的
`System.AppUserModelID` 属性完成这件事；portable 应用没有安装器和快捷方式，直接发送
会失败或被归到错误身份下。

解决方式是手动在 HKCU 写入注册键（Firefox、Chrome 与 PowerShell 的 BurntToast 使用
同一机制）：

```text
HKCU\Software\Classes\AppUserModelId\<AUMID>
    DisplayName  (REG_EXPAND_SZ)  显示名，用于操作中心头部与通知设置
    IconUri      (REG_EXPAND_SZ)  品牌图标的本地文件 URI
```

## AUMID 约束

- 长度不超过 128 个 UTF-16 码元（code unit，非 BMP 字符每个占 2 个码元）；不能包含
  空白、控制字符或路径分隔符（正反斜杠）——
  分隔符会逃逸出 `AppUserModelId` 注册表分支，控制字符（尤其 NUL）会截断传给注册表
  API 的路径。`desktop-notify` 的校验比微软“不含空格”的官方措辞更严，就是为了挡住
  这两类输入。
- **AUMID 在产品生命周期内必须保持稳定**：Windows 通知设置按它分组，用户对它的
  静音/优先级配置都挂在它上面，改了 AUMID 等于换了一个新应用，旧的静音设置全部
  失效。发布 AUMID 一旦上线就不要再改。
- 开发构建应使用独立 AUMID（例如加 `.dev` 后缀）与独立显示名，这样 dev 版通知与
  正式版在操作中心分开分组，可以单独静音，也不会污染正式版的注册键。
- `desktop_notify::validate_app_user_model_id` 在发送前强制校验这些约束。

## 品牌图标物化

`IconUri` 必须指向一个稳定存在的本地文件。推荐做法：

- 把品牌 PNG（约 128×128）用 `include_bytes!` 编进二进制。
- 发送前把它物化到用户可写目录（如 `%LOCALAPPDATA%\<AppName>\toast-icon.png`）。
- 重写前先做**字节比较**：只按文件长度判断会在图标内容变化、长度不变时漏更新。
- 图标物化失败不应让通知失败：退回无 `IconUri` 的注册，toast 仍可发送。

## COM 套间与线程

- WinRT toast 调用要求调用线程已初始化 COM 套间。`desktop_notify::show` 会在发送
  线程首次使用时调用 `CoInitializeEx(MTA)`，并容忍 `RPC_E_CHANGED_MODE`（线程已加入其他
  套间模型，toast 仍可用）；套间由线程本地守卫持有，并在线程退出时按需
  `CoUninitialize`，避免连续发送时反复初始化 / 清理导致 WinRT 后端崩溃。
- 底层 `Toast` 对象不是 `Send`：必须**在同一线程内创建并发送**。crate 的 `show`
  签名保证了这一点；应用如果用后台线程投递，就在该线程内完整调用 `show`。

## 注销清理

用户关闭通知通道时，应删除 AUMID 注册键与物化图标，不在用户机器上留下残留：

- 删除 `HKCU\Software\Classes\AppUserModelId\<AUMID>` 整棵子树。
- 删除物化的图标文件。
- 键不存在视为成功——清理是 best-effort，失败只记录，不应让配置保存失败。

## 应用接入

```rust
use desktop_notify::{show, remove_registration, ToastApp, ToastIcon, ToastMessage};

fn toast_app() -> ToastApp {
    let icon = std::env::var_os("LOCALAPPDATA").map(|dir| ToastIcon {
        png: include_bytes!("../icons/128x128.png"),
        cache_dir: std::path::PathBuf::from(dir).join("MyApp"),
    });
    ToastApp {
        app_user_model_id: "com.example.myapp".to_string(),
        display_name: "MyApp".to_string(),
        icon,
    }
}

fn notify(text: &str) -> Result<(), String> {
    let app = toast_app();
    // show 内部会先确保 AUMID 注册（幂等），应用无需额外初始化。
    show(
        &app,
        &ToastMessage {
            title: &app.display_name,
            text,
            secondary: None,
        },
    )
}

fn disable_notifications() -> Result<(), String> {
    remove_registration(&toast_app())
}
```

接入清单：

1. 为产品选定终身不变的 AUMID；dev 构建准备独立 AUMID 与显示名。
2. 品牌图标以 `include_bytes!` 提供，缓存目录由应用决定。
3. 用户关闭通知通道时调用 `remove_registration`。
4. 不要依赖任何通知库的默认 AUMID（见下）。

## 排障

- **通知没弹出但接口成功**：检查 Windows「通知设置」中该应用是否被关、专注助手
  （勿扰）是否抑制了 toast。系统抑制不算发送失败。
- **通知来源显示成 PowerShell**：说明发送时用了 PowerShell 的 AUMID。这是部分跨平台
  通知库（如未设置 `app_id` 的 notify-rust）在 Windows 上的回退行为；显式注册并传入
  自己的 AUMID 即可避免。
- **分组或静音突然失效**：AUMID 变了。检查是否改动了 AUMID 常量或混用了 dev/release
  身份。
- **改了图标没生效**：确认物化逻辑按字节比较而不是按长度判断。

## 测试要求

- 单元测试只覆盖纯逻辑（AUMID 校验、注册键路径拼接、图标物化的创建/更新/失败路径、
  非 Windows stub 返回值），不弹真实 toast；对注册表的唯一真实调用是对一个不存在的
  唯一测试键执行删除，不会留下任何注册表或文件残留。
- 真实 toast 的回归依赖手工矩阵：release/dev 身份分组、图标、多条事件折叠（若应用
  有此策略）、关闭通道后的清理、再开启后恢复、长文本与非 ASCII 文案。
