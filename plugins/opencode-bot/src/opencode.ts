import { createOpencodeClient } from "@opencode-ai/sdk/v2";
import { spawn, type ChildProcess } from "child_process";

let _serverProcess: ChildProcess | null = null;

process.on("exit", () => {
  if (_serverProcess) {
    _serverProcess.kill();
  }
});

export interface BotState {
  client: ReturnType<typeof createOpencodeClient>;
  projectDir: string;
  sessions: {
    [userAddr: string]: {
      mode: "sdk" | "tui";
      sessionId: string | null;
    };
  };
  serverProcess: ChildProcess | null;
}

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

    // Retry project.current() — server may not be ready immediately
    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        const probe = createOpencodeClient({ baseUrl, directory: projectDir });
        const proj = await probe.project.current();
        const data = proj.data as any;
        if (data?.worktree) projectDir = data.worktree;
        break;
      } catch {
        if (attempt < 2) {
          await new Promise((r) => setTimeout(r, 1500));
        }
      }
    }

    const client = createOpencodeClient({
      baseUrl,
      directory: projectDir,
    });

    return { client, projectDir, sessions: {}, serverProcess: child };
  } catch (e: any) {
    process.stderr.write(`[opencode-bot] initBot failed: ${e.message ?? e}\n`);
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
): BotState["sessions"][string] {
  if (!state.sessions[userAddr]) {
    state.sessions[userAddr] = { mode: "sdk", sessionId: null };
  }
  return state.sessions[userAddr];
}

export async function sendPrompt(
  client: any,
  sessionId: string,
  text: string,
): Promise<string> {
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

export async function listSessions(
  client: any,
): Promise<{ label: string; value: string; description: string }[]> {
  try {
    const result = await client.experimental.session.list();
    let sessions: any[];
    if (Array.isArray(result)) {
      sessions = result;
    } else if (result?.data) {
      sessions = result.data;
    } else if (result?.sessions) {
      sessions = result.sessions;
    } else if (result?.items) {
      sessions = result.items;
    } else {
      const raw = JSON.stringify(result).slice(0, 200);
      process.stderr.write(`[opencode-bot] unexpected session.list format: ${raw}\n`);
      return [];
    }
    return sessions.slice(0, 20).map((s: any) => ({
      label: s.title || s.id?.slice(0, 8) || "Untitled",
      value: s.id,
      description: s.directory || s.worktree
        ? `📁 ${s.directory || s.worktree}`
        : `🕐 ${s.updatedAt || s.updated ? new Date((s.updatedAt || s.updated) as number).toLocaleDateString("zh-CN") : "?"}`,
    }));
  } catch (e: any) {
    process.stderr.write(`[opencode-bot] listSessions failed: ${e.message ?? e}\n`);
    return [];
  }
}

export async function getSessionInfo(
  client: any,
  sessionId: string,
): Promise<string> {
  try {
    const s = await client.session.get({ sessionID: sessionId });
    const data = s.data as any;
    if (!data) return "未找到会话";
    const lines: string[] = [];
    lines.push(`📋 会话: ${data.title ?? "(无标题)"}  (${data.id || sessionId})`);
    if (data.directory || data.worktree)
      lines.push(`📁 目录: ${data.directory || data.worktree}`);
    if (data.time?.createdAt)
      lines.push(
        `🕐 创建: ${new Date(data.time.createdAt).toLocaleString("zh-CN")}`,
      );
    else if (data.created)
      lines.push(
        `🕐 创建: ${new Date(data.created).toLocaleString("zh-CN")}`,
      );
    return lines.join("\n");
  } catch (e: any) {
    return `获取会话信息失败: ${e.message ?? e}`;
  }
}
