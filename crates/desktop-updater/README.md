# desktop-updater

[English README](./README.en.md)

`desktop-updater` 为 Windows-first 桌面应用提供经签名的 portable ZIP 更新流程：检查 GitHub Raw 清单、验证 Ed25519 签名和 SHA-256、下载、由独立 helper 替换发行文件，并在新进程成功启动前保留回滚副本。

它不绑定 Tauri、egui 或任何 UI 框架。应用只负责展示状态、在启动时调用 `acknowledge_if_requested`，并提供一个极薄的 updater helper 二进制。

`check_signed` 会返回经验证的清单与签名字节，配合 `UpdateCandidate::from_persisted_bytes` 可在应用重启后离线重建下载候选（重验签名，无需联网）；下载超时按声明的包大小动态放宽，慢速但仍在推进的传输不会被固定预算中断。

第一版只支持 Windows x64 portable ZIP；安装器、macOS 与 Linux 由后续 apply backend 处理。
