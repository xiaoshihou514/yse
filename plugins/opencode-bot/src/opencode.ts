import { createOpencodeClient } from "@opencode-ai/sdk/v2";

export interface BotState {
  client: ReturnType<typeof createOpencodeClient>;
  projectDir: string;
  sessions: {
    [userAddr: string]: {
      mode: "sdk" | "tui";
      sessionId: string | null;
    };
  };
}

export async function initBot(): Promise<BotState | null> {
  try {
    const client = createOpencodeClient({ baseUrl: "http://localhost:4096" });
    let projectDir = process.cwd();
    try {
      const proj = await client.project.current();
      const data = proj.data as any;
      if (data?.worktree) projectDir = data.worktree;
    } catch {}
    return { client, projectDir, sessions: {} };
  } catch (e: any) {
    return null;
  }
}

export function getUserState(state: BotState, userAddr: string): BotState["sessions"][string] {
  if (!state.sessions[userAddr]) {
    state.sessions[userAddr] = { mode: "sdk", sessionId: null };
  }
  return state.sessions[userAddr];
}

export async function sendPrompt(client: any, sessionId: string, text: string): Promise<string> {
  try {
    const result = await client.session.prompt({
      path: { id: sessionId },
      body: {
        parts: [{ type: "text", text }],
      },
    });
    const info = result.data?.info as any;
    const parts: any[] = info?.parts ?? [];
    const texts = parts
      .filter((p: any) => p.type === "text")
      .map((p: any) => p.text)
      .filter(Boolean);
    return texts.join("\n") || "(empty response)";
  } catch (e: any) {
    return `Error: ${e.message ?? e}`;
  }
}

export async function sendTuiPrompt(client: any, text: string): Promise<void> {
  await client.tui.appendPrompt({ body: { text } });
  await client.tui.submitPrompt();
}

export async function listSessions(client: any): Promise<
  { label: string; value: string; description: string }[]
> {
  try {
    const result = await client.session.list();
    const sessions: any[] = result.data ?? [];
    return sessions.slice(0, 20).map((s: any) => ({
      label: s.title || s.id?.slice(0, 8) || "Untitled",
      value: s.id,
      description: s.worktree
        ? `📁 ${s.worktree}`
        : `🕐 ${s.time?.updatedAt ? new Date(s.time.updatedAt).toLocaleDateString("zh-CN") : "?"}`,
    }));
  } catch {
    return [];
  }
}

export async function getSessionInfo(client: any, sessionId: string): Promise<string> {
  try {
    const s = await client.session.get({ path: { id: sessionId } });
    const data = s.data as any;
    if (!data) return "未找到会话";
    const lines: string[] = [];
    lines.push(`📋 会话: ${data.title ?? "(无标题)"}  (${data.id})`);
    if (data.worktree) lines.push(`📁 目录: ${data.worktree}`);
    if (data.time?.createdAt) lines.push(`🕐 创建: ${new Date(data.time.createdAt).toLocaleString("zh-CN")}`);
    return lines.join("\n");
  } catch {
    return "获取会话信息失败";
  }
}
