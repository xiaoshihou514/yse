import { createOpencodeClient } from "@opencode-ai/sdk/v2";
import { spawn, type ChildProcess } from "child_process";

type OpenCodeClient = ReturnType<typeof createOpencodeClient>;

let _serverProcess: ChildProcess | null = null;

process.on("exit", () => {
  if (_serverProcess) {
    _serverProcess.kill();
  }
});

// ---- Model configuration types ----

export interface ModelSpec {
  modelId: string;
  providerId: string;
  variant?: string;
}

export interface ModelConfig {
  defaultModel?: ModelSpec;
  fallbackChain: ModelSpec[];
}

export interface SessionState {
  mode: "sdk" | "tui";
  sessionId: string | null;
  planMode?: boolean;
  modelId?: string;
  providerId?: string;
  modelVariant?: string;
  agentId?: string;
  modelMode?: "global" | "manual";
}

export interface BotState {
  client: ReturnType<typeof createOpencodeClient>;
  projectDir: string;
  baseUrl: string;
  sessions: {
    [userAddr: string]: SessionState;
  };
  modelConfig: ModelConfig;
  serverProcess: ChildProcess | null;
}

interface SessionShape {
  id: string; title: string; directory: string; updatedAt: number;
}

function formatSession(raw: { id?: string; title?: string; directory?: string; updatedAt?: number }): SessionShape {
  return {
    id: raw.id ?? "",
    title: raw.title ?? "",
    directory: raw.directory ?? "",
    updatedAt: raw.updatedAt ?? 0,
  };
}

interface ApiModel {
  id?: string;
  providerID?: string;
  variant?: string;
}

interface ApiSkill {
  id?: string;
  name?: string;
  description?: string;
}

interface ApiAgent {
  id?: string;
  name?: string;
  description?: string;
}

interface PromptPart {
  type: string;
  text?: string;
}

// ---- Pure model resolution logic (testable without server) ----

export function resolveModelChain(
  session: { modelMode?: string; modelId?: string; providerId?: string; modelVariant?: string },
  globalConfig: ModelConfig,
): ModelSpec[] {
  if (session.modelMode === "manual" && session.modelId && session.providerId) {
    return [{ modelId: session.modelId, providerId: session.providerId, variant: session.modelVariant }];
  }
  const chain: ModelSpec[] = [];
  if (globalConfig.defaultModel) {
    chain.push(globalConfig.defaultModel);
  }
  chain.push(...globalConfig.fallbackChain);
  if (chain.length === 0) {
    return [{ modelId: "", providerId: "" }];
  }
  return chain;
}

export function isQuotaError(e: unknown): boolean {
  if (!e || typeof e !== "object" || !("message" in e)) return false;
  const msg = String((e as { message: unknown }).message).toLowerCase();
  return ["quota", "rate", "limit", "exhausted", "insufficient", "429"].some(k => msg.includes(k));
}

export async function tryModelsWithFallback(
  chain: ModelSpec[],
  attemptFn: (spec: ModelSpec, index: number) => Promise<string>,
  onSwitch?: (from: ModelSpec, to: ModelSpec) => void,
): Promise<string> {
  const attempts = chain.length > 0 ? chain : [{ modelId: "", providerId: "" }];
  let lastError: unknown = null;
  for (let i = 0; i < attempts.length; i++) {
    try {
      return await attemptFn(attempts[i], i);
    } catch (e: unknown) {
      if (isQuotaError(e)) {
        lastError = e;
        if (i + 1 < attempts.length) {
          onSwitch?.(attempts[i], attempts[i + 1]);
        }
        continue;
      }
      throw e;
    }
  }
  throw lastError || new Error("所有模型均不可用");
}

// ---- Server management ----

