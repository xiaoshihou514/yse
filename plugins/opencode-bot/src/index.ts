import { parseStdin, sendResponse, sendList, sendLog, setPluginAddr, pluginAddr } from "./yse.js";
import { initBot, killServer } from "./server.js";
import {
  listModels, listSkills, listAgents, listVariants,
  fetchAllSessions, getSessionInfo,
} from "./api.js";
import {
  getUserState,
  sendPromptStreaming,
  sendPrompt,
  resolveModelChain,
  type BotState,
  type SessionState,
  type ModelConfig,
  type ModelSpec,
  type PromptResult,
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

function saveStateImpl(state: BotState) {
  if (!stateFile) return;
  try {
    fs.mkdirSync(path.dirname(stateFile), { recursive: true });
    let lastUserState: {
      sessionId: string; planMode: boolean; agentId?: string;
      modelId?: string; providerId?: string; modelVariant?: string; modelMode?: string;
    } | null = null;
    for (const s of Object.values(state.sessions)) {
      if (s.sessionId) {
        lastUserState = {
          sessionId: s.sessionId,
          planMode: s.planMode ?? false,
          agentId: s.agentId,
          modelId: s.modelId,
          providerId: s.providerId,
          modelVariant: s.modelVariant,
          modelMode: s.modelMode ?? "global",
        };
        break;
      }
    }
    fs.writeFileSync(
      stateFile,
      JSON.stringify({
        sessions: state.sessions,
        projectDir: state.projectDir,
        lastUserState,
        modelConfig: state.modelConfig,
      }),
    );
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] saveState failed: ${e instanceof Error ? e.message : String(e)}\n`);
  }
}

async function main() {
  sendLog("info", "opencode-bot starting...");
  let state: BotState | null = null;
  try {
    state = await initBot();
  } catch (e: unknown) {
    sendLog("error", `failed to connect to OpenCode: ${e instanceof Error ? e.message : String(e)}`);
    sendLog(
      "info",
      "opencode-bot running in degraded mode — OpenCode unavailable",
    );
  }

  for await (const msg of parseStdin()) {
    if (msg.method === "config") {
      const params = msg.params;
      if (params.virtual_addr) {
        setPluginAddr(params.virtual_addr);
      }
      const dir = params.state_dir;
      if (dir) {
        stateFile = path.join(dir, "sessions.json");
        try {
          const raw = fs.readFileSync(stateFile, "utf-8");
          const saved: {
            sessions?: BotState["sessions"];
            projectDir?: string;
            modelConfig?: ModelConfig;
            lastUserState?: { sessionId: string; planMode: boolean; agentId?: string; modelId?: string; providerId?: string; modelVariant?: string; modelMode?: string };
          } = JSON.parse(raw);
          if (state && saved.sessions) state.sessions = saved.sessions;
          if (state && saved.projectDir) state.projectDir = saved.projectDir;
          if (state && saved.modelConfig) state.modelConfig = saved.modelConfig;
          if (state && saved.lastUserState?.sessionId) {
            pendingUserRestore = saved.lastUserState;
          }
          sendLog("info", `loaded state from ${stateFile}`);
        } catch (e: unknown) {
          process.stderr.write(`[opencode-bot] failed to load state: ${e instanceof Error ? e.message : String(e)}\n`);
        }
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
        } catch (e: unknown) {
          sendResponse(from, `❌ 提交回答失败: ${e instanceof Error ? e.message : String(e)}`);
        }
        continue;
      }
      if (!state) {
        sendResponse(from, "OpenCode 未连接，无法处理列表选择。");
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

    const userState = getUserState(state, from);

    if (pendingUserRestore && !userState.sessionId) {
      userState.sessionId = pendingUserRestore.sessionId;
      userState.planMode = pendingUserRestore.planMode ?? false;
      userState.agentId = pendingUserRestore.agentId;
      userState.modelId = pendingUserRestore.modelId;
      userState.providerId = pendingUserRestore.providerId;
      userState.modelVariant = pendingUserRestore.modelVariant;
      userState.modelMode = pendingUserRestore.modelMode ?? "global";
      pendingUserRestore = null;
      saveStateImpl(state);
    }

    // Command routing
    if (text.startsWith("/")) {
      const [cmd, ...args] = text.slice(1).split(/\s+/);
      await handleCommand(state, from, userState, cmd, args.join(" "));
    } else if (userState.sessionId) {
      const chain = resolveModelChain(userState, state.modelConfig);
      pendingQuestions.delete(from);
      const result = await sendPromptStreaming(
        state.client, userState.sessionId, text, state.projectDir,
        chain, userState.agentId,
        makeEventHandler(from),
      );
      pendingQuestions.delete(from);

      if (!result.text.trim() || result.text === "(empty response)") {
        const summary = await sendPrompt(
          state.client, userState.sessionId,
          "你上一轮任务做了什么？用一两句话简单总结一下。",
          state.projectDir, chain, userState.agentId,
        );
        if (summary && summary !== "(empty response)") {
          sendResponse(from, summary);
        }
      } else {
        sendResponse(from, result.text);
      }

      if (result.tokens) {
        const i = formatTokens(result.tokens.input);
        const o = formatTokens(result.tokens.output);
        sendResponse(from, `词元：输入${i}，输出${o}`);
      }
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
  return name === "write" || name === "edit";
}

function formatTokens(n: number): string {
  if (n >= 10000) return `${(n / 10000).toFixed(2)}万`;
  return String(n);
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
  const contentM = out.match(/<content>\n?([\s\S]*?)\n?<\/content>/);
  const entriesRaw = entriesM?.[1] || contentM?.[1] || "";
  const entries = entriesRaw
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("(") && !l.startsWith("<"));

  if (type === "directory" && entries.length > 0) {
    return `📂 ${path}\n${entries.map((e) => `  ${e}`).join("\n")}`;
  }
  if ((type === "file" || (!type && contentM)) && entriesRaw) {
    const lang = extLang(path);
    const content = entriesRaw.slice(0, 1000);
    const truncated = entriesRaw.length > 1000 ? "\n... (truncated)" : "";
    return `📄 ${path}\n\`\`\`${lang}\n${content}\n\`\`\`${truncated}`;
  }
  return out;
}

