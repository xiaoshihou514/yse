# Yse

Rust 优先的框架，用于构建严肃的跨平台 Qt 6 Widgets 应用，且不需要 QML。
以 CXX-Qt 作为互操作后端，反应式编程模型借鉴 Airstream 与 Laminar。

Yse 面向数据密集的原生桌面应用：既能享受 Qt 成熟的控件与 Linux 集成，
又无需编写 QML，也不必手动管理信号连接的声明周期。

> **预发布声明：** API 仍在演进，crates 尚未发布。在干净的机器与打包
> 链路加固完成前，请使用本地检出。

## 五分钟本地起步

你只需要 Rust 与 C++ 工具链。Qt 6 由 `gansi` 自行下载并管理（绝不经过系统
包管理器）。

按平台安装 C++ 工具链：

```sh
# Fedora
sudo dnf install gcc-c++ cmake ninja-build pkgconf-pkg-config

# Ubuntu / Debian
sudo apt install g++ cmake ninja-build pkg-config

# openSUSE Tumbleweed
sudo zypper install gcc-c++ cmake ninja lld pkgconf

# Windows（Visual Studio Build Tools 2022，勾选“使用 C++ 的桌面开发”工作负载）
#   + CMake 与 Ninja：scoop install cmake ninja
```

然后让 `gansi` 获取 Qt：

基于当前检出创建并运行一个应用：

```sh
cargo install --path crates/gansi
gansi doctor
cd /tmp
gansi create --local /path/to/yse hello-yse
cd hello-yse
gansi run
```

`gansi setup`（上述命令在需要时自动运行）会将预编译的 Qt 6 二进制下载到
gansi 数据目录——不需要 `qt6-*-devel` 软件包，也不需要手动安装 Qt。

生成的应用持有反应式 Rust 状态、派生其标签文本，并在 Qt 对象树销毁时
释放控件绑定。其极简 `build.rs` 仅保留 CXX-Qt 依赖初始化器；不生成
应用自有的 C++ 或 QML。

## 支持的环境

| 环境 | 状态 |
|---|---|
| Fedora、Qt 6、GCC、Wayland/offscreen | 主开发环境 |
| Ubuntu、Qt 6、GCC | 支持；干净机器验证进行中 |
| KDE Plasma 与 GNOME | 目标 Tier 1 桌面；完整 UX 矩阵进行中 |
| Windows | 已有构建辅助脚本与任务管理器后端 |
| macOS | 实验性；不阻塞首次发布 |

无需 sudo 即可运行完整的无显示 Fedora 验收路径：

```sh
CCACHE_DISABLE=1 just release-check
```

该命令不会发布或上传任何内容。它依次运行工作区、MSRV、依赖许可证、
独立 crate 打包、生成项目、示例与 Valgrind 门禁。Linux 冒烟门禁会构建并
执行默认发布包；调试打包时可改用
`YSE_SMOKE_BUNDLE_PROFILE=debug just linux-smoke`。在有可用 Wayland
会话时，`just examples-wayland-smoke` 会在原生显示插件上运行同样的八条
应用流程。

精确的已验证环境与仍未验证的平台场景见
[docs/validation.md](docs/validation.md)。

Phase 0–4 计划到实现与证据的一一映射见
[docs/roadmap-status.md](docs/roadmap-status.md)。

完整路线图见 [agent_docs/PLAN.md](agent_docs/PLAN.md)。

架构说明见 [docs/architecture.md](docs/architecture.md)，发布流程见
[RELEASE.md](RELEASE.md)。Qt 与 CXX-Qt 各自的再分发义务汇总于
[docs/licensing.md](docs/licensing.md)。

## 当前状态

Phase 0（CXX-Qt 可行性验证）到 Phase 4（`gansi`）所描述的实现均已就位。
Fedora 本地的功能与打包门禁通过，但路线图中干净主机与多平台的退出标准
尚未全部验证，见 [docs/validation.md](docs/validation.md)。
[`yse`](crates/yse) facade 再导出两层公共 API，应用可直接
`use yse::*` 使用整个栈。

