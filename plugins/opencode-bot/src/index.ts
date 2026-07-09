import { parseStdin, sendResponse, sendList, sendLog, setPluginAddr, pluginAddr } from "./yse.js";
import {
  initBot,
  getUserState,
  sendPromptStreaming,
  fetchAllSessions,
  getSessionInfo,
  listModels,
  listSkills,
  listAgents,
  killServer,
} from "./opencode.js";
import * as fs from "fs";
import * as path from "path";

const HELP = `可用命令：
发消息 → 发送给 AI（需先用 /select 或 /new 选择会话）
/sessions     — 按项目目录选择会话（点选切换）
/select <id>  — 直接按 ID 切换会话
/new [标题] [目录] — 新建会话（可选指定目录）
/info         — 当前会话详情
/abort        — 中止当前生成
/undo         — 撤回上一条消息
/redo         — 恢复撤回
/messages [n] — 查看最近 n 条消息（默认 5）
/models       — 列出可用模型（点选用该模型新建会话）
/variants     — 列出当前模型可用 variant（点选用该 variant 新建会话）
/plan         — 列出可用 plan（点选用该 plan 新建会话）
/skills       — 列出可用 skill
/project      — 当前项目信息
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
      const params = msg.params as any;
      if (params.virtual_addr) {
        setPluginAddr(params.virtual_addr);
      }
      const dir = params.state_dir as string | undefined;
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

    // Use incoming message's "to" as our identity if config didn't provide one
    if (!pluginAddr) setPluginAddr(msg.params.to);

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
    } else if (us.sessionId) {
      // SDK mode — send prompt with real-time tool event streaming
      const reply = await sendPromptStreaming(
        state.client, us.sessionId, text, state.projectDir,
        { modelId: us.modelId, providerId: us.providerId, variant: us.modelVariant, agentId: us.agentId },
        makeEventHandler(from),
      );
      sendResponse(from, reply);
    } else {
      sendResponse(from, "请先选择会话：/sessions 或 /new [标题]");
    }
  }
}

// ---- Event formatting for streaming ----

function formatToolInput(input: any): string {
  if (!input) return "";
  const keys = Object.keys(input);
  if (keys.length === 0) return "";
  const parts = keys.slice(0, 3).map((k) => {
    const v = input[k];
    if (typeof v === "string") return `${k}=${v.slice(0, 80)}`;
    if (typeof v === "number" || typeof v === "boolean") return `${k}=${v}`;
    return `${k}=${JSON.stringify(v).slice(0, 80)}`;
  });
  let s = parts.join(", ");
  if (keys.length > 3) s += ", …";
  return s;
}

function formatToolContent(content: any[]): string {
  if (!content || !content.length) return "";
  return content
    .filter((c: any) => c.type === "text")
    .map((c: any) => (c.text || "").slice(0, 1500))
    .filter(Boolean)
    .join("\n");
}

function makeEventHandler(from: string) {
  return (type: string, data: any) => {
    switch (type) {
      case "tool_called":
        sendResponse(from, `🔧 ${data.name}(${formatToolInput(data.input)})`);
        break;
      case "tool_progress": {
        const t = (data.text || "").slice(0, 1000);
        if (t) sendResponse(from, `📝 ${t}`);
        break;
      }
      case "tool_success": {
        let msg = `✅ ${data.name} 完成`;
        const content = formatToolContent(data.content);
        if (content) msg += `\n${content.slice(0, 2000)}`;
        sendResponse(from, msg);
        break;
      }
      case "tool_failed":
        sendResponse(from, `❌ ${data.name} 失败: ${data.error?.message ?? JSON.stringify(data.error).slice(0, 500)}`);
        break;
    }
  };
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
        await state.client.session.abort({ sessionID: us.sessionId });
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
        await state.client.session.revert({ sessionID: us.sessionId });
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
        await state.client.session.unrevert({ sessionID: us.sessionId });
        sendResponse(from, "✅ 已恢复撤回");
      } catch (e: any) {
        sendResponse(from, `恢复失败: ${e.message ?? e}`);
      }
      break;

    case "messages":
      await cmdMessages(state, from, us, arg);
      break;

    case "models": {
      const list = await listModels(state);
      if (list.length === 0) {
        sendResponse(from, "暂无可用模型");
        break;
      }
      sendList(from, "请选择模型（将用该模型新建会话）：", "可用模型", list);
      break;
    }

    case "variants": {
      if (!us.sessionId) {
        sendResponse(from, "请先选择会话");
        break;
      }
      const info = await getSessionInfo(state.client, us.sessionId);
      const listVariants = async (): Promise<{ label: string; value: string; description: string }[]> => {
        try {
          const res = await fetch(`${state.baseUrl}/api/model`);
          const data = await res.json();
          const items: any[] = Array.isArray(data) ? data : (data?.data ?? []);
          return items.map((m: any) => ({
            label: `${m.id ?? "?"} / ${m.variant ?? "default"}`,
            value: JSON.stringify({ model: m.id, provider: m.providerID, variant: m.variant }),
            description: `provider: ${m.providerID ?? "?"}`,
          }));
        } catch {
          return [];
        }
      };
      const vlist = await listVariants();
      if (vlist.length === 0) {
        sendResponse(from, "暂无可用 variant");
        break;
      }
      sendList(from, "请选择 variant（将用该 model+variant 新建会话）：", "可用 Variant", vlist);
      break;
    }

    case "plan": {
      if (us.planMode) {
        us.planMode = false;
        us.agentId = undefined;
        sendResponse(from, "✅ 已切换为默认模式");
      } else {
        const list = await listAgents(state);
        if (list.length === 0) {
          sendResponse(from, "暂无可用 plan");
          break;
        }
        us.planMode = true;
        us.agentId = JSON.parse(list[0].value).agent;
        sendResponse(from, "✅ 已切换为 plan 只读规划模式");
      }
      saveStateImpl(state);
      // If there's follow-up text and an active session, send as prompt
      if (arg && us.sessionId) {
        const reply = await sendPromptStreaming(
          state.client, us.sessionId, arg, state.projectDir,
          { modelId: us.modelId, providerId: us.providerId, variant: us.modelVariant, agentId: us.agentId },
          makeEventHandler(from),
        );
        sendResponse(from, reply);
      }
      break;
    }

    case "skills": {
      const list = await listSkills(state);
      if (list.length === 0) {
        sendResponse(from, "暂无可用 skill");
        break;
      }
      sendResponse(from, list.map((s) => `• ${s.label}\n  ${s.description}`).join("\n\n"));
      break;
    }

    case "project":
      await cmdProject(state, from);
      break;

    default:
      sendResponse(from, `未知命令: /${cmd}\n输入 /help 查看可用命令`);
  }
}

async function cmdSessions(state: any, from: string) {
  const sessions = await fetchAllSessions(state.client);
  if (sessions.length === 0) {
    sendResponse(from, "暂无会话，输入 /new [标题] 创建一个");
    return;
  }

  // Group by directory
  const groups = new Map<string, any[]>();
  for (const s of sessions) {
    const dir = s.directory || "";
    if (!groups.has(dir)) groups.set(dir, []);
    groups.get(dir)!.push(s);
  }

  process.stderr.write(`[opencode-bot] cmdSessions: ${sessions.length} sessions, ${groups.size} dirs: ${JSON.stringify(Array.from(groups.keys()))}\n`);

  if (groups.size === 1) {
    const dir = Array.from(groups.keys())[0];
    // Single directory with sessions — show session list
    const list = sessions.map(formatSessionItem);
    sendList(from, dir ? `请选择会话（${path.basename(dir)}）：` : "请选择会话：", "可选会话", list);
    return;
  }

  // Multiple directories — show directory list first
  const dirList = Array.from(groups.entries()).map(([dir, sList]) => ({
    label: dir ? path.basename(dir) : "(默认)",
    value: `dir:${dir}`,
    description: `📁 ${dir || "(默认)"} — ${sList.length} 个会话`,
  }));
  sendList(from, "请选项目目录：", "项目目录", dirList);
}

function formatSessionItem(s: any): { label: string; value: string; description: string } {
  return {
    label: s.title || s.id?.slice(0, 8) || "Untitled",
    value: s.id,
    description: s.directory
      ? `📁 ${s.directory}`
      : `🕐 ${s.updatedAt ? new Date(s.updatedAt as number).toLocaleDateString("zh-CN") : "?"}`,
  };
}

async function cmdNew(state: any, from: string, us: any, arg: string) {
  try {
    // Last word starting with / is the directory, rest is title
    let title = arg;
    let dir = state.projectDir;
    const lastSpace = arg.lastIndexOf(" ");
    if (lastSpace >= 0) {
      const last = arg.slice(lastSpace + 1);
      if (last.startsWith("/")) {
        title = arg.slice(0, lastSpace).trim();
        dir = last;
      }
    }
    const result = await state.client.session.create({
      title: title || undefined,
      directory: dir,
    } as any);
    const sessionId = (result.data as any)?.id;
    if (!sessionId) {
      sendResponse(from, "创建会话失败：无返回 ID");
      return;
    }
    us.sessionId = sessionId;
    state.projectDir = dir;
    const info = await getSessionInfo(state.client, sessionId);
    sendResponse(
      from,
      `✅ 已新建会话「${title || "(无标题)"}」\n${info}`,
    );
    saveStateImpl(state);
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
    const result = await state.client.session.messages({ sessionID: us.sessionId });
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
  // Directory selection from /sessions flow
  if (value.startsWith("dir:")) {
    const dir = value.slice(4);
    const sessions = await fetchAllSessions(state.client);
    const filtered = sessions.filter(
      (s: any) => (s.directory || "") === dir,
    );
    if (filtered.length === 0) {
      sendResponse(from, "该目录下暂无会话");
      return;
    }
    sendList(from, "请选择会话：", "可选会话", filtered.map(formatSessionItem));
    return;
  }
  // Check if value is a JSON config (model/variant/plan selection)
  if (value.startsWith("{")) {
    let config: any;
    try { config = JSON.parse(value); } catch { return; }
    try {
      // Model/variant/plan selection — update local state if session exists
      if (us.sessionId) {
        if (config.model) us.modelId = config.model;
        if (config.provider) us.providerId = config.provider;
        if (config.variant) us.modelVariant = config.variant;
        if (config.agent) us.agentId = config.agent;
        sendResponse(from, "✅ 已设置，下一条消息将使用新配置");
        saveStateImpl(state);
        return;
      }
      // Agent/plan without session: ask to select one first
      if (config.agent && !config.model) {
        sendResponse(
          from,
          "请先用 /sessions 选择或 /new 创建会话后再选 plan",
        );
        return;
      }
      // No active session → create new one with the chosen config
      const body: any = {};
      if (config.model) {
        body.model = { id: config.model, providerID: config.provider };
        if (config.variant) body.model.variant = config.variant;
      }
      if (config.agent) body.agent = config.agent;
      if (!body.model && !body.agent) body.agent = config;

      const title = config.model
        ? `${config.model}${config.variant ? ` (${config.variant})` : ""}`
        : config.agent ?? "new";
      const result = await state.client.session.create({
        title,
        directory: state.projectDir,
        ...body,
      });
      const newId = (result.data as any)?.id;
      if (!newId) { sendResponse(from, "创建失败"); return; }
      us.sessionId = newId;
      us.mode = "sdk";
      const info = await getSessionInfo(state.client, newId);
      sendResponse(from, `✅ 已新建会话「${title}」\n${info}`);
      saveStateImpl(state);
    } catch (e: any) {
      sendResponse(from, `创建失败: ${e.message ?? e}`);
    }
    return;
  }
  // Session list selection
  us.sessionId = value;
  us.mode = "sdk";
  saveStateImpl(state);
  const info = await getSessionInfo(state.client, value);
  sendResponse(from, `✅ 已切换\n${info}`);
}

main().catch((e) => {
  process.stderr.write(`[opencode-bot] fatal: ${e}\n`);
  process.exit(1);
});

process.on("exit", () => {
  // Cleanup: kill the opencode server on exit
  // Note: state is not accessible here, but the server will be orphaned
  // which is fine — opencode serve exits when its parent pipe closes.
});
