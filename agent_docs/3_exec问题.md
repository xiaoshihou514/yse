# exec 工具规范合规性修复计划

## 一、问题总览

通过全面比对规范文档与现有代码，发现以下 **8 类**行为偏差，其中 **4 项严重**（直接影响核心功能可用性），**2 项中等**（影响健壮性或体验），**2 项轻微**（与规范描述存在差异但不影响主要流程）。所有问题均需修复以达到完全合规。

| 编号 | 问题分类 | 严重性 | 影响范围 |
|------|----------|--------|----------|
| P1 | 超时判定逻辑错误（使用“输出变化”而非“绝对时长”） | **严重** | 持续输出命令永不超时，阻塞工具 |
| P2 | SSH 连接中断后不重建，无错误恢复 | **严重** | 远程任务在断网后静默失败，不可靠 |
| P3 | 查询任务时窗格存在性检查使用错误命令（`has-session` 误用） | **严重** | 所有后台任务查询均失败，功能完全不可用 |
| P4 | `ensureSession` 未保证 `main` 窗口在外部删除后重建 | **中等** | 外部操作（如用户手动关闭）破坏会话后，工具无法自愈 |
| P5 | `queryTask` 的 `AbortSignal` 触发未携带 `task_id` | **轻微** | 上层无法获得中断任务的标识，与 `executeCommand` 行为不一致 |
| P6 | 轮询间隔 150ms 与规范建议 200ms 不符 | **轻微** | 非功能性差异，可接受但未严格对齐 |
| P7 | 降级后新 `main` 窗口未激活（使用 `-d`） | **中等** | 人类 attach 后的直观体验与“打开新终端”略有偏差 |
| P8 | 任务自动清理机制（闲置 1 小时）未实现 | **可选** | 规范提及但非强制，可作为增强项 |

---

## 二、修复方案

### P1：超时判定逻辑修正（严重）

**当前行为**：以“输出在 2 分钟内无变化”作为超时条件，导致持续输出命令永不超时。

**规范要求**：从命令发送开始，绝对等待时间达到 2 分钟即触发降级或返回 `running`。

**修复方案**：
- 在 `executeCommand` 和 `queryTask` 中，将计时基准改为**从函数进入时起的绝对时间戳**。
- 移除 `lastChange` 重置逻辑，改用 `Date.now() - start >= MAX_STALE_MS` 作为唯一超时判定。
- 保留输出变化监控仅用于“命令是否仍在运行”的参考，但不再影响超时触发。

```diff
  const start = Date.now();
- let lastChange = start;
  let prev = "";
  while (true) {
    // ...
-   if (out !== prev) { prev = out; lastChange = Date.now(); }
-   else if (Date.now() - lastChange > MAX_STALE_MS) { ... }
+   if (Date.now() - start > MAX_STALE_MS) {
+     // 超时，触发降级或返回 running
+   }
  }
```

---

### P2：SSH 连接健康检查与自动重建（严重）

**当前行为**：仅在首次调用 `ensureSSHMaster` 时建立控制连接，后续所有 SSH 操作均不检查连接状态，断线后静默失败。

**规范要求**：远程服务器断开时应尝试重建 SSH 连接；失败则返回连接错误。

**修复方案**：
1. 抽取 `checkSSHMaster(server, sid)` 函数，调用 `ssh -O check -S <control> <server>`。
2. 在每次调用 `tmuxProc` 执行 SSH 命令前，先检查控制连接，若异常则尝试重建（调用 `ensureSSHMaster`，若仍失败则抛出明确错误）。
3. 在 `tmuxProc` 内部检查 `spawnSync` 的返回码，若非 0 且 stderr 包含连接错误，则向上层抛出可恢复的错误，由上层决定重试。

```ts
function ensureControlAlive(server: string, sid: string): string {
  const cp = sshControlPath(server, sid);
  const check = spawnSync("ssh", ["-O", "check", "-S", cp, server], { stdio: "ignore" });
  if (check.status !== 0) {
    // 尝试重建主连接
    spawnSync("ssh", ["-M", "-S", cp, "-f", "-N", server], { stdio: "ignore", timeout: 10000 });
    // 再次验证
    const retry = spawnSync("ssh", ["-O", "check", "-S", cp, server], { stdio: "ignore" });
    if (retry.status !== 0) throw new Error(`无法建立 SSH 连接到 ${server}`);
  }
  return cp;
}
```

---

### P3：任务窗格存在性检查命令修正（严重）

**当前行为**：使用 `has-session -t "yse:task-N"` 检查窗格，但该命令期望的是会话名，导致总是失败，所有任务都被删除。

**规范要求**：根据 `task_id` 定位对应窗格（如 `task-1`），若不存在则返回错误。

**修复方案**：
- 改用 `has-window -t yse:task-N` 或 `list-windows -t yse -F "#{window_name}"` 检查窗口是否存在。
- 或直接通过 `capture-pane -t yse:task-N` 并捕获错误状态来判断。

```diff
  const paneCheck = tmuxProc(
-   ["-S", sock, "has-session", "-t", task.pane],
+   ["-S", sock, "has-window", "-t", `yse:${task.pane}`], // task.pane 应为 "task-N"
    server,
    { controlPath }
  );
```
- 需要将存储的 `pane` 字段改为仅存储窗口名（如 `"task-1"`）而不带 `"yse:"` 前缀，以便与其他 tmux 命令一致。

