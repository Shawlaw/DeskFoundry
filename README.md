# DeskFoundry

[English README](./README.en.md)

DeskFoundry 是一个 **Windows-first** 的 Rust 桌面应用 monorepo。

它的目标是把 **LogcatX**、**clipImg** 这类真实桌面工具里反复出现的公共能力沉淀下来，统一维护成可复用资产：

- 可复用 SDK crate
- 发布 / 构建流程约定
- 打包脚本
- 新项目模板骨架
- 设计复盘与复用文档

## 仓库内容

### 可复用 crate

- [`desktop-logger`](./crates/desktop-logger/README.md) — 文件/控制台双写日志、日志轮转、panic hook、时间戳
- [`desktop-config`](./crates/desktop-config/README.md) — 便携模式 / AppData 配置路径策略、JSON 配置读写、目录可写性辅助
- [`desktop-egui`](./crates/desktop-egui/README.md) — `egui` CJK 字体 fallback、系统字体探测
- [`desktop-i18n`](./crates/desktop-i18n/README.md) — 轻量级语言资源、回退翻译、系统语言检测
- [`desktop-fs`](./crates/desktop-fs/README.md) — 面向用户展示的路径规范化、打开文件/目录、字节大小格式化

### 模板

- [`desktop-app-template`](./templates/desktop-app-template/README.md) — 面向新 Windows 桌面工具的起步模板

### 文档

- [`docs/desktop-app-reuse-guide.md`](./docs/desktop-app-reuse-guide.md) — LogcatX / clipImg 的共性总结与可复用拆分依据

## 仓库策略

DeskFoundry 采用 **monorepo** 形式：

- 每个 crate 独立目录、独立 README
- 每个 crate 都按“可独立发布”标准组织
- 业务项目的复用路径建议是：
  1. 抽离阶段先用本地 `path` 依赖
  2. 跨仓联调阶段切到 GitHub `git` 依赖
  3. API 稳定后再考虑发布到 crates.io

## 当前版本

当前初始 SDK 版本：

- `desktop-logger` — `0.1.0`
- `desktop-config` — `0.1.0`
- `desktop-egui` — `0.1.0`
- `desktop-i18n` — `0.1.0`
- `desktop-fs` — `0.1.1`

## 本地开发

```bash
cargo test
```

## 许可证

[MIT](./LICENSE)
