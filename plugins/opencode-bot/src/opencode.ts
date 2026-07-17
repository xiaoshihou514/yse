import { createOpencodeClient } from "@opencode-ai/sdk/v2";
import { log } from "./logger.js";
import { logToFile } from "./logger.js";

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
  serverProcess: { close: () => void } | null;
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

function extractTextParts(msg: { text?: string; parts?: PromptPart[]; content?: PromptPart[] }): string {
  if (msg.text) return msg.text;
  const items = msg.content ?? msg.parts ?? [];
  const textParts = items.filter((p) => p.type === "text").map((p) => p.text).filter((t): t is string => !!t);
  return textParts.join("\n") || "(empty response)";
}

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

// ---- Prompt sending (v1 API) ----

export interface PromptResult {
  text: string;
  tokens?: { input: number; output: number; reasoning?: number; cache?: { read: number; write: number } };
}

export async function sendPromptStreaming(
  client: OpenCodeClient,
  sessionId: string,
  text: string,
  onEvent: (type: string, data: any) => void,
  signal?: AbortSignal,
  agentId?: string,
): Promise<PromptResult> {
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

          if (ev.type === "message.part.delta") {
            if (p?.delta) logToFile(`[text] ${p.delta}`);
          }

          if (ev.type === "question.v2.asked" || ev.type === "question.asked") {
            try {
              onEvent("question_asked", {
                requestID: ev.id,
                sessionID: p.sessionID,
                questions: p.questions,
              });
            } catch (e: unknown) {
              log(`question_asked onEvent error: ${e}`);
            }
            client.v2.session.question.reject({ sessionID: sessionId, requestID: ev.id }).catch(() => {});
            continue;
          }

          if (ev.type === "permission.asked") {
            try {
              onEvent("permission_asked", {
                requestID: p.id || ev.id,
                sessionID: p.sessionID,
                permission: p.permission,
                patterns: p.patterns,
              });
            } catch (e: unknown) {
              log(`permission_asked onEvent error: ${e}`);
            }
            client.v2.session.permission.reply({ sessionID: sessionId, requestID: p.id || ev.id, reply: "always" }).catch(() => {});
            continue;
          }

          if (ev.type === "message.part.updated") {
            const part = p.info?.part || p.part;
            if (!part || part.type !== "tool") continue;
            const s = part.state;
            if (!s) continue;
            if (s.status === "running" || s.status === "pending") {
              try { onEvent("tool_called", { name: part.tool, input: s.input }); } catch (e: unknown) {
                log(`tool_called onEvent error: ${e}`);
              }
            } else if (s.status === "completed") {
              try { onEvent("tool_success", { name: part.tool, output: s.output || "", result: s.metadata }); } catch (e: unknown) {
                log(`tool_success onEvent error: ${e}`);
              }
            } else if (s.status === "failed") {
              try { onEvent("tool_failed", { name: part.tool, error: s.error }); } catch (e: unknown) {
                log(`tool_failed onEvent error: ${e}`);
              }
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
    log(`v1 prompt: session=${sessionId}${agentId ? ` agent=${agentId}` : ""}`);
    const params: any = { sessionID: sessionId, parts: [{ type: "text", text }] };
    if (agentId) params.agent = agentId;

    let result: any;
    while (true) {
      try {
        result = await client.session.prompt(params);
        if (!result.error) break;
        const errMsg = (result.error as any)?.data?.message ?? (result.error as any)?.message ?? JSON.stringify(result.error);
        log(`v1 prompt error: ${errMsg}, retrying...`);
      } catch (e: unknown) {
        log(`v1 prompt failed: ${e instanceof Error ? e.message : String(e)}, retrying...`);
      }
      if (signal?.aborted) {
        return { text: "Error: aborted" };
      }
      await new Promise(r => setTimeout(r, 2000));
    }

    const data = result.data as { parts?: PromptPart[]; info?: any };
    let textResult = extractTextParts(data);
    let tokens = data?.info?.tokens;

    // If v1 prompt returned empty parts, fall back to v2 messages()
    if (!data.parts?.length || textResult === "(empty response)") {
      const msgsResult = await client.v2.session.messages({ sessionID: sessionId, order: "desc", limit: 5 });
      const msgs: any[] = msgsResult.data?.data ?? [];
      const assistantMsg = msgs.find((m: any) => (m.role ?? m.type) === "assistant");
      if (assistantMsg) {
        textResult = extractTextParts(assistantMsg);
        tokens = assistantMsg.tokens;
        log(`v2 messages fallback: content=${(assistantMsg.content ?? assistantMsg.parts)?.length} text_len=${textResult.length}`);
      }
    }

    log(`v1 result: parts=${data.parts?.length ?? 0} text_len=${textResult.length}`);

    return {
      text: textResult,
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
  agentId?: string,
): Promise<string> {
  const params: any = { sessionID: sessionId, parts: [{ type: "text", text }] };
  if (agentId) params.agent = agentId;

  while (true) {
    try {
      const result = await client.session.prompt(params);
      if (result.data) {
        let textResult = extractTextParts(result.data as { parts?: PromptPart[] });
        if (textResult === "(empty response)") {
          const msgsResult = await client.v2.session.messages({ sessionID: sessionId, order: "desc", limit: 5 });
          const msgs: any[] = msgsResult.data?.data ?? [];
          const assistantMsg = msgs.find((m: any) => (m.role ?? m.type) === "assistant");
          if (assistantMsg) textResult = extractTextParts(assistantMsg);
        }
        if (textResult !== "(empty response)") return textResult;
        return textResult;
      }
      const errMsg = (result.error as any)?.data?.message ?? (result.error as any)?.message ?? JSON.stringify(result.error);
      log(`sendPrompt error: ${errMsg}, retrying...`);
    } catch (e: unknown) {
      log(`sendPrompt failed: ${e instanceof Error ? e.message : String(e)}, retrying...`);
    }
    await new Promise(r => setTimeout(r, 2000));
  }
}
