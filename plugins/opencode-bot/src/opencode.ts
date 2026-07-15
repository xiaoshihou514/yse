import { createOpencodeClient } from "@opencode-ai/sdk/v2";
import { type ChildProcess } from "child_process";
import { log } from "./logger.js";

export type OpenCodeClient = ReturnType<typeof createOpencodeClient>;

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
  client: OpenCodeClient;
  projectDir: string;
  baseUrl: string;
  sessions: {
    [userAddr: string]: SessionState;
  };
  modelConfig: ModelConfig;
  serverProcess: ChildProcess | null;
}

export interface SessionShape {
  id: string; title: string; directory: string; updatedAt: number;
}

export interface ApiModel {
  id?: string;
  providerID?: string;
  variant?: string;
}

export interface ApiSkill {
  id?: string;
  name?: string;
  description?: string;
}

export interface ApiAgent {
  id?: string;
  name?: string;
  description?: string;
}

interface PromptPart {
  type: string;
  text?: string;
}

interface PromptParams {
  sessionID: string;
  parts: { type: "text"; text: string }[];
  directory?: string;
  model?: { modelID: string; providerID: string; variant?: string };
  agent?: string;
}

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
): PromptParams {
  const params: PromptParams = {
    sessionID: sessionId,
    parts: [{ type: "text", text }],
    ...(directory ? { directory } : {}),
  };
  if (spec.modelId && spec.providerId) {
    params.model = {
      modelID: spec.modelId,
      providerID: spec.providerId,
      ...(spec.variant ? { variant: spec.variant } : {}),
    };
  } else if (agentId) {
    params.agent = agentId;
  }
  return params;
}

// ---- Pure model resolution logic ----

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

// ---- State management ----

export function userKey(addr: string): string {
  const i = addr.indexOf("#");
  return i >= 0 ? addr.slice(0, i) : addr;
}

export function getUserState(
  state: BotState,
  userAddr: string,
): SessionState {
  const key = userKey(userAddr);
  if (!state.sessions[key]) {
    state.sessions[key] = { mode: "sdk", sessionId: null, modelMode: "global" };
  }
  return state.sessions[key];
}

// ---- Prompt sending ----

export interface PromptResult {
  text: string;
  tokens?: { input: number; output: number; reasoning?: number; cache?: { read: number; write: number } };
}

export async function sendPromptStreaming(
  client: OpenCodeClient,
  sessionId: string,
  text: string,
  directory: string | undefined,
  chain: ModelSpec[],
  agentId: string | undefined,
  onEvent: (type: string, data: any) => void,
  signal?: AbortSignal,
): Promise<PromptResult> {
  let ourMessageId: string | null = null;
  const abortController = new AbortController();
  let eventStream: any = null;
  let eventConsumer: Promise<void> | null = null;

  if (signal) {
    signal.addEventListener("abort", () => {
      client.session.abort({ sessionID: sessionId }).catch(() => {});
      abortController.abort();
    }, { once: true });
  }

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

          log(`SSE event: type=${ev.type} id=${ev.id}`);

          if (ev.type === "question.v2.asked") {
            try {
              onEvent("question_asked", {
                requestID: ev.id,
                sessionID: p.sessionID,
                questions: p.questions,
              });
            } catch (e: unknown) {
              log(`question_asked onEvent error: ${e}`);
            }
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
        log(`SSE consumer error: ${e instanceof Error ? e.message : String(e)}`);
      }
    })();
  } catch (e: unknown) {
    log(`SSE subscribe failed: ${e instanceof Error ? e.message : String(e)}`);
  }

  try {
    let promptInfo: any = null;
    const resultText = await tryModelsWithFallback(
      chain,
      async (spec) => {
        ourMessageId = null;
        const params = buildPromptParams(sessionId, text, directory, spec, agentId);
        const result = await client.session.prompt(params);
        promptInfo = (result.data as any)?.info;
        return extractTextParts(result.data as { parts?: PromptPart[] });
      },
      (from, to) => {
        onEvent("model_switched", {
          from: { modelId: from.modelId, providerId: from.providerId },
          to: { modelId: to.modelId, providerId: to.providerId },
        });
      },
    );
    const tokens = promptInfo?.tokens;
    return {
      text: resultText,
      tokens: tokens ? {
        input: tokens.input ?? tokens.prompt,
        output: tokens.output ?? tokens.completion,
        reasoning: tokens.reasoning,
        cache: tokens.cache,
      } : undefined,
    };
  } catch (e: unknown) {
    return { text: `Error: ${e instanceof Error ? e.message : String(e)}` };
  } finally {
    abortController.abort();
    if (eventConsumer) {
      try { await eventConsumer; } catch (e: unknown) {
        log(`eventConsumer await failed: ${e instanceof Error ? e.message : String(e)}`);
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
