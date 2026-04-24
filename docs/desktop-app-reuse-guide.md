# LogcatX / clipImg 桌面应用复用总结

本文基于 **LogcatX** 与 **clipImg** 两个项目的实现经验，整理：

1. 两个项目的共通模式
2. 后续新开桌面应用项目时可直接复用的方法论
3. 适合抽离成 SDK / 公共 crate / 模板仓库的能力边界

---

## 1. 两个项目的共通处

虽然两个项目的业务完全不同：

- **LogcatX**：多设备 `adb logcat` 采集桌面工具
- **clipImg**：面向 WSL2 / Docker 的剪贴板图片工具

但从“一个可发布的 Rust 桌面应用”角度，它们已经形成了一套高度相似的工程骨架。

### 1.1 都是“Windows-first 的轻量桌面工具”

共通点：

- 默认面向 **Windows** 发布
- 都追求 **单 exe / 绿色版 zip**
- 都尽量避免安装器依赖
- 都需要处理“普通用户双击即用”的体验问题

这意味着它们共享的重点不是业务逻辑，而是：

- 启动体验
- 配置落盘
- 日志与排障
- 图标与版本信息
- 打包与发布链路

### 1.2 都有“发布级基础设施”，而不只是业务代码

两个项目都已经包含以下发布级能力：

| 能力 | LogcatX | clipImg | 结论 |
|---|---|---|---|
| 无黑框启动 | 有 | 有 | 必须成为桌面模板默认项 |
| 文件日志 + panic hook | 有 | 有 | 必须内建 |
| Windows 图标 / 版本元数据 | 有 | 有 | 必须标准化 |
| 版本号与 CHANGELOG | 有 | 有 | 必须随版本维护 |
| README / 构建说明 | 有 | 有 | 必须与代码一起演进 |
| `cargo xwin` 交叉构建 | 有 | 有 | 应固化为统一发布路径 |
| GitHub Actions tag release | 有 | 有 | 应成为默认 CI 能力 |

结论：  
这类项目真正重复的不是“主功能”，而是“从原型到可发布”的整套外围能力。

### 1.3 都把配置文件放在可理解的位置

两个项目都采用了“**配置文件应可定位、可编辑、可迁移**”的思路：

- `config.json` 是核心入口
- 首次运行会引导用户确认关键路径
- 配置错误时有明确反馈，而不是静默失败
- 配置结构会随着版本迭代被补字段、迁移旧格式

区别在于：

- **clipImg** 更偏“exe 同目录配置”
- **LogcatX** 进一步做了“**exe 同目录优先，不可写则回退 AppData**”

经验总结：

> 对小型桌面工具来说，配置策略本身就是产品体验的一部分，不应临时拼凑。

### 1.4 都把“运行日志”当成一等公民

两个项目都已经证明：

- 用户不会主动开控制台
- GUI 程序一旦出问题，如果没有日志就很难排查
- panic 不落盘时，现场信息会直接丢失

因此共有做法是：

- 文件日志默认开启
- 可选控制台输出
- panic hook 单独写入
- 启动时记录关键环境信息

经验总结：

> 对桌面工具而言，“日志系统”不是调试阶段临时能力，而是发布后排障基础设施。

### 1.5 都把“首次运行引导”视为正式功能

两个项目都存在首次使用必须确认的关键配置：

- clipImg：保存目录、输出路径
- LogcatX：adb 路径、日志目录、语言

这类逻辑的共通特点：

- 不能把首次引导塞成一个临时弹框
- 需要明确告诉用户“这个值是干什么的”
- 需要在保存前做校验
- 需要把失败原因直说

经验总结：

> 首启流程本质上是一个产品 onboarding，不是一个“先凑合弹个框”的技术细节。

### 1.6 都需要“面向发布”的资源管理

两边都已经沉淀出类似的资源组织：

- `assets/`：设计源文件 / 说明
- `icons/`：构建时使用的 ico / png
- `build.rs`：写入 Windows 资源信息

经验总结：

> 图标、版本描述、产品名、OriginalFilename 这些看似边角料，实际上是用户判断“这个软件是否成熟”的第一层信号。

### 1.7 都已经形成“代码 + 文档 + 发布脚本 + CI”的闭环

两个项目都不是“只有源码”，而是有完整交付链条：

1. 代码实现
2. README / CHANGELOG / LICENSE
3. 本地打包脚本
4. GitHub Actions 自动发布

经验总结：

> 新项目最值得复用的，不是单个模块，而是一套完整的仓库骨架。

---

## 2. 可复用的项目经验

下面这些经验适合在后续新桌面应用项目中直接复用。

## 2.1 先搭“发布骨架”，再填业务功能

推荐顺序：

