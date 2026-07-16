# opencode-bot 插件

通过 YSE 聊天控制 OpenCode。让手机能远程操作 OpenCode AI 编程助手。

## 构建

```sh
cd plugins/opencode-bot && npm install && npm run build
# 产物: plugins/opencode-bot/dist/index.js
```

启动方式: `node plugins/opencode-bot/dist/index.js`

## 命令

| 命令            | 说明                  | 交互               |
| --------------- | --------------------- | ------------------ |
| 任意文字        | 发送给当前 session AI | 文字               |
| `/sessions`     | 列出所有会话          | 列表组件——点选切换 |
| `/select <id>`  | 按 ID 切换会话        | 文字               |
| `/new [标题]`   | 新建会话              | 文字               |
| `/info`         | 当前会话详情          | 文字               |
| `/abort`        | 中止 AI 生成          | 文字               |
| `/undo`         | 撤回上一条            | 文字               |
| `/redo`         | 恢复撤回              | 文字               |
| `/messages [n]` | 最近 n 条消息         | 文字               |
| `/tui-connect`  | 接入 TUI 模式         | 文字               |
| `/tui-detach`   | 断开 TUI              | 文字               |
| `/tui-status`   | TUI 状态              | 文字               |
| `/project`      | 当前项目信息          | 文字               |
| `/dir <path>`   | 切换项目路径          | 文字               |
| `/help`         | 帮助                  | 文字               |

## 架构

### 通信

- 通过 `@opencode-ai/sdk/v2` 连接本地 OpenCode 服务（localhost:4096）
- 使用 `createOpencodeClient()`（仅客户端，不启动新服务）
- stdin/stdout JSON-RPC 行协议

### 两种模式

**SDK 模式（默认）**：

- `client.session.prompt()` 直接发送给 AI
- 适合一次性问题，等待完整回答

**TUI 模式**：

- `client.tui.appendPrompt()` + `client.tui.submitPrompt()` 写入 TUI 输入框并提交
- 适合需要 TUI 工具调用、实时交互的场景
- 消息通过 `session.messages()` 轮询获取

### Session 管理

- 每个 `from_addr`（用户）独立维护 sessionId + mode
- `/sessions` 使用列表组件呈现，点选切换
- `/new` 创建新 session，自动设为当前

### 依赖

- `@opencode-ai/sdk` — OpenCode TypeScript SDK

## exec 工具已知问题

exec 工具（`opencode-tools/exec.ts`）通过 tmux session 执行命令，每次调用生成随机 PS1 标记（`__YSE_<uuid>__`），用 `capture-pane -p -J` 捕获输出后解析命令和 PS1 间的行。

### sleep 阻塞 shell

AI 在 exec 中运行 `sleep N` 等长时间阻塞命令会导致后面的所有 exec 调用排队等待 shell 空闲。exec 的 `waitFor` 在 120 秒无变化后返回部分输出，此时 PS1 可能仍未出现，解析失败返回 `[PARSE_ERR]` 或空。AI 看到空输出后迷惑。

**避免方式：**
- 不要用 `sleep` 等后台进程——用 `tmux send-keys C-c` 中断当前命令再发新命令（exec 未实现）
- 轮询后台进程用工具内的 `watch` 或 `while` 循环，而不是多个串行 exec 调用
- 如果 shell 卡住，手动 `/abort` 后重新发消息
