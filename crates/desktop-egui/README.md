# desktop-egui

[English README](./README.en.md)

`desktop-egui` 是一个面向 `egui/eframe` 桌面应用的小型辅助 crate。

它目前聚焦一个非常具体但很常见的问题：  
**当应用内置中文文案时，默认字体往往没有覆盖 CJK glyph，导致界面显示异常。**

## 版本

- 当前版本：**0.1.0**

## API 概览

### `install_cjk_fallback_fonts`

```rust
pub fn install_cjk_fallback_fonts(ctx: &egui::Context) -> Result<Option<String>, String>
```

给 `egui` 注入系统 CJK 字体 fallback。

- 找到系统里的常见中文字体时，返回 `Ok(Some(font_name))`
- 没找到可用字体时，返回 `Ok(None)`
- 字体文件读取失败时，返回 `Err(...)`

## 当前字体探测策略

按平台尝试系统常见字体：

- Windows：`Microsoft YaHei` / `SimHei` / `SimSun` / `DengXian`
- macOS：`PingFang SC` / `STHeiti` / `Hiragino Sans GB`
- Linux：`Noto Sans CJK` / `WenQuanYi Zen Hei` / `AR PL UKai`

## 设计边界

这个 crate 只处理：

- `egui` 字体 fallback 接线
- 常见系统 CJK 字体探测

它 **不会**处理：

- 应用自己的 i18n 文案
- 字体下载与安装
- 通用 UI 主题系统
