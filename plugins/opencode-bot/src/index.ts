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
  resolveModelChain,
  type BotState,
  type ModelSpec,
} from "./opencode.js";
import * as fs from "fs";
import * as path from "path";

const HELP = `可用命令：
发消息 → 发送给 AI（需先用 /select 或 /new 选择会话）
/sessions     — 按项目目录选择会话（点选切换）
/select <id>  — 直接按 ID 切换会话
/new [标题] [目录] — 新建会话（可选指定目录）
/curr         — 查看当前选中的会话和模式
/abort        — 中止当前生成
/undo         — 撤回上一条消息
/models       — 列出可用模型（点选新建会话）
/variants     — 列出当前模型可用 variant（点选用该 variant 新建会话）
/plan         — 列出可用 plan（点选用该 plan 新建会话）
/model        — 查看或配置模型选择策略
/skills       — 列出可用 skill
/help         — 显示此帮助`;

let stateFile = "";
let pendingUserRestore: any = null;
const pendingQuestions = new Map<string, { requestID: string }>();

function saveStateImpl(s: any) {
  if (!stateFile || !s) return;
  try {
    fs.mkdirSync(path.dirname(stateFile), { recursive: true });
    let lastUserState: any = null;
    for (const us of Object.values(s.sessions) as any[]) {
      if (us.sessionId) {
        lastUserState = {
          sessionId: us.sessionId,
          planMode: us.planMode ?? false,
          agentId: us.agentId,
          modelId: us.modelId,
          providerId: us.providerId,
          modelVariant: us.modelVariant,
          modelMode: us.modelMode ?? "global",
        };
        break;
      }
    }
    fs.writeFileSync(
      stateFile,
      JSON.stringify({
        sessions: s.sessions,
        projectDir: s.projectDir,
        lastUserState,
        modelConfig: s.modelConfig,
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
          if (state && saved.modelConfig) state.modelConfig = saved.modelConfig;
          if (state && saved.lastUserState?.sessionId) {
            pendingUserRestore = saved.lastUserState;
          }
          sendLog("info", `loaded state from ${stateFile}`);
        } catch { /* no state file yet */ }
      }
      continue;
    }

    if (msg.method !== "message") continue;
    const text = msg.params.text?.trim();
    const from = msg.params.from;
    if (!text) continue;

    if (!pluginAddr) setPluginAddr(msg.params.to);

    // Handle list selection response from user
    const respValue = msg.params.meta?.plugin?.response?.value;
    if (respValue) {
      // Check if this answers a pending AI question
      const pending = pendingQuestions.get(from);
      if (pending && state) {
        try {
          await state.client.question.reply({
            requestID: pending.requestID,
            answers: [[respValue]],
          });
          pendingQuestions.delete(from);
        } catch (e: any) {
          sendResponse(from, `❌ 提交回答失败: ${e.message ?? e}`);
        }
        continue;
      }
      await handleListResponse(state, from, respValue);
      saveStateImpl(state);
      continue;
    }

    if (text === "?" || text === "？") {
      sendResponse(from, HELP);
      continue;
    }

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

    if (pendingUserRestore && !us.sessionId) {
      const u: any = us;
      u.sessionId = pendingUserRestore.sessionId;
      u.planMode = pendingUserRestore.planMode ?? false;
      u.agentId = pendingUserRestore.agentId;
      u.modelId = pendingUserRestore.modelId;
      u.providerId = pendingUserRestore.providerId;
      u.modelVariant = pendingUserRestore.modelVariant;
      u.modelMode = pendingUserRestore.modelMode ?? "global";
      pendingUserRestore = null;
      saveStateImpl(state);
    }

    // Command routing
    if (text.startsWith("/")) {
      const [cmd, ...args] = text.slice(1).split(/\s+/);
      await handleCommand(state, from, us, cmd, args.join(" "));
    } else if (us.sessionId) {
      const chain = resolveModelChain(us, state.modelConfig);
      pendingQuestions.delete(from);
      const reply = await sendPromptStreaming(
        state.client, us.sessionId, text, state.projectDir,
        chain, us.agentId,
        makeEventHandler(from),
      );
      pendingQuestions.delete(from);
      sendResponse(from, reply);
      sendResponse(from, "✅ 处理完成");
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

function isWriteTool(name: string): boolean {
  return name === "write_to_file" || name === "edit_file" || name === "write_file";
}

function extLang(filePath: string): string {
  const i = filePath.lastIndexOf(".");
  if (i < 0) return "";
  return filePath.slice(i + 1);
}

function formatReadOutput(out: string): string {
  const pathM = out.match(/<path>([^<]*)<\/path>/);
  const path = pathM?.[1] || "";
  const typeM = out.match(/<type>([^<]*)<\/type>/);
  const type = typeM?.[1] || "";
  const entriesM = out.match(/<entries>\n?([\s\S]*?)\n?<\/entries>/);
  const entriesRaw = entriesM?.[1] || "";
  const entries = entriesRaw
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("(") && !l.startsWith("<"));

  if (type === "directory" && entries.length > 0) {
    return `📂 ${path}\n${entries.map((e) => `  ${e}`).join("\n")}`;
  }
  if (type === "file" && entriesRaw) {
    const lang = extLang(path);
    const content = entriesRaw.slice(0, 1000);
    const truncated = entriesRaw.length > 1000 ? "\n... (truncated)" : "";
    return `📄 ${path}\n\`\`\`${lang}\n${content}\n\`\`\`${truncated}`;
  }
  return out;
}

function makeEventHandler(from: string) {
  return (type: string, data: any) => {
    switch (type) {
      case "tool_called": {
        if (data.name === "bash") {
          const cmd = data.input?.command || formatToolInput(data.input);
          sendResponse(from, `🔧 bash\n\`\`\`bash\n${cmd.slice(0, 2000)}\n\`\`\``);
        } else if (isWriteTool(data.name)) {
          const path = data.input?.filePath || "";
          const content = data.input?.content || "";
          sendResponse(from, `🔧 ${data.name}${path ? ` (${path})` : ""}\n\`\`\`diff\n+ ${content.slice(0, 2000)}\n\`\`\``);
        } else if (data.name === "read") {
          const path = data.input?.filePath || data.input?.filePattern || formatToolInput(data.input);
          sendResponse(from, `📂 read: ${path.slice(0, 200)}`);
        } else if (data.name === "question") {
          // handled by question_asked event → sendList
        } else {
          sendResponse(from, `🔧 ${data.name}(${formatToolInput(data.input)})`);
        }
        break;
      }
      case "tool_success": {
        const out = (data.output || "").slice(0, 2000);
        if (data.name === "bash" && !out) break;
        let msg = `✅ ${data.name} 完成`;
        if (out) {
          if (data.name === "bash") {
            msg += `\n\`\`\`\n${out}\n\`\`\``;
          } else if (isWriteTool(data.name)) {
            msg += `\n\`\`\`diff\n${out}\n\`\`\``;
          } else if (data.name === "read") {
            msg += `\n${formatReadOutput(out)}`;
          } else if (data.name === "question") {
            msg = `❓ 问题已回答`;
          } else {
            msg += `\n${out}`;
          }
        }
        sendResponse(from, msg);
        break;
      }
      case "tool_failed":
        if (data.name === "question") {
          pendingQuestions.delete(from);
        }
        sendResponse(from, `❌ ${data.name} 失败: ${data.error?.message ?? JSON.stringify(data.error).slice(0, 500)}`);
        break;
      case "model_switched":
        sendResponse(from, `⚠️  ${data.from?.modelId ?? "?"} 配额用完，切换至 ${data.to?.modelId ?? "?"}`);
        break;
      case "question_asked": {
        const { requestID, questions } = data;
        const q = questions?.[0];
        if (!q) break;
        pendingQuestions.set(from, { requestID });
        sendList(from, q.question, q.header, q.options.map((o: any) => ({
          label: o.label,
          value: o.label,
          description: o.description,
        })));
        break;
      }
    }
  };
}

// ---- Model config helpers ----

function formatModelSpec(spec: ModelSpec): string {
  return `${spec.modelId} (${spec.providerId})${spec.variant ? ` variant=${spec.variant}` : ""}`;
}

function formatModelChain(chain: ModelSpec[]): string {
  if (chain.length === 0) return "  (无)";
  return chain.map((s, i) => `  ${i + 1}. ${formatModelSpec(s)}`).join("\n");
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

    case "curr": {
      const lines = [`会话ID: ${us.sessionId || "(无)"}`];
      lines.push(`模式: ${us.planMode ? "plan 只读" : "默认"}`);
      lines.push(`agentId: ${us.agentId || "(无)"}`);
      lines.push(`策略: ${us.modelMode ?? "global"}`);
      if (us.modelId) lines.push(`模型: ${us.modelId} (${us.providerId || "?"})`);
      if (us.modelVariant) lines.push(`variant: ${us.modelVariant}`);
      if (us.mode) lines.push(`mode: ${us.mode}`);
      if (state.projectDir) lines.push(`项目目录: ${state.projectDir}`);
      sendResponse(from, lines.join("\n"));
      break;
    }

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
      if (arg && us.sessionId) {
        const chain = resolveModelChain(us, state.modelConfig);
        const reply = await sendPromptStreaming(
          state.client, us.sessionId, arg, state.projectDir,
          chain, us.agentId,
          makeEventHandler(from),
        );
        sendResponse(from, reply);
        sendResponse(from, "✅ 处理完成");
      }
      break;
    }

    case "model":
      await cmdModel(state, from, us, arg);
      saveStateImpl(state);
      break;

    case "skills": {
      const list = await listSkills(state);
      if (list.length === 0) {
        sendResponse(from, "暂无可用 skill");
        break;
      }
      sendResponse(from, list.map((s) => `• ${s.label}\n  ${s.description}`).join("\n\n"));
      break;
    }

    default:
      sendResponse(from, `未知命令: /${cmd}\n输入 /help 查看可用命令`);
  }
}