---

### P4：确保 `main` 窗口始终存在（中等）

**当前行为**：`ensureSession` 仅检查索引 0 的窗口名，若名为 `"task-"` 开头则不创建新 `main`，但若 `main` 被外部删除，会话中可能无 `main` 窗口，后续发送命令失败。

**修复方案**：
- 改用 `list-windows -t yse -F "#{window_name}"` 查找是否存在名为 `main` 的窗口。
- 若不存在，则创建新窗口（`new-window -d -n main`）并设为 `remain-on-exit`。
- 若存在，则无需额外操作（或可检查其是否处于空闲状态，但非必需）。

```ts
function ensureMainWindow(sock, server, controlPath, dir) {
  const result = tmuxProc(["-S", sock, "list-windows", "-t", "yse", "-F", "#{window_name}"], server, { controlPath });
  const windows = (result.stdout || "").split("\n").map(s => s.trim());
  if (!windows.includes("main")) {
    // 创建新 main 窗口
    tmuxProc(["-S", sock, "new-window", "-d", "-n", "main", ...(dir ? ["-c", dir] : []), SHELL], server, { controlPath });
    tmuxProc(["-S", sock, "set-option", "-t", "yse:main", "remain-on-exit", "on"], server, { controlPath });
  }
}
```

---

### P5：`queryTask` 中断错误附加 `task_id`（轻微）

**当前行为**：仅在 `executeCommand` 中抛出带 `details.task_id` 的 `AbortError`，`queryTask` 未包含。

**修复方案**：统一两处错误处理，在 `queryTask` 中抛出相同格式的错误，便于上层捕获。

```diff
  if (opts?.signal?.aborted) {
-   const e = new Error("aborted"); e.name = "AbortError"; throw e;
+   const err = new Error(`查询任务 ${taskId} 被中断`);
+   err.name = "AbortError";
+   (err as any).details = { task_id: taskId };
+   throw err;
  }
```

---

### P6：轮询间隔调整至规范建议（轻微）

**当前行为**：`POLL_MS = 150`。

**规范建议**：200ms。

**修复方案**：将常量改为 `200`，以完全对齐规范。

---

### P7：降级后新 `main` 窗口的激活状态（中等）

**当前行为**：使用 `new-window -d` 后台创建，不在会话中切换到新窗口。

**规范意图**：让用户感觉“打开了一个新终端”。人类 attach 时默认进入最后一个活跃窗口，若最后一个活跃窗口是 `task-N`，则看不到新 `main`。

**修复方案**：
- 在创建新 `main` 窗口后，执行 `tmux select-window -t yse:main` 将焦点切换到新窗口（使用 `select-window` 会改变会话的当前窗口，不影响工具自身的 `send-keys`）。
- 但注意，`select-window` 会影响 attach 后的默认视图，这更符合“新终端”的体验。

```diff
  tmuxProc(["-S", sock, "new-window", "-d", "-n", "main", ...], server, { controlPath });
+ tmuxProc(["-S", sock, "select-window", "-t", "yse:main"], server, { controlPath, stdio: "ignore" });
```

---

### P8：任务自动清理机制（可选增强）

**规范提及**：闲置 1 小时后关闭任务窗格。

**当前行为**：无清理机制，任务窗格永久保留。

**修复方案**（可作为后续迭代）：
- 在任务对象中记录最后活动时间（查询或完成时更新）。
- 由后台定期扫描（如每次查询或每 10 分钟）检查哪些任务已完成且超过 1 小时未访问，则执行 `kill-window -t yse:task-N` 并删除记录。
- 对于 `running` 状态任务，可考虑不清理，或记录最后一次输出变化时间作为活跃度指标。

---

## 三、实施顺序与优先级

| 优先级 | 编号 | 预计工作量 |
|--------|------|------------|
| 最高 | P3（查询检查） | 0.5h |
| 最高 | P1（超时逻辑） | 1h |
| 高 | P2（SSH 恢复） | 2h |
| 高 | P4（main 窗口自愈） | 1h |
| 中 | P7（窗口激活） | 0.5h |
| 低 | P5、P6 | 0.5h |
| 可选 | P8 | 2h |

**建议**：优先修复 P3、P1、P2、P4，使核心功能可用且稳定；P7、P5、P6 作为体验优化；P8 视资源和需求决定是否纳入当前迭代。

---

## 四、测试验证要点

修复完成后，应针对以下场景进行回归测试：

- **场景 1**：执行短命令（<2 秒），正常返回输出。
- **场景 2**：执行长命令（>2 分钟），无论输出与否，均在 2 分钟时降级/返回 `running`。
- **场景 3**：查询后台任务，正确获取结果或 `running`，任务窗格存在性检查正常。
- **场景 4**：模拟 SSH 连接断开（如关闭远程 sshd 或 kill 控制进程），工具应尝试重建并成功恢复。
- **场景 5**：外部删除 `main` 窗口后，下次调用自动重建。
- **场景 6**：人类 attach 后看到焦点在新建的 `main` 窗口。

---

以上计划覆盖所有已知偏差，完成后将使插件行为与规范完全一致。请按优先级分步实施。