[`yse-model`](crates/yse-model) 是纯 Rust 反应式运行时（无 Qt、无
`unsafe`）：事务化、无毛刺的传播，强制订阅所有权，基础操作符集合，
以及可取消的异步任务 API（`spawn_task`），其结果通过
[`Scheduler`](https://docs.rs/yse-model) 投递回图所在线程（确定性测试见
`QueueScheduler`）。它还提供 `ListModel<T>`——带当前快照与结构化变更
流（`Insert`/`Remove`/`Update`/`Reset`）的增量列表，含原地 `sort_by`/
`retain` 并发出增量变更——以及带 `Command` trait 的 `UndoStack`，其
`can_undo`/`can_redo` 状态是反应式的。另有 `Diagnostics`（事务化图事件的
流：节点处理、观察者触发、延迟写入、循环拒绝——用于解释控件为何变化）
和带可观察记录流的 `Logger`。运行测试：

```sh
cargo test -p yse-model
```

[`yse-ui`](crates/yse-ui) 通过小型 C++ shim 将 `yse-model` 绑定到 Qt
Widgets 保留树：窗口、行列布局、标签、按钮、单行输入、复选框、组合框、
旋钮、滑块、进度条、日期/时间编辑与网格布局（含通用 `add` 嵌套），另有
带快捷键的动作、菜单、菜单栏与工具栏。具备信号到属性绑定、双向单行输入
绑定，以及随 Qt 对象销毁自动释放的组件 Owner。`QtGuiScheduler` 将
`yse_model::spawn_task` 的结果投递到 Qt 事件循环；`yse-model` 的时间
操作符（`debounce`、`throttle`、`delay`）在应用中使用 `QtTimer` 驱动，
测试中使用 `ManualTimer`。
`StringListModel` 通过 C++ `QAbstractListModel` 适配器驱动 `QListView`，
用 `beginInsertRows`/`beginRemoveRows` 增量应用变更，普通编辑不会重置
视图；`ListView::selection_changed()` 将 `QItemSelectionModel` 的变更作为
行索引流暴露。设置演示将编辑菜单的撤销/重做动作经 `bind_enabled` 接到
`UndoStack`。`StringTableModel` 为多列 `QTableView` 做同样的事
（`QAbstractTableModel`，带头部与增量行插入/删除/更新）。标准对话框以
反应式包装形式提供：`MessageBox`（`Ok`/`OkCancel`/`YesNo`，结果流）与
`FileDialog`（打开文件，可选路径结果），均非模态且可在无头环境测试。
`Settings` 提供基于 QSettings 的字符串键值持久化。

大表性能有粗略基准：

```sh
cargo run -p yse-model --release --example bench_large_table
```

在本机，10 万行加载约 11 ms、1 万行逐行增量更新约 6 ms、原地排序约
9 ms——全程不重建控件树。

Phase 3 演示应用 [`data_browser`](crates/yse-ui/examples/data_browser.rs)
串起了整个栈：异步加载记录（支持取消与错误态）、过滤并排序增量表、通过
表单编辑行（带撤销/重做）、持久化设置，并暴露菜单快捷键。无头运行：

```sh
QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui --example data_browser
```

驱动完整流程后会打印 `status`、可见行数、活动过滤器与首行可见行。

## 开发者工具链（`gansi`）

[`gansi`](crates/gansi) 是 Phase 4 开发者 CLI。安装：

```sh
cargo install --path crates/gansi
```

然后：

```sh
gansi create hello        # 在当前检出内，自动探测本地 Yse facade
gansi create --local /path/to/yse hello-facade  # 其他位置，显式选择检出
cd hello
gansi run                 # 构建并运行
gansi test                # 运行测试
gansi analyze             # Clippy，全部目标，警告视为错误
gansi format --check      # 校验 rustfmt 输出
gansi upgrade -- --offline # 离线刷新 Cargo.lock
gansi add serde -- --features derive # 添加 Cargo 依赖
gansi build               # 发布构建 + 平台包（具备 windeployqt/macdeployqt 时使用）
```

Linux 包包含启动器、应用二进制、所需 Qt 库、精选桌面插件、Qt 运行时清单
与可发现的 Qt 许可文本。生成的 freedesktop 与 AppStream 元数据以及可缩放
图标放在 `share/` 下用于打包集成。`GLIBC_REQUIREMENTS.tsv` 记录每个被打包
ELF 对象所需的最新 glibc 符号。包有意将 glibc、显卡驱动与非 Qt 系统库
留给目标系统，并在 `SYSTEM_RUNTIME.tsv` 中列出其构建主机解析路径。
`.tar.gz` 制品保留完整可重定位目录以便传输与测试，附
`dist/SHA256SUMS` 中的 SHA-256 摘要。

生成项目在创建时将检测到的 Qt 版本、编译器族与目标架构写入
`gansi.toml`；`run`、`test` 与 `build` 在调用 Cargo 前拒绝不匹配。
项目还附 `RELEASE.md` 指南，列出仍属于应用侧的事项（代码签名、公证、
商店提交、原生依赖）。`gansi setup` 下载的 Qt SDK 归档在解压前按
各归档校验和侧车文件逐一验证；优先 SHA-256，Qt 仓库未发布 SHA-256
侧车时回退 SHA-1。`--local` 变体依赖你检出的 `yse` facade crate，
生成的应用无需 C++ shim 即可使用完整框架。crates 发布前，`gansi create`
在无法发现检出时会拒绝注册表解析。失败的依赖搭建会分阶段清理，不遗留
部分项目目录。

运行其生命周期测试与无头设置表单演示：

```sh
cargo test -p yse-ui
QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui --example settings
```

设置演示在冒烟模式下端到端驱动表单：经双向绑定输入名称，点击提交（由派生
状态启用），从后台任务加载状态并经 GUI 线程投递，再触发文件菜单动作，
随后打印问候语、状态与加载状态并退出。

[`controls`](crates/yse-ui/examples/controls.rs) 演示以声明式树展示数值
控件：组合框、旋钮、滑块与进度条，用信号绑定与值变更处理器联动。无头
运行 `QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui
--example controls`。FFI 安全不变量——指针生命周期、销毁钩子、C++ 边界
的 panic 遏制与控件扩展模式——记录于 [docs/safety.md](docs/safety.md)。

[`yse-taskmgr`](examples/taskmgr) 是跨平台系统任务管理器（Windows /
Linux），完全采用 Laminar 风格：所有状态放在 `Var` 中，UI 绑定到派生
信号（`bind_rows`、`bind_series`、`bind_visible`、……），事件处理器只修改
状态。系统数据由 `cfg` 选择的平台后端采集：Linux 读 `/proc` 与 `/sys`，
Windows 用 `windows-sys`。它跟随系统配色（亮/暗）、显示真实进程图标
（Windows 用 shell 图标，Linux 用 PATH 查找），并采用现代 Fusion +
样式表外观。无头运行 `QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run
-p yse-taskmgr`（Windows 宿主用 `just windows-run yse-taskmgr`）。

## 开发环境

- Rust 1.89 或更新（edition 2024）
- C++17 编译器与 CMake/Ninja（命令见上文）
- Qt 6 由 `gansi setup` 下载并管理；请勿自行安装
  `qt6-base-dev` / `qt6-qtbase-devel`

## 构建与运行

```sh
just --list                  # 列出开发快捷键
just check                   # 格式检查、lint、测试与文档
cargo build
```

安装 [`just`](https://just.systems/) 以使用快捷键；底层 Cargo 命令仍可
直接使用。

## CI

为节省 GitHub Actions 配额，有意不提交工作流。工作区以本地验证代替：

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```