async function cmdModel(state: any, from: string, us: any, arg: string) {
  const parts = arg.split(/\s+/);
  const sub = parts[0] || "";
  const subArg = parts.slice(1).join(" ");

  switch (sub) {
    case "": {
      // Show current config
      const lines: string[] = ["📋 模型配置\n"];
      lines.push("【全局】");
      if (state.modelConfig.defaultModel) {
        lines.push(`  默认: ${formatModelSpec(state.modelConfig.defaultModel)}`);
      } else {
        lines.push("  默认: (无，由服务端决定)");
      }
      lines.push("  备用链:");
      lines.push(formatModelChain(state.modelConfig.fallbackChain));

      lines.push("");
      lines.push("【当前会话】");
      const mode = us.modelMode ?? "global";
      if (mode === "manual" && us.modelId && us.providerId) {
        lines.push(`  策略: manual → ${formatModelSpec({ modelId: us.modelId, providerId: us.providerId, variant: us.modelVariant })}`);
      } else {
        lines.push(`  策略: global（继承全局）`);
        const chain = resolveModelChain(us, state.modelConfig);
        lines.push(`  实际使用: ${formatModelSpec(chain[0])}`);
      }
      sendResponse(from, lines.join("\n"));
      break;
    }

    case "list": {
      const list = await listModels(state);
      if (list.length === 0) {
        sendResponse(from, "暂无可用模型");
        break;
      }
      sendList(from, "点选模型后，当前会话将使用该模型：", "可选模型", list);
      // The list response will be handled by handleListResponse with action model-select
      break;
    }

    case "default":
      await cmdModelDefault(state, from, subArg);
      break;

    case "fallback":
      await cmdModelFallback(state, from, subArg);
      break;

    case "override": {
      if (!subArg || subArg === "clear") {
        us.modelMode = "global";
        delete us.modelId;
        delete us.providerId;
        delete us.modelVariant;
        sendResponse(from, "✅ 当前会话已切回全局策略");
      } else {
        sendResponse(from, "用法: /model override clear — 清空当前会话的模型覆盖");
      }
      break;
    }

    default:
      sendResponse(from, "用法:\n/model — 查看配置\n/model list — 列出可用模型\n/model default — 查看/设置全局默认模型\n/model fallback — 查看/设置备用链\n/model override clear — 清空当前会话覆盖");
  }
}