function startServer(): { child: ChildProcess; port: Promise<number> } {
  const child = spawn("opencode", ["serve", "--port", "0", "--print-logs"], {
    stdio: ["ignore", "pipe", "inherit"],
  });
  let stdout = "";
  const port = new Promise<number>((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error("opencode server start timeout (15s)"));
    }, 15000);
    child.stdout!.on("data", (data: Buffer) => {
      stdout += data.toString();
      const m = stdout.match(/listening on http:\/\/127\.0\.0\.1:(\d+)/);
      if (m) {
        clearTimeout(timeout);
        resolve(parseInt(m[1], 10));
      }
    });
    child.on("error", (e) => {
      clearTimeout(timeout);
      reject(e);
    });
    child.on("exit", (code) => {
      if (code !== null && code !== 0) {
        clearTimeout(timeout);
        reject(new Error(`opencode server exited with code ${code}`));
      }
    });
  });
  return { child, port };
}

export async function initBot(): Promise<BotState | null> {
  try {
    const { child, port } = startServer();
    _serverProcess = child;
    const actualPort = await port;
    process.stderr.write(`[opencode-bot] server started on port ${actualPort}\n`);

    const cwd = process.cwd();
    const baseUrl = `http://127.0.0.1:${actualPort}`;
    let projectDir = cwd;

    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        const probe = createOpencodeClient({ baseUrl, directory: projectDir });
        const proj: { data?: { worktree?: string } } = await probe.project.current();
        if (proj.data?.worktree) projectDir = proj.data.worktree;
        break;
      } catch (e: unknown) {
        process.stderr.write(`[opencode-bot] project.current attempt ${attempt} failed: ${e instanceof Error ? e.message : String(e)}\n`);
        if (attempt < 2) {
          await new Promise((r) => setTimeout(r, 1500));
        }
      }
    }

    const client = createOpencodeClient({
      baseUrl,
      directory: projectDir,
    });

    return {
      client, projectDir, baseUrl,
      sessions: {},
      modelConfig: { defaultModel: undefined, fallbackChain: [] },
      serverProcess: child,
    };
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] initBot failed: ${e instanceof Error ? e.message : String(e)}\n`);
    return null;
  }
}

export function killServer(state: BotState) {
  if (state.serverProcess) {
    state.serverProcess.kill();
  }
}

export function getUserState(
  state: BotState,
  userAddr: string,
): SessionState {
  if (!state.sessions[userAddr]) {
    state.sessions[userAddr] = { mode: "sdk", sessionId: null, modelMode: "global" };
  }
  return state.sessions[userAddr];
}

export function getSessionState(
  state: BotState | null,
  userAddr: string,
): SessionState | null {
  if (!state) return null;
  return getUserState(state, userAddr);
}

// ---- Prompt helpers ----

function extractTextParts(msg: { parts?: PromptPart[] }): string {
  const texts = (msg?.parts ?? [])
    .filter((p) => p.type === "text")
    .map((p) => p.text)
    .filter((t): t is string => !!t);
  return texts.join("\n") || "(empty response)";
}

function buildPromptParams(
  sessionId: string,
  text: string,
  directory: string | undefined,
  spec: ModelSpec,
  agentId: string | undefined,
): any {
  const params: any = {
    sessionID: sessionId,
    parts: [{ type: "text", text }],
    ...(directory ? { directory } : {}),
  };
  if (spec.modelId && spec.providerId) {
    params.model = {
      id: spec.modelId,
      providerID: spec.providerId,
      ...(spec.variant ? { variant: spec.variant } : {}),
    };
  } else if (agentId) {
    params.agent = agentId;
  }
  return params;
}

export async function sendPromptStreaming(
  client: OpenCodeClient,
  sessionId: string,
  text: string,
  directory: string | undefined,
  chain: ModelSpec[],
  agentId: string | undefined,
  onEvent: (type: string, data: any) => void,
): Promise<string> {
  let ourMessageId: string | null = null;
  const abortController = new AbortController();
  let eventStream: any = null;
  let eventConsumer: Promise<void> | null = null;

  try {
    const sub = await client.global.event({ signal: abortController.signal });
    eventStream = sub.stream;

    eventConsumer = (async () => {
      try {
        for await (const raw of eventStream) {
          const ev = raw?.payload || raw;
          if (!ev?.type) continue;
          const p = ev.properties;
          if (!p?.sessionID || p.sessionID !== sessionId) continue;

          if (ev.type === "question.v2.asked") {
            onEvent("question_asked", {
              requestID: ev.id,
              sessionID: p.sessionID,
              questions: p.questions,
            });
            continue;
          }

          if (!ourMessageId && ev.type === "message.updated") {
            const info = p.info || p;
            if (info?.role === "assistant" && info?.id) {
              ourMessageId = info.id;
            }
          }
          if (!ourMessageId) continue;

          const msgId = p.info?.part?.messageID || p.messageID || p.info?.messageID;
          if (msgId && msgId !== ourMessageId) continue;

          if (ev.type === "message.part.updated") {
            const part = p.info?.part || p.part;
            if (!part || part.type !== "tool") continue;
            const s = part.state;
            if (!s) continue;
            if (s.status === "running" || s.status === "pending") {
              onEvent("tool_called", { name: part.tool, input: s.input });
            } else if (s.status === "completed") {
              onEvent("tool_success", { name: part.tool, output: s.output || "", result: s.metadata });
            }
          }
        }
      } catch (e: unknown) {
        process.stderr.write(`[opencode-bot] SSE consumer error: ${e instanceof Error ? e.message : String(e)}\n`);
      }
    })();
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] SSE subscribe failed: ${e instanceof Error ? e.message : String(e)}\n`);
  }

  try {
    return await tryModelsWithFallback(
      chain,
      async (spec) => {
        ourMessageId = null;
        const params = buildPromptParams(sessionId, text, directory, spec, agentId);
        const result = await client.session.prompt(params);
        return extractTextParts(result.data as { parts?: PromptPart[] });
      },
      (from, to) => {
        onEvent("model_switched", {
          from: { modelId: from.modelId, providerId: from.providerId },
          to: { modelId: to.modelId, providerId: to.providerId },
        });
      },
    );
  } catch (e: unknown) {
    return `Error: ${e instanceof Error ? e.message : String(e)}`;
  } finally {
    abortController.abort();
    if (eventConsumer) {
      try { await eventConsumer; } catch (e: unknown) {
        process.stderr.write(`[opencode-bot] eventConsumer await failed: ${e instanceof Error ? e.message : String(e)}\n`);
      }
    }
  }
}

