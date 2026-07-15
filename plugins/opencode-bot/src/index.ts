import { parseStdin, sendResponse, sendList, sendLog, setPluginAddr, pluginAddr } from "./yse.js";
import { log, setLogFile } from "./logger.js";
import { initBot, killServer } from "./server.js";
import { diffLines } from "diff";
import {
  listModels, listSkills, listAgents, listVariants,
  fetchAllSessions, getSessionInfo, ensureTmuxSession,
} from "./api.js";
import {
  getUserState,
  userKey,
  sendPromptStreaming,
  sendPrompt,
  resolveModelChain,
  type BotState,
  type SessionState,
  type ModelConfig,
  type ModelSpec,
  type PromptResult,
} from "./opencode.js";
import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import { loadModelConfig, modelConfigPath } from "./model-config.js";


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
/plan <内容>  — 以 plan 模式执行
/model        — 查看当前模型配置
/model list   — 列出可用模型（点选新建会话）
/skills       — 列出可用 skill
/help         — 显示此帮助`;

let stateFile = "";
let pendingUserRestore: any = null;
const pendingQuestions = new Map<string, { requestID: string }>();
const pendingPerms = new Map<string, { requestID: string }>();

async function pendingAnswer(state: BotState, from: string, text: string) {
  const pending = pendingQuestions.get(from);
  if (!pending) {
    sendResponse(from, "当前没有待回答的 AI 问题。");
    return;
  }
  try {
    await state.client.question.reply({
      requestID: pending.requestID,
      answers: [[text]],
    });
    pendingQuestions.delete(from);
    sendResponse(from, `✅ 已提交回答: ${text}`);
  } catch (e: unknown) {
    sendResponse(from, `❌ 提交回答失败: ${e instanceof Error ? e.message : String(e)}`);
  }
}

async function pendingPermit(state: BotState, from: string, action: string) {
  const pending = pendingPerms.get(from);
  if (!pending) return;
  try {
    await (state.client as any).permission.reply({
      requestID: pending.requestID,
      reply: action,
    });
    pendingPerms.delete(from);
    sendResponse(from, `✅ 已确认权限: ${action}`);
  } catch (e: unknown) {
    sendResponse(from, `❌ 确认权限失败: ${e instanceof Error ? e.message : String(e)}`);
  }
}

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
      }),
    );
  } catch (e: unknown) {
    log(`saveState failed: ${e instanceof Error ? e.message : String(e)}`);
  }
}

// ---- 消息队列（后台持续读 stdin，前台 dequeue） ----

let promptAbort: AbortController | null = null;
const msgQueue: any[] = [];

function dequeueMsg(): Promise<any> {
  if (msgQueue.length > 0) return Promise.resolve(msgQueue.shift()!);
  return new Promise(r => {
    const iv = setInterval(() => {
      if (msgQueue.length > 0) {
        clearInterval(iv);
        r(msgQueue.shift()!);
      }
    }, 20);
  });
}

// Background reader — keeps pushing to queue so main thread never waits on stdin
(() => {
  const stdinIt = parseStdin()[Symbol.asyncIterator]();
  (async () => {
    try {
      for await (const msg of stdinIt) {
        msgQueue.push(msg);
        if (!msg.params?.meta?.plugin?.response && promptAbort) (promptAbort as AbortController).abort();
      }
    } catch (e: unknown) {
      log(`reader error: ${e}`);
    }
    msgQueue.push(null); // EOF sentinel
  })();
})();

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

  while (true) {
    const msg = await dequeueMsg();
    if (msg === null) break;
    if (msg.method === "config") {
      const params = msg.params;
      if (params.virtual_addr) {
        setPluginAddr(params.virtual_addr);
      }
      const dir = params.state_dir;
      if (params.virtual_addr && dir) {
        stateFile = path.join(path.dirname(dir), params.virtual_addr, "sessions.json");
        setLogFile(path.join(path.dirname(dir), params.virtual_addr, "bot.log"));
        try {
          const raw = fs.readFileSync(stateFile, "utf-8");
          const saved: {
            sessions?: Record<string, SessionState>;
            projectDir?: string;
            lastUserState?: { sessionId: string; planMode: boolean; agentId?: string; modelId?: string; providerId?: string; modelVariant?: string; modelMode?: string };
          } = JSON.parse(raw);
          if (state && saved.sessions) {
            state.sessions = {};
            for (const [k, v] of Object.entries(saved.sessions)) {
              const nk = userKey(k);
              if (!state.sessions[nk] || !state.sessions[nk].sessionId) {
                state.sessions[nk] = { ...v };
              }
            }
          }
          if (state && saved.projectDir) state.projectDir = saved.projectDir;
          if (state) state.modelConfig = loadModelConfig();
          if (state && saved.lastUserState?.sessionId) {
            pendingUserRestore = saved.lastUserState;
            ensureTmuxSession(saved.lastUserState.sessionId, saved.projectDir);
          }
          sendLog("info", `loaded state from ${stateFile}`);
        } catch (e: unknown) {
          log(`failed to load state: ${e instanceof Error ? e.message : String(e)}`);
        }
      }
      continue;
    }

    if (msg.method !== "message") continue;
    const from = msg.params.from;
    const respValue = msg.params.meta?.plugin?.response?.value;
    if (respValue) {
      if (pendingQuestions.has(from)) {
        await pendingAnswer(state!, from, respValue);
        continue;
      }
      if (pendingPerms.has(from)) {
        await pendingPermit(state!, from, respValue);
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

    const text = msg.params.text?.trim();
    if (!text) continue;

    if (!pluginAddr) setPluginAddr(msg.params.to);

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

      const ctrl = new AbortController();
      promptAbort = ctrl;
      const result = await sendPromptStreaming(
        state.client, userState.sessionId, text, state.projectDir,
        chain, userState.agentId,
        makeEventHandler(from),
        ctrl.signal,
      );

      if (ctrl.signal.aborted) {
        promptAbort = null;
        pendingQuestions.delete(from);
        sendResponse(from, "🔴 已中断");
        continue;
      }
      promptAbort = null;
      pendingQuestions.delete(from);

      if (result.text.trim().length < 20) {
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

  if (type === "directory") {
    if (entries.length > 0) {
      return `📂 ${path}\n${entries.map((e) => `  ${e}`).join("\n")}`;
    }
    return `📂 ${path}\n  (空目录)`;
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

function formatCompactDiff(oldStr: string, newStr: string, ctxLines = 3): string {
  const changes = diffLines(oldStr, newStr);

  const lines: Array<{ prefix: string; text: string; changed: boolean }> = [];
  for (const c of changes) {
    const prefix = c.added ? "+" : c.removed ? "-" : " ";
    const changed = !!(c.added || c.removed);
    const ls = c.value.replace(/\n$/, "").split("\n");
    for (const l of ls) {
      lines.push({ prefix, text: l, changed });
    }
  }

  if (!lines.some((l) => l.changed)) return "";

  const include = new Set<number>();
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].changed) {
      for (let j = Math.max(0, i - ctxLines); j <= Math.min(lines.length - 1, i + ctxLines); j++) {
        include.add(j);
      }
    }
  }

  const blocks: string[][] = [];
  let current: string[] = [];
  let lastIncluded = -1;

  for (let i = 0; i < lines.length; i++) {
    if (!include.has(i)) continue;
    if (lastIncluded >= 0 && i - lastIncluded > ctxLines) {
      if (current.length > 0) { blocks.push(current); current = []; }
    }
    current.push(`${lines[i].prefix} ${lines[i].text}`);
    lastIncluded = i;
  }
  if (current.length > 0) blocks.push(current);

  if (blocks.length === 0) return "";
  return blocks.map((b) => b.join("\n")).join("\n\n...\n\n");
}

function makeEventHandler(from: string) {
  return (type: string, data: any) => {
    switch (type) {
      case "tool_called": {
        if (data.name === "bash") {
          const cmd = data.input?.command || "";
          if (!cmd.trim()) break;
          if (cmd.length > 200) {
            sendResponse(from, `🔧 bash\n:::details 命令 (${cmd.length} 字符)\n\`\`\`bash\n${cmd}\n\`\`\`\n:::`);
          } else {
            sendResponse(from, `🔧 bash\n\`\`\`bash\n${cmd}\n\`\`\``);
          }
        } else if (isWriteTool(data.name)) {
          const path = data.input?.filePath || "";
          if (data.name === "edit") {
            const oldS = data.input?.oldString || "";
            const newS = data.input?.newString || "";
            const diffText = formatCompactDiff(oldS, newS);
            sendResponse(from, `🔧 edit${path ? ` (${path})` : ""}\n\`\`\`diff\n${diffText}\n\`\`\``);
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
        const raw = (data.output || "");
        if (data.name === "bash" && !raw) break;
        let msg = `✅ ${data.name} 完成`;
        if (raw) {
          if (data.name === "bash") {
            if (raw.length > 300) {
              msg += `\n:::details 输出 (${raw.length} 字符)\n\n\`\`\`\n${raw}\n\`\`\`\n\n:::`;
            } else {
              msg += `\n\`\`\`\n${raw}\n\`\`\``;
            }
          } else if (isWriteTool(data.name)) {
            msg += `\n\`\`\`diff\n${raw.slice(0, 2000)}\n\`\`\``;
          } else if (data.name === "read") {
            const content = formatReadOutput(raw);
            if (raw.length > 300) {
              msg += `\n:::details 文件内容 (${raw.length} 字符)\n\n${content}\n\n:::`;
            } else {
              msg += `\n${content}`;
            }
          } else if (data.name === "todowrite") {
            msg = `✅ 任务列表已更新`;
          } else if (data.name === "question") {
            msg = `❓ 问题已回答`;
          } else {
            msg += `\n${raw.slice(0, 2000)}`;
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
        if (!q) {
          sendResponse(from, `❓ AI 请求权限（ID: ${requestID}），但内容为空，请检查 OpenCode 界面。`);
          break;
        }
        pendingQuestions.set(from, { requestID });
        try {
          sendList(from, q.question, q.header, (q.options || []).map((o: any) => ({
            label: o.label,
            value: o.label,
            description: o.description,
          })));
          sendResponse(from, `❓ ${q.question}\n请在上面选择或输入 /answer <你的回答>`);
        } catch (e: unknown) {
          sendResponse(from, `❓ ${q.question}\n（无法渲染选择列表，请到 OpenCode 界面操作，或输入 /answer <你的回答> 回复）`);
        }
        break;
      }
      case "permission_asked": {
        const { requestID, permission, patterns } = data;
        const pats: string[] = typeof patterns === "string"
          ? (() => { try { return JSON.parse(patterns); } catch { return [patterns]; } })()
          : (Array.isArray(patterns) ? patterns : []);
        pendingPerms.set(from, { requestID });
        sendList(from, `🔐 ${permission}\n${pats.map((p: string) => `  ${p}`).join("\n")}`,
          "权限确认", [
            { label: "✅ 允许一次", value: "once" },
            { label: "🔁 总是允许", value: "always" },
            { label: "❌ 拒绝", value: "reject" },
          ]);
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
      const sessionRes = await state.client.session.get({ sessionID: arg });
      if (sessionRes.data?.directory) {
        state.projectDir = sessionRes.data.directory;
      }
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
      lines.push(`模型策略: ${userState.modelMode ?? "global"}`);
      if (userState.modelId) lines.push(`模型: ${userState.modelId} (${userState.providerId || "?"})`);
      if (userState.modelVariant) lines.push(`variant: ${userState.modelVariant}`);
      if (userState.mode) lines.push(`mode: ${userState.mode}`);
      if (state.projectDir) lines.push(`项目目录: ${state.projectDir}`);
      if (userState.sessionId) ensureTmuxSession(userState.sessionId, state.projectDir);
      try {
        const sid = (userState.sessionId || "default").replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
        const sock = `/tmp/yse-tmux/yse-${sid}.sock`;
        const out = execSync(`tmux -S ${sock} list-windows -F "#{window_id}: #W" 2>/dev/null || true`, { encoding: "utf-8", timeout: 2000 }).toString().trim();
        if (out) {
          const wins = out.split("\n").filter(l => l && !l.startsWith("0:"));
          if (wins.length > 0) lines.push(`tmux: ${wins.join(", ")}`);
        }
        lines.push(`kitty: kitty tmux -S ${sock} attach`);
      } catch {}
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
      const ctrl = new AbortController();
      promptAbort = ctrl;
      const result = await sendPromptStreaming(
        state.client, userState.sessionId, arg, state.projectDir,
        chain, agentId,
        makeEventHandler(from),
        ctrl.signal,
      );
      if (ctrl.signal.aborted) {
        promptAbort = null;
        sendResponse(from, "🔴 已中断");
        break;
      }
      promptAbort = null;
      if (result.text && result.text !== "(empty response)") {
        sendResponse(from, result.text);
      }
      sendResponse(from, "✅ 处理完成");
      break;
    }

    case "model":
      await cmdModel(state, from, userState, arg);
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

    case "answer":
      if (!arg) {
        sendResponse(from, "用法: /answer <你的回答>");
        break;
      }
      if (!state) {
        sendResponse(from, "OpenCode 未连接");
        break;
      }
      pendingAnswer(state, from, arg);
      break;

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
      state.modelConfig = loadModelConfig();
      lines.push("【全局（TOML）】");
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
      lines.push(`\n编辑 ${modelConfigPath()} 修改全局配置`);
      sendResponse(from, lines.join("\n"));
      break;
    }

    case "list": {
      const list = await listModels(state);
      if (list.length === 0) {
        sendResponse(from, "暂无可用模型");
        break;
      }
      sendList(from, "点选模型后新建会话：", "可选模型", list);
      break;
    }

    default:
      sendResponse(from, "用法:\n/model — 查看配置\n/model list — 列出可用模型");
  }
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

  log(`cmdSessions: ${sessions.length} sessions, ${groups.size} dirs: ${JSON.stringify(Array.from(groups.keys()))}`);

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

async function handleJsonConfig(state: BotState, from: string, value: string) {
  const userState = getUserState(state, from);
  let config: JsonListValue;
  try { config = JSON.parse(value); } catch { return; }

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
  const sessionRes = await state.client.session.get({ sessionID: value });
  if (sessionRes.data?.directory) {
    state.projectDir = sessionRes.data.directory;
  }
  const info = await getSessionInfo(state.client, value);
  sendResponse(from, `✅ 已切换\n${info}`);
}

main().catch((e) => {
  log(`fatal: ${e}`);
  process.exit(1);
});

process.on("exit", () => {
});