async function cmdModelDefault(state: any, from: string, arg: string) {
  const [action, ...rest] = arg.split(/\s+/);
  switch (action) {
    case "":
      if (state.modelConfig.defaultModel) {
        sendResponse(from, `全局默认模型:\n  ${formatModelSpec(state.modelConfig.defaultModel)}`);
      } else {
        sendResponse(from, "全局默认模型: (无，由服务端决定)");
      }
      break;

    case "set": {
      const list = await listModelsWithAction(state, "default-set");
      if (list.length === 0) {
        sendResponse(from, "暂无可用模型");
        break;
      }
      sendList(from, "点选模型设为全局默认：", "设为默认模型", list);
      break;
    }

    case "clear":
      state.modelConfig.defaultModel = undefined;
      sendResponse(from, "✅ 已清除全局默认模型");
      break;

    default:
      sendResponse(from, "用法: /model default set — 点选设置 | /model default clear — 清除");
  }
}

async function cmdModelFallback(state: any, from: string, arg: string) {
  const [action, ...rest] = arg.split(/\s+/);
  switch (action) {
    case "":
      if (state.modelConfig.fallbackChain.length === 0) {
        sendResponse(from, "备用链: (空)\n可用 /model fallback add 添加");
      } else {
        const lines = ["备用链:", formatModelChain(state.modelConfig.fallbackChain)];
        sendResponse(from, lines.join("\n"));
      }
      break;

    case "add": {
      const list = await listModelsWithAction(state, "fallback-add");
      if (list.length === 0) {
        sendResponse(from, "暂无可用模型");
        break;
      }
      sendList(from, "点选模型追加到备用链末尾：", "追加备用模型", list);
      break;
    }

    case "set": {
      const list = await listModelsWithAction(state, "fallback-set");
      if (list.length === 0) {
        sendResponse(from, "暂无可用模型");
        break;
      }
      sendList(from, "点选模型替换整个备用链：", "替换备用链", list);
      break;
    }

    case "remove": {
      const idxStr = rest.join(" ").trim();
      const idx = parseInt(idxStr, 10);
      if (isNaN(idx) || idx < 1 || idx > state.modelConfig.fallbackChain.length) {
        sendResponse(from, `用法: /model fallback remove <序号>\n当前备用链共 ${state.modelConfig.fallbackChain.length} 项`);
        break;
      }
      const removed = state.modelConfig.fallbackChain.splice(idx - 1, 1)[0];
      sendResponse(from, `✅ 已移除第 ${idx} 项: ${formatModelSpec(removed)}`);
      break;
    }

    case "clear":
      state.modelConfig.fallbackChain = [];
      sendResponse(from, "✅ 已清空备用链");
      break;

    default:
      sendResponse(from, "用法:\n/model fallback — 查看\n/model fallback add — 追加\n/model fallback set — 替换\n/model fallback remove <序号> — 移除\n/model fallback clear — 清空");
  }
}