1. 建立项目目录结构
2. 接入日志 / panic / 配置
3. 接入无黑框启动 / 图标 / 版本元数据
4. 接入打包脚本与 CI
5. 再迭代业务功能

原因：

- 否则业务代码会越来越多，基础设施永远补不齐
- 原型阶段不做，后面补起来成本更高

## 2.2 统一目录骨架

后续新项目建议默认采用类似结构：

```text
project/
├── src/
├── assets/
├── icons/
├── scripts/
├── .github/workflows/
├── README.md
├── README.en.md
├── CHANGELOG.md
├── LICENSE
├── config.example.json
├── build.rs
└── .cargo/config.toml
```

这套结构已经被 LogcatX 和 clipImg 共同验证过，适合作为新的桌面应用模板。

## 2.3 发布链路必须标准化

应固定为：

- 本地构建：`cargo xwin build --target x86_64-pc-windows-msvc --release`
- 本地打包：统一 `scripts/package_windows_release.sh`
- 远端发布：push `v*` tag 自动构建并生成 GitHub Release

好处：

- 构建路径不再依赖个人记忆
- 新项目只需替换产品名和产物名
- 本地 / CI 产物保持一致

## 2.4 日志和错误处理要“用户可理解”

推荐默认策略：

- 启动失败：弹错误框
- 运行错误：状态区可见
- 详细信息：写日志文件
- panic：单独落日志并尽可能附带 backtrace

不要做的事：

- 静默返回
- 宽泛吞错
- 只把错误打印到控制台

## 2.5 配置策略要可迁移

LogcatX 和 clipImg 都说明了一个事实：

> 小工具也会长期演进，因此配置结构迁移是刚需，不是“以后再说”。

建议所有新项目默认具备：

- 默认值补全
- 废弃字段迁移
- 新字段自动补齐
- 配置合法性验证
- 保存前归一化

## 2.6 文档不是收尾工作，而是发布接口的一部分

建议默认维护：

- `README.md`：主文档
- `README.en.md`：英文别名文档
- `CHANGELOG.md`：版本记录
- `LICENSE`

这样后续做开源、Release、Issue 排障时都会轻松很多。

## 2.7 业务逻辑和发布基础设施要主动分层

这两个项目最值得复用的经验之一，就是把“发布基础设施”从“业务能力”里分离出来看。

后续做新桌面应用时，应主动分成三层：

1. **产品业务层**  
   例如 ADB、剪贴板、托盘、热键、日志查看、文件处理

2. **桌面应用基础设施层**  
   例如配置、日志、图标、版本、首次引导、路径处理、发布脚本

3. **发布与仓库治理层**  
   例如 CI、tag release、CHANGELOG、LICENSE、README

---

## 3. 哪些能力适合抽成 SDK / 公共 crate

以下不是都应该立刻拆，但它们已经具备“跨项目复用价值”。

## 3.1 最值得先拆的部分

### A. `desktop-logger`

适合抽离内容：

- 文件 + 控制台双写 logger
- 日志轮转
- panic hook
- 时间戳格式化
- 启动日志公共字段

适合原因：

- 两个项目都在用
- 业务无关
- API 边界清晰

建议接口方向：

- `init(log_path, console_mode, max_size_mb)`
- `set_panic_hook(log_path)`
- `filename_timestamp()`
- `now_timestamp()`

### B. `desktop-config`

适合抽离内容：

- `config.json` 读写
- 默认值补齐
- 迁移辅助
- portable / AppData 路径策略
- 配置路径解析
- 可写目录探测

适合原因：

- 新桌面工具几乎都要做
- 每个项目都容易重复写一遍

建议接口方向：

- `resolve_app_paths(app_id, app_name)`
- `load_or_default(path, defaults)`
- `save_pretty_json(path, value)`
- `is_dir_writable(path)`

### C. `desktop-release`

适合抽离内容：

- `build.rs` 中 Windows 资源模板生成
- 产品名 / EXE 名 / 版本号映射
- 统一的打包脚本模板
- GitHub Actions release workflow 模板

更适合的形式：

- 不一定是 crate
- 更适合做成 **模板仓库 / 脚手架 / 复用脚本集合**

原因：

- `build.rs`
- workflow YAML
- shell script

这些不适合作为普通运行时 SDK，但非常适合沉淀成模板。

### D. `desktop-i18n`

适合抽离内容：

- 内嵌 locale 文件加载
- 语言代码归一化
- 系统语言检测
- fallback 逻辑
- 占位符替换

适合原因：

- LogcatX 已经用上
- clipImg 后续也很可能需要
- 小型桌面工具不一定需要引入重量级 i18n 框架

建议接口方向：

- `I18n::new(lang)`
- `normalize_language_code()`
- `detect_system_language()`
- `tr() / tr_args()`

### E. `desktop-fs`

适合抽离内容：

