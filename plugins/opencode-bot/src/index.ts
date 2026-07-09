import { parseStdin, sendResponse, sendList, sendLog } from "./yse.js";
import {
  initBot,
  getUserState,
  sendPrompt,
  sendTuiPrompt,
  listSessions,
  getSessionInfo,
} from "./opencode.js";
import * as fs from "fs";
import * as path from "path";

const HELP = `可用命令：
发消息 → 发送给 AI（需先用 /select 或 /new 选择会话）
/sessions     — 列出所有会话（点选切换）
/select <id>  — 直接按 ID 切换会话
/new [标题]    — 新建会话
/info         — 当前会话详情
/abort        — 中止当前生成
/undo         — 撤回上一条消息
/redo         — 恢复撤回
/messages [n] — 查看最近 n 条消息（默认 5）
/tui-connect  — 连接到 TUI 模式
/tui-detach   — 断开 TUI 模式
/tui-status   — TUI 运行状态
/project      — 当前项目信息
/dir <path>   — 切换项目目录
/help         — 显示此帮助`;

let stateFile = "";

function saveStateImpl(s: any) {
  if (!stateFile || !s) return;
  try {
    fs.mkdirSync(path.dirname(stateFile), { recursive: true });
    fs.writeFileSync(
      stateFile,
      JSON.stringify({
        sessions: s.sessions,
        projectDir: s.projectDir,
      }),
    );
  } catch {}
}

async function main() {
  sendLog("info", "opencode-bot starting...");
  let state: Awaited<ReturnType<typeof initBot>>;
  try {
    state = await initBot();
  } catch (e: any) {
    sendLog("error", `failed to connect to OpenCode: ${e.message ?? e}`);
    sendLog(
      "info",
      "opencode-bot running in degraded mode — OpenCode unavailable",
    );
    state = null as any;
  }

  for await (const msg of parseStdin()) {
    if (msg.method === "config") {
      const dir = (msg.params as any)?.state_dir as string | undefined;
      if (dir) {
        stateFile = path.join(dir, "sessions.json");
        try {
          const raw = fs.readFileSync(stateFile, "utf-8");
          const saved = JSON.parse(raw);
          if (state && saved.sessions) state.sessions = saved.sessions;
          if (state && saved.projectDir) state.projectDir = saved.projectDir;
          sendLog("info", `loaded state from ${stateFile}`);
        } catch { /* no state file yet */ }
      }
      continue;
    }

    if (msg.method !== "message") continue;
    const text = msg.params.text?.trim();
    const from = msg.params.from;
    if (!text) continue;

    // Handle list selection response from user
    const respValue = msg.params.meta?.plugin?.response?.value;
    if (respValue) {
      await handleListResponse(state, from, respValue);
      saveStateImpl(state);
      continue;
    }

    // Help trigger (protocol: ? or ？)
    if (text === "?" || text === "？") {
      sendResponse(from, HELP);
      continue;
    }

    // No OpenCode connection — only allow /help
    if (!state) {
      if (text === "/help") {
        sendResponse(from, HELP);
      } else {
        sendResponse(
          from,
          "OpenCode 未连接，无法处理。输入 ? 或 /help 查看可用命令。",
        );
      }
      continue;
    }

    const us = getUserState(state, from);

    // Command routing
    if (text.startsWith("/")) {
      const [cmd, ...args] = text.slice(1).split(/\s+/);
      await handleCommand(state, from, us, cmd, args.join(" "));
    } else if (us.mode === "tui") {
      // TUI mode — append to TUI input and submit
      try {
        await sendTuiPrompt(state.client, text);
        sendResponse(from, "✅ 已发送到 TUI，请等待 AI 回复...");
      } catch (e: any) {
        sendResponse(from, `TUI 发送失败: ${e.message ?? e}`);
      }
    } else if (us.sessionId) {
      // SDK mode — send prompt directly
      const reply = await sendPrompt(state.client, us.sessionId, text);
      sendResponse(from, reply);
    } else {
      sendResponse(from, "请先选择会话：/sessions 或 /new [标题]");
    }
  }
}

// ---- Command handlers ----

