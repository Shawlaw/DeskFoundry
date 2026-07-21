# desktop-update-publisher

[English README](./README.en.md)

这是 `desktop-updater` 的发布侧 CLI。它从一个已经构建好的 portable ZIP 计算 SHA-256 与精确大小，生成 `updates/stable.json`，并使用 Ed25519 私钥生成 detached Base64 签名文件 `updates/stable.json.sig`。

```powershell
# 仅在受控终端生成一次；私钥只能进入 GitHub Environment secret。
cargo run -p desktop-update-publisher -- keygen
```

完整参数、GitHub Action 和密钥管理要求见 [portable 更新规范](../../docs/portable-update-guide.md)。