- Windows 路径展示归一化
- `\\?\` verbatim path 处理
- 打开文件 / 打开目录
- 目录大小统计
- 历史文件清理
- 安全文件名生成
- 时间戳文件名工具

适合原因：

- 这类代码经常零散重复出现
- 对桌面工具很常见

## 3.2 可以考虑拆，但要谨慎

### F. `desktop-bootstrap`

可包含：

- `windows_subsystem = "windows"` 相关模式约定
- 启动流程骨架
- fatal error dialog
- 单实例保护
- 启动信息打印

问题：

- 不同 UI 框架差异大（egui、tray、原生 Win32、tauri）
- 抽太早容易变成不好用的大而全框架

建议：

- 先做“参考实现 / 模板模块”
- 暂时不要急着发布成独立 crate

### G. `desktop-first-run`

可包含：

- 首次运行状态机
- 必填项校验
- 保存 / 取消逻辑
- 用户提示文案组织

问题：

- UI 强相关
- 不同产品的首启流程差异很大

建议：

- 先沉淀设计模式，不要急着抽象成强绑定 SDK

### H. `desktop-windows`

可包含：

- 隐藏子进程黑框
- 单实例 mutex
- Windows MessageBox 错误弹窗
- Explorer 打开路径
- 字体探测 / CJK fallback

问题：

- 平台绑定强
- 但复用价值也很高

建议：

- 可以做成一个偏 Windows-only 的辅助 crate

---

## 4. 哪些东西不建议过早抽 SDK

以下更适合继续留在业务项目中：

### 4.1 UI 结构本身

原因：

- LogcatX 用 `egui/eframe`
- clipImg 主要是 tray + 原生 Win32 消息循环

两边差异已经很大，强行抽 UI 基座，往往会让接口非常别扭。

### 4.2 业务核心能力

例如：

- ADB 设备探测 / `logcat` 采集
- 剪贴板监听 / WSL2 路径转换
- 全局热键 / 托盘逻辑

这些属于产品壁垒，不应为了“看起来通用”而提前抽象。

### 4.3 文案层面的产品逻辑

例如：

- 状态提示语
- 首启说明文案
- 错误提示风格

可以共用组织方式，但不要急着共用具体内容。

---

## 5. 更推荐的复用形态

综合来看，后续最推荐的复用方式不是只做一个 SDK，而是同时准备三种复用层：

## 5.1 模板仓库（优先级最高）

最适合复用的内容：

- 目录结构
- `build.rs`
- `scripts/package_windows_release.sh`
- `.github/workflows/release.yml`
- README / CHANGELOG / LICENSE 骨架
- icons / assets 结构
- `.cargo/config.toml`

这是最有价值、最容易落地的复用层。

## 5.2 小而稳的基础 crate

优先建议拆分：

1. `desktop-logger`
2. `desktop-config`
3. `desktop-i18n`
4. `desktop-fs`
5. `desktop-windows`（如后续 Windows-only 项目继续增多）

原则：

- 每个 crate 只做一个稳定问题域
- 不做大而全框架
- 不绑定具体业务

## 5.3 示例项目 / 参考实现

例如准备两个示例：

1. **GUI 示例**：类似 LogcatX
2. **Tray / 后台工具示例**：类似 clipImg

这样比单纯文档更有复用价值。

---

## 6. 建议的后续抽离顺序

推荐顺序如下：

### 第一阶段：先做模板仓库

目标：

- 新项目可以一键复制骨架
- 默认带版本、日志、图标、CI、打包脚本

这是收益最高的一步。

### 第二阶段：拆 `desktop-logger`

目标：

- 先去掉两个项目最明显的重复实现

这是最容易成功的 SDK 抽离。

### 第三阶段：拆 `desktop-config`

目标：

- 统一配置落盘、默认值、迁移、portable/AppData 策略

### 第四阶段：拆 `desktop-i18n` 和 `desktop-fs`

目标：

- 让后续新工具直接继承基础桌面能力

### 第五阶段：视项目数量决定是否拆 `desktop-windows`

如果后面继续做多个 Windows-first 小工具，这一步会非常值得。

---

## 7. 一句话结论

LogcatX 和 clipImg 的最大共通点，不是它们都用了 Rust，也不是它们都做了桌面 UI，而是：

> 它们都已经证明，一个“可对外发布的小型桌面工具”真正需要复用的核心，不是业务逻辑，而是 **发布骨架、日志体系、配置体系、资源体系和自动发布链路**。

因此后续如果再开新的桌面应用项目，最值得先复用的是：

1. **模板仓库**
2. **desktop-logger**
3. **desktop-config**
4. **desktop-release workflow / packaging**
5. **desktop-i18n / desktop-fs**

而不是一开始就试图抽一个“大一统桌面应用框架”。
