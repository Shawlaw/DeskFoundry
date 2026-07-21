# desktop-app-template

[English README](./README.en.md)

`desktop-app-template` 是一个面向 Windows-first Rust 桌面应用的起步模板。

它不是拿来直接发布成成品的，而是作为新项目的脚手架：复制出去后，再把产品相关字段改成你自己的业务名称。

## 模板已内置的内容

- `#![windows_subsystem = "windows"]` GUI 启动行为
- logger + panic hook 接线
- 便携模式 / AppData 配置路径策略
- `egui` 中文字体 fallback 接线
- 通过 `build.rs` 注入 Windows 资源元数据
- 图标目录结构
- 打包脚本
- GitHub Actions tag release 工作流
- `desktop-updater` helper 二进制、启动确认与平铺 portable ZIP 约定
- README / CHANGELOG / LICENSE / 配置示例布局

## 重命名检查清单

从这个模板创建新应用时，至少需要替换：

- Cargo package 名称
- 产品名称 / 可执行文件名称
- `build.rs` 里的公司 / 应用标识
- 图标文件
- README 文案
- release zip 命名
- GitHub workflow 里的发布产物命名
- `desktop-update.toml` 中的产品 ID、Raw URL、公钥和 allow-list

## 已接入的 SDK

模板默认通过本地 path 依赖接入 DeskFoundry crate：

- `desktop-logger`
- `desktop-config`
- `desktop-egui`
- `desktop-fs`
- `desktop-i18n`
- `desktop-updater`

## 构建

```bash
cargo xwin build --target x86_64-pc-windows-msvc --release
```

## 打包

```bash
./scripts/package_windows_release.sh
```