async function handleCommand(
  state: any,
  from: string,
  us: any,
  cmd: string,
  arg: string,
) {
  switch (cmd) {
    case "help":
      sendResponse(from, HELP);
      break;

    case "sessions":
      await cmdSessions(state, from);
      break;

    case "select":
      if (!arg) {
        sendResponse(from, "用法: /select <session_id>");
        break;
      }
      us.sessionId = arg;
      us.mode = "sdk";
      const info = await getSessionInfo(state.client, arg);
      sendResponse(from, `✅ 已切换\n${info}`);
      saveStateImpl(state);
      break;

    case "new":
      if (us.mode === "tui") us.mode = "sdk";
      await cmdNew(state, from, us, arg);
      saveStateImpl(state);
      break;

    case "info":
      if (!us.sessionId) {
        sendResponse(from, "未选择会话");
        break;
      }
      sendResponse(from, await getSessionInfo(state.client, us.sessionId));
      break;

    case "abort":
      if (!us.sessionId) {
        sendResponse(from, "未选择会话");
        break;
      }
      try {
        await state.client.session.abort({ path: { id: us.sessionId } });
        sendResponse(from, "✅ 已中止");
      } catch (e: any) {
        sendResponse(from, `中止失败: ${e.message ?? e}`);
      }
      break;

    case "undo":
      if (!us.sessionId) {
        sendResponse(from, "未选择会话");
        break;
      }
      try {
        await state.client.session.revert({ path: { id: us.sessionId } });
        sendResponse(from, "✅ 已撤回上一条消息");
      } catch (e: any) {
        sendResponse(from, `撤回失败: ${e.message ?? e}`);
      }
      break;

    case "redo":
      if (!us.sessionId) {
        sendResponse(from, "未选择会话");
        break;
      }
      try {
        await state.client.session.unrevert({ path: { id: us.sessionId } });
        sendResponse(from, "✅ 已恢复撤回");
      } catch (e: any) {
        sendResponse(from, `恢复失败: ${e.message ?? e}`);
      }
      break;

    case "messages":
      await cmdMessages(state, from, us, arg);
      break;

    case "tui-connect":
      us.mode = "tui";
      sendResponse(from, "✅ 已连接到 TUI 模式，后续消息将发送到 TUI 输入框");
      saveStateImpl(state);
      break;

    case "tui-detach":
      us.mode = "sdk";
      sendResponse(from, "✅ 已断开 TUI 模式，回到 SDK 直连模式");
      saveStateImpl(state);
      break;

    case "tui-status":
      sendResponse(
        from,
        `TUI 模式: ${us.mode === "tui" ? "已连接" : "未连接"}\n会话: ${us.sessionId ?? "未选择"}`,
      );
      break;

    case "project":
      await cmdProject(state, from);
      break;

    case "dir":
      if (!arg) {
        sendResponse(from, "用法: /dir <path>");
        break;
      }
      state.projectDir = arg;
      us.sessionId = null;
      sendResponse(from, `✅ 已切换到目录: ${arg}\n请用 /new 或 /select 选择会话`);
      saveStateImpl(state);
      break;

    default:
      sendResponse(from, `未知命令: /${cmd}\n输入 /help 查看可用命令`);
  }
}

async function cmdSessions(state: any, from: string) {
  const list = await listSessions(state.client);
  if (list.length === 0) {
    sendResponse(from, "暂无会话，输入 /new [标题] 创建一个");
    return;
  }
  sendList(from, "请选择会话：", "可选会话", list);
}

async function cmdNew(state: any, from: string, us: any, title: string) {
  try {
    const result = await state.client.session.create({
      body: {
        title: title || undefined,
      },
    } as any);
    const sessionId = (result.data as any)?.id;
    if (!sessionId) {
      sendResponse(from, "创建会话失败：无返回 ID");
      return;
    }
    us.sessionId = sessionId;
    sendResponse(
      from,
      `✅ 已新建会话「${title || "(无标题)"}」\nID: ${sessionId}\n现在可以发送消息了`,
    );
  } catch (e: any) {
    sendResponse(from, `创建会话失败: ${e.message ?? e}`);
  }
}

async function cmdMessages(state: any, from: string, us: any, arg: string) {
  if (!us.sessionId) {
    sendResponse(from, "未选择会话");
    return;
  }
  const limit = parseInt(arg, 10) || 5;
  try {
    const result = await state.client.session.messages({
      path: { id: us.sessionId },
    } as any);
    const msgs = (result.data as any[]) ?? [];
    const recent = msgs.slice(-limit);
    if (recent.length === 0) {
      sendResponse(from, "暂无消息");
      return;
    }
    const lines = recent.map((m: any, i: number) => {
      const role = m.info?.role ?? "?";
      const text =
        m.parts
          ?.filter((p: any) => p.type === "text")
          .map((p: any) => p.text)
          .join(" ")
          .slice(0, 200) ?? "";
      return `${i + 1}. [${role}] ${text}`;
    });
    sendResponse(from, lines.join("\n\n"));
  } catch (e: any) {
    sendResponse(from, `获取消息失败: ${e.message ?? e}`);
  }
}

async function cmdProject(state: any, from: string) {
  try {
    const p = await state.client.project.current();
    const data = p.data as any;
    if (!data) {
      sendResponse(from, "未获取到项目信息");
      return;
    }
    const lines: string[] = [];
    if (data.name) lines.push(`📁 项目: ${data.name}`);
    if (data.directory) lines.push(`📂 目录: ${data.directory}`);
    lines.push(`📍 当前目录: ${state.projectDir}`);
    sendResponse(from, lines.join("\n"));
  } catch {
    sendResponse(from, "获取项目信息失败");
  }
}

async function handleListResponse(state: any, from: string, value: string) {
  const us = getUserState(state, from);
  us.sessionId = value;
  us.mode = "sdk";
  const info = await getSessionInfo(state.client, value);
  sendResponse(from, `✅ 已切换\n${info}`);
}

main().catch((e) => {
  process.stderr.write(`[opencode-bot] fatal: ${e}\n`);
  process.exit(1);
});
