# Portable 自动更新规范

[English version](./portable-update-guide.en.md)

本文定义 DeskFoundry 第一版通用更新能力。它只支持 **Windows x64、portable ZIP、GitHub Raw 清单与 GitHub Release 资产**；MSI、NSIS、macOS 与 Linux 不在此协议范围内。

## 设计边界

- `desktop-updater` 不依赖 Tauri、egui 或产品 UI。
- 应用负责展示状态、决定检查频率，并在启动时确认更新成功。
- GitHub Raw 只保存小型 JSON 清单和 detached signature；二进制 ZIP 必须从 GitHub Release 下载。
- 每个产品使用独立 Ed25519 密钥对。私钥仅放在受保护的 GitHub Environment/Secret，公钥编译进客户端。
- 更新器只替换产品声明的文件，绝不清空安装目录；配置、日志、`secrets/`、缓存和用户文件不受影响。

## 产品描述

每个应用在自身仓库维护 `desktop-update.toml`，它是打包与运行时共用的布局契约：

```toml
app_id = "com.example.myapp"
channel = "stable"
manifest_url = "https://raw.githubusercontent.com/example/MyApp/main/updates/stable.json"
signature_url = "https://raw.githubusercontent.com/example/MyApp/main/updates/stable.json.sig"
public_key_base64 = "<32-byte Ed25519 public key in Base64>"
archive_root = ""
replace_files = ["MyApp.exe", "MyApp.Updater.exe"]
preserve_files = ["portable.marker"]
```

`replace_files` 和 `preserve_files` 共同构成 allow-list。前者会被替换，后者必须在 ZIP 中但绝不改写已安装副本；portable marker 通常应放在后者。ZIP 中任何未声明的文件、重复文件、绝对路径、`..` 路径或超出 `archive_root` 的文件都会拒绝更新。新项目推荐平铺 ZIP（`archive_root = ""`）；旧项目可声明单个 ZIP 根目录。

## 应用接入

将 `desktop-updater` 接入应用后：

1. 后台调用 `check`，仅在候选版本严格高于当前 SemVer 时提示用户。
2. 用户选择更新后调用 `download`，下载到当前配置目录的 `updates/`。
3. 调用 `apply_and_restart`，随后主动退出主进程。
4. 新进程在完成最小启动检查后尽早调用 `acknowledge_if_requested`。

应用必须额外打包一个极薄 helper 二进制：

```rust
fn main() {
    if let Err(error) = desktop_updater::run_helper_from_args() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
```

运行中的 Windows exe 不能直接覆盖。主程序会把 helper 复制到 `updates/` 后启动；helper 等待主程序退出，解压到 staging，备份旧文件、替换新文件并重启应用。新进程未在 60 秒内确认启动时，helper 会终止新进程、恢复备份并重启旧版本。

## 发布与签名

首次为产品生成密钥时，在受控的本机终端运行：

```powershell
cargo run -p desktop-update-publisher -- keygen
```

把 `private_key_base64` 保存为该产品 GitHub Environment 的 `DESKTOP_UPDATE_PRIVATE_KEY` secret；把 `public_key_base64` 写进 `desktop-update.toml` 和产品编译配置。不要把私钥提交仓库、写入日志或粘贴到 Issue。

产品发布工作流先构建并上传 ZIP 到 GitHub Release，再调用共享 Action：

```yaml
- uses: Shawlaw/DeskFoundry/actions/publish-portable-update@v0.1.0
  with:
    app-id: com.example.myapp
    version: ${{ steps.version.outputs.value }}
    asset-path: artifacts/MyApp_1.2.3_windows_x64_portable.zip
    asset-url: https://github.com/example/MyApp/releases/download/v1.2.3/MyApp_1.2.3_windows_x64_portable.zip
    notes-url: https://github.com/example/MyApp/releases/tag/v1.2.3
    private-key: ${{ secrets.DESKTOP_UPDATE_PRIVATE_KEY }}
```

调用方 workflow 需要 `permissions: contents: write`，并且应在 release asset 上传成功后才调用 Action。Action 会先按 `desktop-update.toml` 校验 ZIP 的根目录、文件 allow-list 与解压大小，再计算 SHA-256 和精确大小，生成 `updates/stable.json` 与 `updates/stable.json.sig`，签名后提交到产品仓库默认分支。

清单格式如下，签名覆盖该 JSON 文件的**原始 UTF-8 字节**：

```json
{
  "schemaVersion": 1,
  "appId": "com.example.myapp",
  "channel": "stable",
  "version": "1.2.3",
  "publishedAt": "2026-07-21T00:00:00.0000000Z",
  "target": "windows-x64",
  "asset": {
    "url": "https://github.com/example/MyApp/releases/download/v1.2.3/MyApp_1.2.3_windows_x64_portable.zip",
    "sha256": "...",
    "size": 1234567
  },
  "notesUrl": "https://github.com/example/MyApp/releases/tag/v1.2.3"
}
```

客户端使用 HTTPS、签名、SHA-256、大小上限和 ZIP allow-list 多层验证。Raw 的 CDN 缓存通过 `Cache-Control: no-cache` 与小时级 query 参数规避；短暂的 JSON/signature 可见性不同步会被安全地视为本次检查失败并在下次重试。

## 测试要求

- 测试签名失败、应用 ID/通道/目标不匹配、降级版本和错误 SHA-256。
- 测试 ZIP traversal、重复条目、未声明文件、缺少声明文件和解压大小限制。
- 测试文件锁、替换失败、启动未确认时的回滚。
- 在发布前用真实 ZIP 执行 publisher，并验证清单、签名与公钥可被目标应用验证。
