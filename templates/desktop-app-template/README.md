# desktop-app-template

[English README](./README.en.md)

`desktop-app-template` 是一个面向 Windows-first Rust 桌面应用的起步模板。

它不是拿来直接发布成成品的，而是作为新项目的脚手架：复制出去后，再把产品相关字段改成你自己的业务名称。

## 模板已内置的内容

- `#![windows_subsystem = "windows"]` GUI 启动行为
- logger + panic hook 接线
- 便携模式 / AppData 配置路径策略
- 通过 `build.rs` 注入 Windows 资源元数据
- 图标目录结构
- 打包脚本
- GitHub Actions tag release 工作流
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

## 已接入的 SDK

模板默认通过本地 path 依赖接入 DeskFoundry crate：

- `desktop-logger`
- `desktop-config`
- `desktop-fs`
- `desktop-i18n`

## 构建

```bash
cargo xwin build --target x86_64-pc-windows-msvc --release
```

## 打包

```bash
./scripts/package_windows_release.sh
```