export async function sendPrompt(
  client: OpenCodeClient,
  sessionId: string,
  text: string,
  directory?: string,
  chain?: ModelSpec[],
  agentId?: string,
): Promise<string> {
  try {
    return await tryModelsWithFallback(
      chain ?? [{ modelId: "", providerId: "" }],
      async (spec) => {
        const params = buildPromptParams(sessionId, text, directory, spec, agentId);
        const result = await client.session.prompt(params);
        return extractTextParts(result.data as { parts?: PromptPart[] });
      },
    );
  } catch (e: unknown) {
    return `Error: ${e instanceof Error ? e.message : String(e)}`;
  }
}

// ---- Listing helpers ----

export async function listModels(
  state: BotState,
): Promise<{ label: string; value: string; description: string }[]> {
  try {
    const res = await fetch(`${state.baseUrl}/api/model`);
    const data = await res.json();
    const items = Array.isArray(data) ? data : (data?.data ?? []);
    return items.map((m: ApiModel) => ({
      label: `${m.id ?? "?"}  (${m.providerID ?? "?"})`,
      value: JSON.stringify({ model: m.id, provider: m.providerID }),
      description: m.variant
        ? `variant: ${m.variant}`
        : `provider: ${m.providerID ?? "?"}`,
    }));
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] listModels failed: ${e instanceof Error ? e.message : String(e)}\n`);
    return [];
  }
}

export async function listSkills(
  state: BotState,
): Promise<{ label: string; value: string; description: string }[]> {
  try {
    const res = await fetch(`${state.baseUrl}/api/skill`);
    const data = await res.json();
    const items = Array.isArray(data) ? data : (data?.data ?? []);
    return items.map((s: ApiSkill) => ({
      label: s.name || s.id || "?",
      value: s.id ?? "",
      description: s.description ?? "",
    }));
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] listSkills failed: ${e instanceof Error ? e.message : String(e)}\n`);
    return [];
  }
}

