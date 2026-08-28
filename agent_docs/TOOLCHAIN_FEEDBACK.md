# 工具链一线反馈：qitu（Flagchip IDE）使用 Gansi/Yse 踩坑汇总

来源：`~/Playground/qitu`（Flagchip IDE，基于 Yse 的嵌入式 MCAL 配置 /
GHS 构建 / Ozone 调试工作台）在原型引导与首次打包过程中记录的工具链
摩擦。原始记录见 qitu `docs/agent/toolchain-blockers.md`（2026-08-17），
本文件按"对 Yse/Gansi 改进"的角度汇总，供后续开发排期参考。

qitu 的依赖方式：`yse = { git = ..., tag = "gansi-v0.1.1" }`，远程钉死在
标签上。堵点分为三类：安装与离线可用性、配置与打包的隐性假设、运行期
语义（取消、跨平台、厂商工具集成）。

## 一、安装与离线可用性

### 1. `cargo install` 默认安装目录只读

现象：安装 gansi 时试图写 `~/.cargo/.crates.toml` 失败（只读文件系统），
编译已开始但二进制未上 PATH。

绕行办法：`cargo install --root /tmp/qitu-gansi --git ... --tag gansi-v0.1.1 gansi --locked`。

建议：文档写明可写的安装根，或支持项目本地工具缓存 / 显式 `GANSI_BIN`
路径；不要假设 `~/.cargo/bin` 可写。

### 2. `gansi create --local` 并非真正离线

现象：给了本地 Yse 检出后，脚手架流程仍去更新 `ustc` 镜像
（`mirrors.ustc.edu.cn` DNS 失败），staging 目录被删除，项目没生成。

建议：提供显式离线模式——用已有检出与 Cargo 缓存、不更新 registry、
校验失败也保留生成的项目、把"脚手架成功"与"依赖校验"分开报告。

### 3. 远程 Git 依赖无网络时无法校验

现象：`cargo check --locked` 尝试刷新 GitHub 源失败，编译无法进入应用
代码。三种可接受的运行模式：联网 fetch；预置 Cargo Git 缓存 + `--offline`；
临时的本地 path 覆盖（仅用于校验，提交清单保持钉死远程标签）。

## 二、配置与打包的隐性假设

### 4. 默认项目配置与宿主机不对齐

现象：`gansi init` 生成 Qt `6.10.2` 钉死，但各开发机 Qt 布局不同；
`doctor` 能检测系统 Qt，却不能精确指出缺哪个根/版本。

建议：`gansi.toml` 显式钉 Qt 版本；`doctor` 输出具体缺失的根目录与
版本号，而不是笼统说 Qt 不可用。

### 5. 打包存在隐性必需文件

现象：`gansi build --debug` 因缺 `THIRD_PARTY_NOTICES.txt` 与 Linux 部署
资产而中止，且是编译完成后才报缺 notice 文件。

建议：打包前置条件由 `doctor` 提前暴露或由 `init` 生成；区分必需元数据
与可选品牌资产。

### 6. Windows 互操作被执行沙箱掩盖

现象：Windows PE 进程在受限沙箱里报 `UtilBindVsockAnyPort`，看似 WSL
互操作坏了；放行后证明互操作正常。另有两个真实的 Windows 要点：

- `QMAKE` 需要显式 `WSLENV=QMAKE/p` + WSL 路径值，Bash 里直接赋
  Windows 形式路径不会正确传给 PE 子进程；
- Qt `bin` 目录必须进原生 Windows `PATH`，否则测试/应用进程以
  `STATUS_DLL_NOT_FOUND` 终止。

建议：IDE 在普通 Linux 构建里应如实报告宿主机不匹配，不把放行后的
自动化桥当作普通 Linux 部署能力。

## 三、运行期语义

### 7. Yse 任务取消是协作式的

现象：`Task::cancel()` 只设取消令牌，不会强停阻塞操作。若适配器直接用
`std::process::Command::output`，界面显示已取消但 EB/GHS 还在后台跑。

qitu 的应对（也是适配器契约）：轮询子进程、读线程排空 stdout/stderr、
令牌置位时 kill 子进程；Unix 上先建独立进程组再启动，Windows 用
`taskkill /T /F` 处理进程树。另外任务 API 交付的是完成结果而非原生
输出流，qitu 自建了适配器拥有的线程安全事件缓冲 + GUI 线程泵，才能让
EB/GHS 实时行进入活动面板。

建议：文档明示"长时间厂商集成必须轮询令牌或自备可取消边界"；若产品
方向允许，可考虑输出流式交付。

### 8. GHS profile 语义分裂在 CLI 与项目图之间

现象：`gbuild.exe` 暴露 `-clean`/`-cleanfirst`，`clean`/`rebuild` 模式映射
到这两个开关；目标/配置由 `.gpj` 项目图声明，没有可靠的额外目标 CLI
标志。qitu 把 target 保留为可审计的项目元数据，不发明命令行覆盖；
`output_dir` 用于定位构建产物。原生验证用 GHS 2022.1.4 成功产出 ELF。

### 9. 参考 EB 项目没有 tresos 配置关联

现象：按文档命令跑 `tresos_cmd.sh ... verify <project>` 返回错误 11001
"没有配置项目关联"。样本自带 `gyx-eb.exe` + `gyx-eb.json`，那才是
项目本地的 XDM/代码生成路径，其 `{"func":"generate_code"}` JSON 数组请求
才是原生生成方式；`[{"func":"find_module"}]` 可发现 31 个模块。GYX 拒绝
更直观的单对象形式，且该契约只写在随附的 skill 与可执行文件用法文本里。

### 10. `gansi setup` 会把错误的 Qt 版本登记为成功

现象：Windows 上 `gansi setup 6.10.2` 找到已有托管 Qt 6.8.3 根就直接
成功，未检查 qmake 的真实版本，破坏了 `qt_version = "6.10.2"` 钉住。
6.8.3 能编译原型，但只是兼容性绕行，不是可复现 6.10.2 构建的证据。

另外：单一 `compiler_family` 项目钉住对 Linux/GCC + Windows/MSVC 双宿主
工作区不便，需要宿主相关覆盖或工具链列表。

## 四、影响 IDE 计划的总体不便

- 工具引导有太多隐式全局假设：Cargo home、registry 镜像、Git DNS、
  Qt 根、平台 PATH。
- Gansi 命令把项目生成、依赖解析、校验混在一起；流程后段失败会抹掉
  有用的脚手架并掩盖真实原因。
- "远程 Yse" 只在 Git 标签与 Cargo 缓存都可用时可复现；应展示清晰环境
  状态而非不透明构建错误。
- 打包在必需 notice/部署元数据缺失时后段失败，小应用错误看起来像
  工具链故障。

## 五、建议的后续行动（对 Yse/Gansi）

1. 保持提交清单里的 Yse 依赖远程且钉死 `gansi-v0.1.1`。
2. `gansi` 增加项目本地工具缓存路径或文档化临时缓存，应对全局 Cargo
   目录只读。
3. `gansi doctor` 快照进 IDE 的 Toolchain 视图，原始输出作为可展开细节
   而非主错误。
4. Windows 构建桥显式化：翻译后的 `QMAKE`、Qt DLL 进原生 `PATH`、
   Windows 本地 Cargo 目标目录。
5. 把 GYX 发现与成功 GHS 样例构建当作原生适配器证据；生成（会改写生成
   文件）单独验证；Ozone/J-Link 需接硬件验证。