async function listModelsWithAction(
  state: any,
  action: string,
): Promise<{ label: string; value: string; description: string }[]> {
  const list = await listModels(state);
  return list.map((item: any) => {
    const base = JSON.parse(item.value);
    return {
      label: item.label,
      value: JSON.stringify({ ...base, action }),
      description: item.description,
    };
  });
}

async function cmdSessions(state: any, from: string) {
  const sessions = await fetchAllSessions(state.client, state.baseUrl);
  if (sessions.length === 0) {
    sendResponse(from, "暂无会话，输入 /new [标题] 创建一个");
    return;
  }

  const groups = new Map<string, any[]>();
  for (const s of sessions) {
    const dir = s.directory || "";
    if (!groups.has(dir)) groups.set(dir, []);
    groups.get(dir)!.push(s);
  }

  process.stderr.write(`[opencode-bot] cmdSessions: ${sessions.length} sessions, ${groups.size} dirs: ${JSON.stringify(Array.from(groups.keys()))}\n`);

  if (groups.size === 1) {
    const dir = Array.from(groups.keys())[0];
    const list = sessions.map(formatSessionItem);
    sendList(from, dir ? `请选择会话（${path.basename(dir)}）：` : "请选择会话：", "可选会话", list);
    return;
  }

  const dirList = Array.from(groups.entries()).map(([dir, sList]) => ({
    label: dir ? path.basename(dir) : "(默认)",
    value: `dir:${dir}`,
    description: `📁 ${dir || "(默认)"} — ${sList.length} 个会话`,
  }));
  sendList(from, "请选项目目录：", "项目目录", dirList);
}