const STATUS_ICONS: Record<string, string> = {
  completed: "x",
  in_progress: "",
  pending: "",
};

const STATUS_LABELS: Record<string, string> = {
  completed: "",
  in_progress: " *(进行中)*",
  pending: "",
};

function formatTodoOutput(input: any): string {
  const todos: any[] = input?.todos ?? [];
  if (!Array.isArray(todos) || todos.length === 0) return "";

  const lines: string[] = [];
  for (const t of todos) {
    const text = t.content || t.task || t.title || String(t);
    const status: string = t.status || "pending";
    const icon = STATUS_ICONS[status] ?? " ";
    const label = STATUS_LABELS[status] ?? "";
    lines.push(`- [${icon}] ${text}${label}`);
  }

  const completed = todos.filter((t: any) =>
    t.status === "completed" || t.status === "done"
  ).length;
  const total = todos.length;
  const pct = total > 0 ? Math.round((completed / total) * 100) : 0;
  lines.push(`\n────────── ${completed}/${total} (${pct}%) ──────────`);

  return lines.join("\n");
}

function makeEventHandler(from: string) {
  return (type: string, data: any) => {
    switch (type) {
      case "tool_called": {
        if (data.name === "bash") {
          const cmd = data.input?.command || "";
          if (!cmd.trim()) break;
          sendResponse(from, `🔧 bash\n\`\`\`bash\n${cmd.slice(0, 2000)}\n\`\`\``);
        } else if (isWriteTool(data.name)) {
          const path = data.input?.filePath || "";
          if (data.name === "edit") {
            const oldS = (data.input?.oldString || "").slice(0, 1000);
            const newS = (data.input?.newString || "").slice(0, 1000);
            sendResponse(from, `🔧 edit${path ? ` (${path})` : ""}\n\`\`\`diff\n${oldS}\n\`\`\``);
          } else {
            const content = data.input?.content || "";
            sendResponse(from, `🔧 write${path ? ` (${path})` : ""}\n\`\`\`diff\n+ ${content.slice(0, 2000)}\n\`\`\``);
          }
        } else if (data.name === "read") {
          const path = data.input?.filePath || data.input?.filePattern || formatToolInput(data.input);
          sendResponse(from, `📂 read: ${path.slice(0, 200)}`);
        } else if (data.name === "todowrite") {
          const count = Array.isArray(data.input?.todos) ? data.input.todos.length : 0;
          if (count > 0) {
            sendResponse(from, `📋 任务列表\n${formatTodoOutput(data.input)}`);
          } else {
            sendResponse(from, `📋 更新任务列表`);
          }
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
          } else if (data.name === "todowrite") {
            // brief confirmation; full list already shown at tool_called
            msg = `✅ 任务列表已更新`;
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
  state: BotState,
  from: string,
  userState: SessionState,
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
      userState.sessionId = arg;
      userState.mode = "sdk";
      const info = await getSessionInfo(state.client, arg);
      sendResponse(from, `✅ 已切换\n${info}`);
      saveStateImpl(state);
      break;

    case "new":
      await cmdNew(state, from, userState, arg);
      saveStateImpl(state);
      break;

    case "curr": {
      const lines = [`会话ID: ${userState.sessionId || "(无)"}`];
      lines.push(`模式: ${userState.planMode ? "plan 只读" : "默认"}`);
      lines.push(`agentId: ${userState.agentId || "(无)"}`);
      lines.push(`策略: ${userState.modelMode ?? "global"}`);
      if (userState.modelId) lines.push(`模型: ${userState.modelId} (${userState.providerId || "?"})`);
      if (userState.modelVariant) lines.push(`variant: ${userState.modelVariant}`);
      if (userState.mode) lines.push(`mode: ${userState.mode}`);
      if (state.projectDir) lines.push(`项目目录: ${state.projectDir}`);
      sendResponse(from, lines.join("\n"));
      break;
    }

    case "abort":
      if (!userState.sessionId) {
        sendResponse(from, "未选择会话");
        break;
      }
      try {
        await state.client.session.abort({ sessionID: userState.sessionId });
        sendResponse(from, "✅ 已中止");
      } catch (e: unknown) {
        sendResponse(from, `中止失败: ${e instanceof Error ? e.message : String(e)}`);
      }
      break;

    case "undo":
      if (!userState.sessionId) {
        sendResponse(from, "未选择会话");
        break;
      }
      try {
        await state.client.session.revert({ sessionID: userState.sessionId });
        sendResponse(from, "✅ 已撤回上一条消息");
      } catch (e: unknown) {
        sendResponse(from, `撤回失败: ${e instanceof Error ? e.message : String(e)}`);
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
      if (!userState.sessionId) {
        sendResponse(from, "请先选择会话");
        break;
      }
      const vlist = await listVariants(state.baseUrl);
      if (vlist.length === 0) {
        sendResponse(from, "暂无可用 variant");
        break;
      }
      sendList(from, "请选择 variant（将用该 model+variant 新建会话）：", "可用 Variant", vlist);
      break;
    }

    case "plan": {
      if (!arg) {
        sendResponse(from, "用法: /plan <你想规划的内容>");
        break;
      }
      if (!userState.sessionId) {
        sendResponse(from, "请先选择会话：/sessions 或 /new [标题]");
        break;
      }
      const list = await listAgents(state);
      if (list.length === 0) {
        sendResponse(from, "暂无可用 plan");
        break;
      }
      const agentId = JSON.parse(list[0].value).agent;
      const chain = resolveModelChain(userState, state.modelConfig);
      const result = await sendPromptStreaming(
        state.client, userState.sessionId, arg, state.projectDir,
        chain, agentId,
        makeEventHandler(from),
      );
      if (result.text && result.text !== "(empty response)") {
        sendResponse(from, result.text);
      }
      sendResponse(from, "✅ 处理完成");
      break;
    }

    case "model":
      await cmdModel(state, from, userState, arg);
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

async function cmdModel(state: BotState, from: string, userState: SessionState, arg: string) {
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
      const mode = userState.modelMode ?? "global";
      if (mode === "manual" && userState.modelId && userState.providerId) {
        lines.push(`  策略: manual → ${formatModelSpec({ modelId: userState.modelId, providerId: userState.providerId, variant: userState.modelVariant })}`);
      } else {
        lines.push(`  策略: global（继承全局）`);
        const chain = resolveModelChain(userState, state.modelConfig);
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
        userState.modelMode = "global";
        delete userState.modelId;
        delete userState.providerId;
        delete userState.modelVariant;
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

async function cmdModelDefault(state: BotState, from: string, arg: string) {
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

async function cmdModelFallback(state: BotState, from: string, arg: string) {
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
  state: BotState,
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

async function cmdSessions(state: BotState, from: string) {
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

async function cmdNew(state: BotState, from: string, userState: SessionState, arg: string) {
  const parts = arg.split(/\s+/);
  const title = parts[0] || `Chat ${Date.now()}`;
  const dir = parts[1] || state.projectDir;
  try {
    const body: any = { title, directory: dir };
    const chain = resolveModelChain(userState, state.modelConfig);
    if (chain[0]?.modelId && chain[0]?.providerId) {
      body.model = { id: chain[0].modelId, providerID: chain[0].providerId };
      if (chain[0].variant) body.model.variant = chain[0].variant;
    }
    const result: { data?: { id?: string } } = await state.client.session.create(body);
    userState.sessionId = result.data?.id ?? null;
    userState.mode = "sdk";
    if (!userState.sessionId) {
      sendResponse(from, "创建失败: 无 sessionId");
      return;
    }
    const info = await getSessionInfo(state.client, userState.sessionId);
    sendResponse(from, `✅ 已新建会话「${title}」\n${info}`);
  } catch (e: unknown) {
    sendResponse(from, `创建失败: ${e instanceof Error ? e.message : String(e)}`);
  }
}

function formatSessionItem(s: { id?: string; title?: string; updatedAt?: number; updated?: number; directory?: string }): { label: string; value: string; description: string } {
  const title = s.title || s.id?.slice(0, 8) || "Untitled";
  const time = s.updatedAt || s.updated || 0;
  const timeStr = time ? new Date(time).toLocaleDateString("zh-CN") : "?";
  return {
    label: title,
    value: s.id ?? "",
    description: `🕐 ${timeStr}${s.directory ? ` 📁 ${path.basename(s.directory)}` : ""}`,
  };
}

interface JsonListValue {
  action?: string; model?: string; provider?: string;
  variant?: string; agent?: string;
}

async function handleDirSelection(state: BotState, from: string, value: string) {
  const dir = value.slice(4);
  const sessions = await fetchAllSessions(state.client, state.baseUrl);
  const filtered = sessions.filter(
    (s) => s.directory === dir,
  );
  if (filtered.length === 0) {
    sendResponse(from, "该目录下暂无会话");
    return;
  }
  sendList(from, "请选择会话：", "可选会话", filtered.map(formatSessionItem));
}

async function handleModelConfigAction(state: BotState, from: string, config: JsonListValue): Promise<boolean> {
  const { action, model, provider } = config;
  if (action === "default-set" && model && provider) {
    state.modelConfig.defaultModel = { modelId: model, providerId: provider };
    sendResponse(from, `✅ 全局默认模型已设为 ${model} (${provider})`);
    saveStateImpl(state);
    return true;
  }
  if (action === "fallback-add" && model && provider) {
    state.modelConfig.fallbackChain.push({ modelId: model, providerId: provider });
    sendResponse(from, `✅ 已追加 ${model} (${provider}) 到备用链末尾`);
    saveStateImpl(state);
    return true;
  }
  if (action === "fallback-set" && model && provider) {
    state.modelConfig.fallbackChain = [{ modelId: model, providerId: provider }];
    sendResponse(from, `✅ 备用链已替换为: ${model} (${provider})`);
    saveStateImpl(state);
    return true;
  }
  return false;
}

async function handleJsonConfig(state: BotState, from: string, value: string) {
  const userState = getUserState(state, from);
  let config: JsonListValue;
  try { config = JSON.parse(value); } catch { return; }

  if (await handleModelConfigAction(state, from, config)) return;

  if (userState.sessionId) {
    if (config.model) {
      userState.modelId = config.model;
      userState.providerId = config.provider;
      userState.modelMode = "manual";
    }
    if (config.variant) userState.modelVariant = config.variant;
    if (config.agent) userState.agentId = config.agent;
    sendResponse(from, "✅ 已设置，下一条消息将使用新配置");
    saveStateImpl(state);
    return;
  }

  if (config.agent && !config.model) {
    sendResponse(from, "请先用 /sessions 选择或 /new 创建会话后再选 plan");
    return;
  }

  try {
    const body: any = {};
    if (config.model) {
      body.model = { id: config.model, providerID: config.provider };
      if (config.variant) body.model.variant = config.variant;
      userState.modelMode = "manual";
    }
    if (config.agent) body.agent = config.agent;
    if (!body.model && !body.agent) body.agent = config;

    const title = config.model
      ? `${config.model}${config.variant ? ` (${config.variant})` : ""}`
      : config.agent ?? "new";
    const result: { data?: { id?: string } } =
      await state.client.session.create({ title, directory: state.projectDir, ...body } as Record<string, unknown>);
    const newId = result.data?.id ?? null;
    if (!newId) { sendResponse(from, "创建失败"); return; }
    userState.sessionId = newId;
    userState.mode = "sdk";
    const info = await getSessionInfo(state.client, newId);
    sendResponse(from, `✅ 已新建会话「${title}」\n${info}`);
    saveStateImpl(state);
  } catch (e: unknown) {
    sendResponse(from, `创建失败: ${e instanceof Error ? e.message : String(e)}`);
  }
}

async function handleListResponse(state: BotState, from: string, value: string) {
  if (value.startsWith("dir:")) {
    await handleDirSelection(state, from, value);
    return;
  }
  if (value.startsWith("{")) {
    await handleJsonConfig(state, from, value);
    return;
  }
  const userState = getUserState(state, from);
  userState.sessionId = value;
  userState.mode = "sdk";
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
