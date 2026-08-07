# gansi（干丝）开发计划

`yse-tool` 的下一步演进。目标三步走：

1. crate 更名为 `gansi`（干丝），从 cargo 子命令变为独立 CLI（对齐 `flutter`）。
2. 去除对 `aqt`（Python 工具）的依赖，用 Rust 实现 Qt SDK 的检测 / 下载 / 安装。
3. 参照 `flutter` 命令面补齐项目管理的缺失功能。

---

## 1. 现状盘点（先摸清再动手）

代码地图：

| 文件                              | 内容                                                                                                                        |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `crates/yse-tool/src/main.rs`     | 手写 `env::args` 命令分发：`new/dev/test/bundle/version/help`，含 `check_qt()` 预检                                         |
| `crates/yse-tool/src/project.rs`  | `Project` 模型、名称校验、`write_project` / `write_project_local`、`bundle()`（macOS plist、`windeployqt` / `macdeployqt`） |
| `crates/yse-tool/src/template.rs` | 模板渲染（`{{name}}` / `{{name_snake}}` / `{{title}}` / `{{yse_path}}`），本地与 facade 两套模板                            |
| `scripts/windows.ps1`             | Windows 宿主辅助脚本；aqt 的唯一真实调用点（`setup` 分支第 85 行），另有 MSVC 环境导入、`CARGO_TARGET_DIR` 隔离         |

要点：

- `yse-tool` 目前零 Cargo 依赖，测试是 `project.rs` 内嵌的 4 个 `#[test]`。
- aqt 不是 Cargo 依赖，只是 windows.ps1 里调的外部 Python 工具。其余出现处：`justfile:46` 注释、`main.rs:153` 报错文案。
- 生成项目当前文件清单：`Cargo.toml`、`build.rs`、`yse.toml`、`.gitignore`、`icon.svg`、`RELEASE.md`、`src/{main.rs,bridge.rs}`、`src/spike.{h,cpp}`。没有 `tests/`、没有 `README.md`、没有 IDE 配置、没有脚手架版本号。
- 模板、README、RELEASE.md、PLAN.md 里散布 `cargo yse` 文案，改名时全部要跟着改。
- aqt源码：https://github.com/miurahr/aqtinstall，已在../../scratch/aqtinstall

---

## 2. 第一步：crate 更名为 `gansi`

### 2.1 重命名方案

- `crates/yse-tool` → `crates/gansi`
- `[package] name = "gansi"`，`[[bin]] name = "gansi"`
- 安装方式随之变为 `cargo install --path crates/gansi`，得到独立命令 `gansi`（不再依赖 cargo 子命令约定，对齐 `flutter`）。
- 命令字：`gansi create/run/test/build/doctor/...`（见第四步）。

### 2.2 波及面（必须同步，否则文档与产物不一致）

- `README.md`：`cargo install --path crates/yse-tool`、全部 `cargo yse` 示例。
- `RELEASE.md`：发布顺序第 4 项 `crates/yse-tool` → `crates/gansi`。
- `agent_docs/PLAN.md`：Phase 4 命令块（第 204-211 行）。
- `AGENTS.md`：`crates/yse-tool` 一行。
- `crates/yse-tool/src/template.rs`：`CARGO_TOML_LOCAL`、`MAIN_RS_LOCAL`、`SPIKE_CPP`、`RELEASE_MD`、`BRIDGE_RS` 内的 `cargo yse ...` 文案与注释。
- `crates/yse-tool/src/main.rs`：模块文档、help 文案、错误文案（`Run \`cargo yse --help\``）。
- `Cargo.lock`：cargo 重新解析后自动更新。
- D2 项目清单更名：建议同步 `yse.toml` → `gansi.toml`

### 2.4 验收

- `cargo install --path crates/gansi && gansi --version` 正常。
- 全仓 grep 无残留 `cargo yse` / `yse-tool`（兼容 shim 除外）。
- `just check` 通过。

---

## 3. 第二步：去除 aqt，Rust 重写 Qt SDK 安装

### 3.1 目标

`gansi doctor`（检测）+ `gansi setup`（安装）在 Rust 内闭环“检测 → 下载 → 校验 → 解压 → 登记”，Windows / macOS / Linux 统一入口，`windows.ps1` 不再调用 aqt。

### 3.2 aqt 行为拆解（Rust 需复刻的能力）

1. 查询官方仓库索引 `https://download.qt.io/online/qtsdkrepository/<os>/<repo>/Updates.xml`，按版本 / 架构定位 qtbase 及 add-ons。
2. 下载 `.7z` 归档到临时目录。
3. 用 `Updates.xml` 中的哈希校验完整性（aqt 校验 SHA-1）。
4. 解压到 `<install-root>/<version>/<arch>/` 布局，使 `bin/qmake.exe` 等落在预期路径。
5. 记录安装信息（版本、路径、目标）。

### 3.3 Rust 实现要点

- 新增依赖（dev 工具，接受体积）：
  - `clap`：命令面扩展后手写 args 解析不可维护（第四步也依赖它）。
  - HTTP：`reqwest`
  - 归档：自选
  - 哈希验证：自选
