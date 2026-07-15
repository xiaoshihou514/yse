import { createOpencodeClient, createOpencodeServer } from "@opencode-ai/sdk/v2";
import path from "path";
import { fileURLToPath } from "url";
import type { BotState, OpenCodeClient } from "./opencode.js";
import { log } from "./logger.js";

let _client: OpenCodeClient | null = null;

export function getClient(): OpenCodeClient | null {
  return _client;
}

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const YSE_ROOT = path.resolve(__dirname, "../../..");

let _server: { close: () => void } | null = null;

process.on("exit", () => {
  _server?.close();
});

const CONFIG = {
  agent: {
    build: {
      tools: { bash: false },
    },
    plan: {
      tools: { bash: false },
    },
  },
};

export async function initBot(): Promise<BotState | null> {
  try {
    const server = await createOpencodeServer({ port: 0, config: CONFIG });
    _server = server;
    const baseUrl = server.url;
    const actualPort = new URL(baseUrl).port;
    log(`server started on port ${actualPort}`);

    let projectDir = YSE_ROOT;

    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        const probe = createOpencodeClient({ baseUrl, directory: projectDir });
        const proj: { data?: { worktree?: string } } = await probe.project.current();
        if (proj.data?.worktree) projectDir = proj.data.worktree;
        break;
      } catch (e: unknown) {
        log(`project.current attempt ${attempt} failed: ${e instanceof Error ? e.message : String(e)}`);
        if (attempt < 2) {
          await new Promise((r) => setTimeout(r, 1500));
        }
      }
    }

    const client = createOpencodeClient({
      baseUrl,
      directory: projectDir,
    });
    _client = client;

    return {
      client, projectDir, baseUrl,
      sessions: {},
      modelConfig: { defaultModel: undefined, fallbackChain: [] },
      serverProcess: server,
    };
  } catch (e: unknown) {
    log(`initBot failed: ${e instanceof Error ? e.message : String(e)}`);
    return null;
  }
}

export function killServer(state: BotState) {
  state.serverProcess?.close();
}
