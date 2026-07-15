import { tool } from "@opencode-ai/plugin";
import { spawnSync } from "child_process";
import * as crypto from "crypto";

// ── Constants ──────────────────────────────────────────────────

const SOCKET_DIR = "/tmp/yse-tmux";
const SHELL = "/bin/bash";
const POLL_MS = 150;
const MAX_STALE_MS = 120_000; // 2 min no change → return partial
const LINE_HAS_PROMPT = /[\w@][\w@:\-.\/~]*[$#%] ?$/;

// ── Helpers ────────────────────────────────────────────────────

function marker(): string {
  return `__YSE_${crypto.randomUUID().slice(0, 8)}__`;
}

function sanitize(s: string): string {
  return s.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
}

// ── Tmux SSH proxy ─────────────────────────────────────────────

function shQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

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
  tmuxProc(
    ["-f", "/dev/null", "-S", sock, "new-session", "-d", "-s", "yse", `exec ${SHELL}`],
    server,
    { stdio: "ignore" },
  );
  if (dir) {
    tmuxProc(
      ["-S", sock, "send-keys", "-t", "yse:0.0", "--", `cd ${shQuote(dir)}`, "Enter"],
      server,
      { stdio: "ignore" },
    );
  }
}

// ── Marker wait ────────────────────────────────────────────────

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

    // Standalone marker
    if (lines.some((l) => l.trim() === mark)) return;

    // Merged marker (not on typed "echo MARK" line)
    const merged = lines.findLastIndex(
      (l) => l.trimEnd().endsWith(mark) && !l.includes(`echo ${mark}`),
    );
    if (merged >= 0) return;

    // Progress detection: if output unchanged for 2 min, return partial
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

function hasMarker(l: string, mark: string): boolean {
  const t = l.trim();
  return t === mark || t.endsWith(mark);
}

// ── Capture algorithm (24/24 test pass) ────────────────────────

function captureOutput(
  sock: string,
  cmd: string,
  dir?: string,
  server?: string,
  signal?: AbortSignal,
): string {
  const START = marker();
  const END = marker();

  // 1. Clear + plant START anchor
  send(sock, `clear; echo ${START}`, server);
  waitFor(sock, START, server, signal);

  // 2. Send command + END marker
  const cd = dir ? `cd ${shQuote(dir)} && ` : "";
  send(sock, `${cd}(${cmd}); echo ${END}`, server);

  // 3. Wait for END, track partial output
  let partial = "";
  try {
    waitFor(sock, END, server, signal, (p) => { partial = p; });
  } catch {
    // If cancelled or timeout, return what we have
  }
  if (partial) return partial;

  const raw = capture(sock, server);
  const lines = raw.split("\n");
  const iStart = lines.findLastIndex((l) => hasMarker(l, START));
  const iEnd = lines.findLastIndex((l) => hasMarker(l, END));

  if (iStart < 0 || iEnd < 0 || iEnd <= iStart) {
    return `[PARSE_ERR] iStart=${iStart} iEnd=${iEnd}`;
  }

  // Between START and END (inclusive)
  const between = lines.slice(iStart + 1, iEnd + 1);
  const afterCmd = between.slice(1);
  const clean: string[] = [];

  for (let i = 0; i < afterCmd.length; i++) {
    const l = afterCmd[i];
    if (i === afterCmd.length - 1) {
      const content = l.replace(END, "").trim();
      if (content) clean.push(content);
      continue;
    }
    // Pure prompt line?
    const m = l.match(LINE_HAS_PROMPT);
    const isPure = m && (m.index === 0 || l.trimEnd() === m[0]);
    if (isPure) continue;
    if (m && m.index! > 0) {
      clean.push(l.slice(0, m.index)); // extract text before prompt
    } else {
      clean.push(l);
    }
  }

  return clean.join("\n").trim();
}

// ── Progress patterns (early return) ──────────────────────────

const PROGRESS_RE = [
  /\d+\/\d+/,       // "1/20"
  /\d+%/,           // "25%"
  /\[=+>+\s*\]/,    // "[====>     ]"
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

    // Ensure session
    ensureSession(sock, server, dir);

    // Execute via tmux capture
    return captureOutput(sock, cmd, dir, server, context.abort);
  },
});