- 目录布局：
  - 安装根 `GANSI_HOME/qt/<version>/<platform>/<arch>/`, GANSI_HOME应当使用跨平台库来解决路径，如linux应当用~/.local/share/gansi等等
  - 全局配置 `GANSI_HOME/config.toml`：qt_roots、默认 qt_version、target_arch、compiler_family。
  - 项目级 `gansi.toml` 钉 qt_version；全局配置提供 SDK 路径，二者职责分开。
- 命令形态：
  - `gansi setup <version>`：安装指定版本。
  - `gansi setup`：按项目 `gansi.toml` 的 qt_version 补齐 SDK。
  - `gansi doctor`：检测 Rust toolchain、C++ 编译器、CMake、Qt SDK，缺什么报什么并给出修复命令——替代现有 `check_qt()`。
- 与 CXX-Qt 打通：`run/test/build` 时设置 `QMAKE` / `CMAKE_PREFIX_PATH` / `PATH`（参考 `windows.ps1` 的 `Import-VsEnv` 做法）；`cxx-qt-build` 经 Qt CMake 定位。MSVC 环境注入仍留在宿主脚本，不在 Rust 里做 shell 魔法。
- 测试：离线单测覆盖 Updates.xml 解析、arch/version 匹配、路径布局；真实下载标记 `#[ignore]`（slow），手动/CI 执行。

### 3.4 验收

- 无 Qt 的干净 Windows 机器：`gansi setup` 后 `gansi run` 成功。
- `windows.ps1` 不再出现 aqt；`justfile:46` 注释更新；`main.rs:153` 文案改为指向 `gansi doctor`。
- `gansi doctor` 退出码区分：0=健康 / 1=可修复 / 2=缺依赖。

---

## 4. 第三步：参照 flutter 补齐项目管理命令

### 4.1 命令对照表

| flutter                               | 现状         | 规划                                                                             | 优先级         |
| ------------------------------------- | ------------ | -------------------------------------------------------------------------------- | -------------- |
| `create`                              | `yse new`    | 更名 `gansi new`                                                   | P0（随重命名） |
| `run`                                 | `yse dev`    | 更名 `run`，加 `--release`；保留参数透传（现 `dev` 已透传）                      | P0             |
| `test`                                | `yse test`   | 保留                                                                             | P0             |
| `doctor`                              | `check_qt()` | 新增（见第三步）                                                                 | P0             |
| `build`                               | `yse bundle` | 扩展为 `build [--debug]` | P1  |
| `clean`                               | —            | 新增：`cargo clean` + 清理 `dist/`                                               | P1             |
| `config`                              | —            | 新增：读写 `~/.gansi/config.toml`（get/set/list），原子写入（临时文件 + rename） | P1             |
| `init`                                | —            | 新增：在当前目录初始化（`flutter create .` 语义），不覆盖已有源文件              | P1             |
| `env`                                 | —            | 新增：打印环境与 SDK 路径（`doctor --verbose`）                                  | P2             |

### 4.2 各命令验收判定

- `create`：目标目录已存在时报错（沿用现状）；生成文件清单与现状一致或按 4.3 扩展。
- `init`：在已有源码目录补齐 `gansi.toml` / `.gitignore`，不覆盖用户文件。
- `run`：编译 + 设置 Qt 环境后启动，参数透传。
- `build`：`--release`/`--debug` 映射 cargo profile；`project.rs::bundle` 复用。
- `clean`：删除 `target/` 与 `dist/`，打印释放空间。
- `doctor`：见 3.4 退出码约定。

### 4.3 脚手架扩展（对照 `flutter create` 输出）

现有输出缺：`tests/`、`README.md`、IDE 配置、脚手架版本号。建议新增：

- `tests/` + 一个启动即通过的 smoke 测试。
- `README.md`（flutter create 会生成）。
- 模板打上 gansi 版本号。

### 4.4 既有约束

- gansi 是纯 dev 工具、无 FFI，保持 `#![forbid(unsafe_code)]`（与 yse-model 一致）。
- 依赖方向不变：gansi 不依赖 `yse` / `yse-ui` / `yse-model`（RELEASE.md 现状）。

---

## 5. 里程碑顺序

| 里程碑 | 内容                                                       | 依赖            | 可独立交付 |
| ------ | ---------------------------------------------------------- | --------------- | ---------- |
| M1     | 重命名 gansi（含全部文档 / 模板同步）                      | —               | 是         |
| M2     | CLI 重构为 clap + `doctor`（`check_qt` 下线）              | —               | 是         |
| M3     | Rust Qt 安装器（Windows 优先，aqt 真正退役）               | M2 的 config 层 | 否         |
| M4     | 项目管理命令补齐（P0/P1：run/build/clean/get/config/init） | M1              | 否         |
| M5     | 脚手架扩展与 IDE 配置                                      | M1              | 否         |
| M6     | P2/P3 命令（analyze/format/upgrade/env/add）+ 回归验收     | M4              | 否         |

每步结束跑 `just check`，Windows 侧回归 `just windows check`。

---

## 6. 风险与开放问题

- 网络：`download.qt.io` 可达性；需要额外支持`https://mirrors.ustc.edu.cn/qtproject/`, `https://mirrors.tuna.tsinghua.edu.cn/qt/`，内部镜像路径都不同
- 许可证：仅下载开源归档（LGPL），不涉及商业安装器授权流程；在 `doctor` / `setup` 文案中注明。
- Windows：MSVC 环境注入仍由宿主脚本负责。

## 7. 建议开工顺序

M1 -> M2 -> M3
