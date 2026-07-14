import { createOpencodeClient } from "@opencode-ai/sdk/v2";
import type { BotState, OpenCodeClient, ApiModel, ApiSkill, ApiAgent, SessionShape } from "./opencode.js";

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

export async function listVariants(
  baseUrl: string,
): Promise<{ label: string; value: string; description: string }[]> {
  try {
    const res = await fetch(`${baseUrl}/api/model`);
    const data = await res.json();
    const items: Array<{ id?: string; providerID?: string; variant?: string }> =
      Array.isArray(data) ? data : (data?.data ?? []);
    return items.map((m) => ({
      label: `${m.id ?? "?"} / ${m.variant ?? "default"}`,
      value: JSON.stringify({ model: m.id, provider: m.providerID, variant: m.variant }),
      description: `provider: ${m.providerID ?? "?"}`,
    }));
  } catch {
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
        .slice(0, 100).map((s) => ({
          id: s.id ?? "",
          title: s.title ?? "",
          directory: s.location?.directory ?? "",
          updatedAt: s.time?.updated ?? 0,
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
      .slice(0, 100).map((s) => ({
        id: s.id ?? "",
        title: s.title ?? "",
        directory: s.directory || s.worktree || s.location?.directory || "",
        updatedAt: s.updatedAt || s.time?.updated || s.updated || 0,
      }));
  } catch (e: unknown) {
    process.stderr.write(`[opencode-bot] fetchAllSessions failed: ${e instanceof Error ? e.message : String(e)}\n`);
    return [];
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

export async function getSessionInfo(
  client: OpenCodeClient,
  sessionId: string,
): Promise<string> {
  try {
    const result: unknown = await client.session.get({ sessionID: sessionId });
    const data = (result as { data?: { title?: string; id?: string; directory?: string; worktree?: string; time?: { createdAt?: number }; created?: number } })?.data;
    if (!data) return "未找到会话";
    const lines: string[] = [];
    lines.push(`📋 会话: ${data.title ?? "(无标题)"}  (${data.id || sessionId})`);
    if (data.directory || data.worktree)
      lines.push(`📁 目录: ${data.directory || data.worktree}`);
    if (data.time?.createdAt)
      lines.push(`🕐 创建: ${new Date(data.time.createdAt).toLocaleString("zh-CN")}`);
    else if (data.created)
      lines.push(`🕐 创建: ${new Date(data.created).toLocaleString("zh-CN")}`);
    const sid = (sessionId || "default").replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
    const sock = `/tmp/yse-tmux/yse-${sid}.sock`;
    lines.push(`kitty: kitty tmux -S ${sock} attach`);
    return lines.join("\n");
  } catch (e: unknown) {
    return `获取会话信息失败: ${e instanceof Error ? e.message : String(e)}`;
  }
}