export async function listAgents(
  state: BotState,
): Promise<{ label: string; value: string; description: string }[]> {
  try {
    const res = await fetch(`${state.baseUrl}/api/agent`);
    const data = await res.json();
    const items = Array.isArray(data) ? data : (data?.data ?? []);
    return items.map((a: ApiAgent) => ({
      label: a.name || a.id || "?",
      value: JSON.stringify({ agent: a.id ?? a.name ?? "" }),
      description: a.description ?? a.id ?? "",
    }));
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] listAgents failed: ${e instanceof Error ? e.message : String(e)}\n`);
    return [];
  }
}

export async function fetchAllSessions(
  client: OpenCodeClient,
  baseUrl?: string,
): Promise<SessionShape[]> {
  const listingClient = (baseUrl ? createOpencodeClient({ baseUrl }) : client);

  try {
    const result = await listingClient.v2.session.list({ limit: 200 });
    const arr: unknown = result?.data?.data;
    if (Array.isArray(arr) && arr.length > 0) {
      return (arr as Array<{ id?: string; title?: string; location?: { directory?: string }; time?: { updated?: number } }>)
        .slice(0, 100).map((s) => formatSession({
          id: s.id, title: s.title,
          directory: s.location?.directory,
          updatedAt: s.time?.updated,
        }));
    }
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] v2 session.list failed: ${e instanceof Error ? e.message : String(e)}\n`);
  }

  try {
    const result: unknown = await listingClient.experimental.session.list();
    const rawArray: unknown =
      Array.isArray(result) ? result
        : (result as Record<string, unknown>)?.data
        ?? (result as Record<string, unknown>)?.sessions
        ?? (result as Record<string, unknown>)?.items
        ?? null;
    if (!rawArray) {
      const raw = JSON.stringify(result).slice(0, 200);
      process.stderr.write(`[opencode-bot] unexpected session.list format: ${raw}\n`);
      return [];
    }
    if (!Array.isArray(rawArray)) {
      process.stderr.write(`[opencode-bot] sessions is not an array: ${JSON.stringify(rawArray).slice(0, 200)}\n`);
      return [];
    }
    return (rawArray as Array<{ id?: string; title?: string; directory?: string; worktree?: string; location?: { directory?: string }; updatedAt?: number; time?: { updated?: number }; updated?: number }>)
      .slice(0, 100).map((s) => formatSession({
        id: s.id, title: s.title,
        directory: s.directory || s.worktree || s.location?.directory,
        updatedAt: s.updatedAt || s.time?.updated || s.updated,
      }));
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] fetchAllSessions failed: ${e instanceof Error ? e.message : String(e)}\n`);
    return [];
  } finally {
    if (listingClient !== client && typeof listingClient === "object") {
    }
  }
}

export async function listSessions(
  client: OpenCodeClient,
  baseUrl?: string,
): Promise<{ label: string; value: string; description: string }[]> {
  const sessions = await fetchAllSessions(client, baseUrl);
  return sessions.map((s: SessionShape) => ({
    label: s.title || s.id.slice(0, 8) || "Untitled",
    value: s.id,
    description: s.directory
      ? `📁 ${s.directory}`
      : `🕐 ${s.updatedAt ? new Date(s.updatedAt).toLocaleDateString("zh-CN") : "?"}`,
  }));
}

interface SessionDetail {
  title?: string;
  id?: string;
  directory?: string;
  worktree?: string;
  time?: { createdAt?: number };
  created?: number;
}

export async function getSessionInfo(
  client: OpenCodeClient,
  sessionId: string,
): Promise<string> {
  try {
    const result: unknown = await client.session.get({ sessionID: sessionId });
    const data = (result as { data?: SessionDetail })?.data;
    if (!data) return "未找到会话";
    const lines: string[] = [];
    lines.push(`📋 会话: ${data.title ?? "(无标题)"}  (${data.id || sessionId})`);
    if (data.directory || data.worktree)
      lines.push(`📁 目录: ${data.directory || data.worktree}`);
    if (data.time?.createdAt)
      lines.push(`🕐 创建: ${new Date(data.time.createdAt).toLocaleString("zh-CN")}`);
    else if (data.created)
      lines.push(`🕐 创建: ${new Date(data.created).toLocaleString("zh-CN")}`);
    return lines.join("\n");
  } catch (e: unknown) {
    return `获取会话信息失败: ${e instanceof Error ? e.message : String(e)}`;
  }
}


