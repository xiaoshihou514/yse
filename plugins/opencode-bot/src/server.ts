import { createOpencodeClient } from "@opencode-ai/sdk/v2";
import { spawn, type ChildProcess } from "child_process";
import path from "path";
import { fileURLToPath } from "url";
import type { BotState } from "./opencode.js";

let _serverProcess: ChildProcess | null = null;

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WORKSPACE = path.resolve(__dirname, "../workspace");

process.on("exit", () => {
  if (_serverProcess) {
    _serverProcess.kill();
  }
});

function startServer(): { child: ChildProcess; port: Promise<number> } {
  const child = spawn("opencode", ["serve", "--port", "0", "--print-logs"], {
    cwd: WORKSPACE,
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