async function cmdNew(state: any, from: string, us: any, arg: string) {
  const parts = arg.split(/\s+/);
  const title = parts[0] || `Chat ${Date.now()}`;
  const dir = parts[1] || state.projectDir;
  try {
    const body: any = { title, directory: dir };
    const chain = resolveModelChain(us, state.modelConfig);
    if (chain[0]?.modelId && chain[0]?.providerId) {
      body.model = { id: chain[0].modelId, providerID: chain[0].providerId };
      if (chain[0].variant) body.model.variant = chain[0].variant;
    }
    const result = await state.client.session.create(body);
    us.sessionId = (result.data as any)?.id;
    us.mode = "sdk";
    if (!us.sessionId) {
      sendResponse(from, "创建失败: 无 sessionId");
      return;
    }
    const info = await getSessionInfo(state.client, us.sessionId);
    sendResponse(from, `✅ 已新建会话「${title}」\n${info}`);
  } catch (e: any) {
    sendResponse(from, `创建失败: ${e.message ?? e}`);
  }
}

function formatSessionItem(s: any): { label: string; value: string; description: string } {
  const title = s.title || s.id?.slice(0, 8) || "Untitled";
  const time = s.updatedAt || s.updated || 0;
  const timeStr = time ? new Date(time).toLocaleDateString("zh-CN") : "?";
  return {
    label: title,
    value: s.id,
    description: `🕐 ${timeStr}${s.directory ? ` 📁 ${path.basename(s.directory)}` : ""}`,
  };
}

async function handleListResponse(state: any, from: string, value: string) {
  const us = getUserState(state, from);
  // Directory selection from /sessions flow
  if (value.startsWith("dir:")) {
    const dir = value.slice(4);
    const sessions = await fetchAllSessions(state.client, state.baseUrl);
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
  // Check if value is a JSON config (model/variant/plan/config selection)
  if (value.startsWith("{")) {
    let config: any;
    try { config = JSON.parse(value); } catch { return; }

    const action = config.action;

    // Model config actions: default-set, fallback-add, fallback-set
    if (action === "default-set") {
      state.modelConfig.defaultModel = { modelId: config.model, providerId: config.provider };
      sendResponse(from, `✅ 全局默认模型已设为 ${config.model} (${config.provider})`);
      saveStateImpl(state);
      return;
    }

    if (action === "fallback-add") {
      state.modelConfig.fallbackChain.push({ modelId: config.model, providerId: config.provider });
      sendResponse(from, `✅ 已追加 ${config.model} (${config.provider}) 到备用链末尾`);
      saveStateImpl(state);
      return;
    }

    if (action === "fallback-set") {
      state.modelConfig.fallbackChain = [{ modelId: config.model, providerId: config.provider }];
      sendResponse(from, `✅ 备用链已替换为: ${config.model} (${config.provider})`);
      saveStateImpl(state);
      return;
    }

    // Model/variant/plan selection (no action or action model-select)
    try {
      if (us.sessionId) {
        if (config.model) {
          us.modelId = config.model;
          us.providerId = config.provider;
          us.modelMode = "manual";
        }
        if (config.variant) us.modelVariant = config.variant;
        if (config.agent) us.agentId = config.agent;
        sendResponse(from, "✅ 已设置，下一条消息将使用新配置");
        saveStateImpl(state);
        return;
      }
      if (config.agent && !config.model) {
        sendResponse(
          from,
          "请先用 /sessions 选择或 /new 创建会话后再选 plan",
        );
        return;
      }
      const body: any = {};
      if (config.model) {
        body.model = { id: config.model, providerID: config.provider };
        if (config.variant) body.model.variant = config.variant;
        us.modelMode = "manual";
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
});
