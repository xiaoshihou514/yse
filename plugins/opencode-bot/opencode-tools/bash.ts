import { tool } from "@opencode-ai/plugin";
import { spawnSync } from "child_process";
import * as crypto from "crypto";

// ── Constants ──────────────────────────────────────────────────

const SOCKET_DIR = "/tmp/yse-tmux";
const SHELL = "/bin/bash";
const POLL_MS = 150;
const MAX_STALE_MS = 120_000; // 2 min no change → return partial

// ── Helpers ────────────────────────────────────────────────────

function sanitize(s: string): string {
  return s.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
}

function shQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

// ── Tmux SSH proxy ─────────────────────────────────────────────

function tmuxProc(
  args: string[],
  server?: string,
  opts?: { stdio?: "ignore"; encoding?: string; maxBuffer?: number; timeout?: number },
) {
  const [cmd, cmdArgs] = server
    ? ["ssh", ["-o", "RequestTTY=no", server, ["tmux", ...args].map(shQuote).join(" ")] as string[]]
    : ["tmux", args as string[]];
  const r = spawnSync(cmd, cmdArgs, {
    encoding: opts?.encoding ?? "utf-8",
    maxBuffer: opts?.maxBuffer ?? 1024 * 1024,
    timeout: opts?.timeout ?? 10000,
    stdio: opts?.stdio,
  });
  return r;
}

function send(sock: string, text: string, server?: string) {
  tmuxProc(
    ["-S", sock, "send-keys", "-t", "yse:0.0", "--", text, "Enter"],
    server,
    { stdio: "ignore" },
  );
}

function capture(sock: string, server?: string): string {
  const r = tmuxProc(
    ["-S", sock, "capture-pane", "-p", "-J", "-S", "-", "-t", "yse:0.0"],
    server,
    { maxBuffer: 4 * 1024 * 1024 },
  );
  return r.stdout ?? "";
}

function ensureSession(sock: string, server?: string, dir?: string) {
  spawnSync("mkdir", ["-p", SOCKET_DIR], { stdio: "ignore" });
  if (server) {
    spawnSync("ssh", ["-o", "RequestTTY=no", server, `mkdir -p ${shQuote(SOCKET_DIR)}`],
      { stdio: "ignore", timeout: 5000 },
    );
  }
  const args = ["-f", "/dev/null", "-S", sock, "new-session", "-d", "-s", "yse"];
  if (dir) args.push("-c", dir);
  args.push(SHELL);
  tmuxProc(args, server, { stdio: "ignore" });
}

// ── Polling wait ────────────────────────────────────────────────

function waitFor(
  sock: string,
  mark: string,
  server?: string,
  signal?: AbortSignal,
  onProgress?: (out: string) => void,
): void {
  const start = Date.now();
  let lastChange = start;
  let prev = "";

  while (true) {
    if (signal?.aborted) throw new Error("aborted");

    const out = capture(sock, server);
    const lines = out.split("\n");

    if (lines.some((l) => l.trim() === mark)) return;

    if (out !== prev) {
      prev = out;
      lastChange = Date.now();
    } else if (Date.now() - lastChange > MAX_STALE_MS) {
      onProgress?.(out);
      return;
    }

    spawnSync("sleep", [String(POLL_MS / 1000)], { stdio: "ignore" });
  }
}

// ── Capture via PS1 marker ──────────────────────────────────────

function captureOutput(
  sock: string,
  cmd: string,
  dir?: string,
  server?: string,
  signal?: AbortSignal,
): string {
  const PS1 = `__YSE_${crypto.randomUUID().slice(0, 8)}__`;
  const cd = dir ? `cd ${shQuote(dir)} && ` : "";

  // Set PS1 to random marker + run command
  send(sock, `PS1='${PS1}'; ${cd}(${cmd})`, server);

  // Wait for PS1 prompt to appear (command finished)
  let partial = "";
  try {
    waitFor(sock, PS1, server, signal, (p) => { partial = p; });
  } catch {
    if (partial) return partial;
    return "aborted";
  }

  const raw = capture(sock, server);
  const lines = raw.split("\n");

  // Find PS1 prompt after command completion
  const endIdx = lines.findLastIndex((l) => l.trim() === PS1);
  if (endIdx < 0) return `[PARSE_ERR] prompt not found: PS1=${PS1}`;

  // Find typed command line (contains PS1=)
  const typedIdx = lines.findLastIndex((l) => l.includes(`PS1='${PS1}'`), endIdx);
  if (typedIdx < 0) return `[PARSE_ERR] typed command not found`;

  // Output = lines between typed command and prompt
  return lines.slice(typedIdx + 1, endIdx).join("\n").trim();
}

// ── Progress patterns (early return) ──────────────────────────

const PROGRESS_RE = [
  /\d+\/\d+/,
  /\d+%/,
  /\[=+>+\s*\]/,
];

function hasProgress(out: string): boolean {
  return PROGRESS_RE.some((r) => r.test(out));
}

// ── Tool definition ────────────────────────────────────────────

export default tool({
  description: `Execute shell commands via tmux. Supports long-running tasks without timeout.
  - "command": the shell command to run
  - "server" (optional): SSH host for remote execution
  The user can attach with: tmux -S /tmp/yse-tmux/yse-<sessionID>.sock attach
  If output contains progress indicators (\\d+/\\d+, \\d+%), the tool may return partial output early.`,

  args: {
    command: tool.schema.string().describe("The shell command to execute"),
    directory: tool.schema.string().optional().describe("Working directory"),
    server: tool.schema.string().optional().describe("SSH remote host (e.g. user@host)"),
  },

  async execute(args, context) {
    const cmd = args.command.trim();
    if (!cmd) return "";

    const sid = sanitize(context.sessionID || "default");
    const sock = `${SOCKET_DIR}/yse-${sid}.sock`;
    const dir = args.directory || context.directory || context.worktree;
    const server = args.server;

    ensureSession(sock, server, dir);
    return captureOutput(sock, cmd, dir, server, context.abort);
  },
});
