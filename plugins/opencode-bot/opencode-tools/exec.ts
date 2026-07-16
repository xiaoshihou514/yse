import { tool } from "@opencode-ai/plugin";
import { spawnSync } from "child_process";
import * as crypto from "crypto";

// ── Constants ──────────────────────────────────────────────────

const SOCKET_DIR = "/tmp/yse-tmux";
const SSH_CONTROL_DIR = "/tmp/yse-ssh";
const SHELL = "/bin/bash";
const POLL_MS = 150;
const MAX_STALE_MS = 120_000;

// ── Helpers ────────────────────────────────────────────────────

function sanitize(s: string): string {
  return s.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
}

function shQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

// ── Types & State ─────────────────────────────────────────────

interface ExecResult {
  status: "completed" | "running" | "error";
  output?: string;
  task_id?: string;
  message?: string;
}

interface TaskInfo {
  id: string;
  pane: string;
  ps1: string;
  cmd: string;
  startTime: number;
}

let taskCounter = 0;
const tasks = new Map<string, TaskInfo>();

// ── SSH ControlMaster ─────────────────────────────────────────

function sshControlPath(server: string, sid: string): string {
  const hash = crypto.createHash("md5").update(server).digest("hex").slice(0, 8);
  return `${SSH_CONTROL_DIR}/${sanitize(sid)}-${hash}`;
}

function ensureSSHMaster(server: string, sid: string): string {
  spawnSync("mkdir", ["-p", SSH_CONTROL_DIR], { stdio: "ignore" });
  const cp = sshControlPath(server, sid);
  const check = spawnSync("ssh", ["-O", "check", "-S", cp, server], {
    stdio: "ignore",
    timeout: 5000,
  });
  if (check.status !== 0) {
    spawnSync("ssh", ["-M", "-S", cp, "-f", "-N", server], {
      stdio: "ignore",
      timeout: 10000,
    });
  }
  return cp;
}

// ── Tmux SSH proxy ─────────────────────────────────────────────

interface TmuxOpts {
  stdio?: "ignore";
  encoding?: string;
  maxBuffer?: number;
  timeout?: number;
  controlPath?: string;
}

function tmuxProc(args: string[], server?: string, opts?: TmuxOpts) {
  const cp = opts?.controlPath;
  let cmd: string;
  let cmdArgs: string[];
  if (server && cp) {
    cmd = "ssh";
    cmdArgs = ["-S", cp, server, ["tmux", ...args].map(shQuote).join(" ")];
  } else if (server) {
    cmd = "ssh";
    cmdArgs = ["-o", "RequestTTY=no", server, ["tmux", ...args].map(shQuote).join(" ")];
  } else {
    cmd = "tmux";
    cmdArgs = args;
  }
  return spawnSync(cmd, cmdArgs, {
    encoding: opts?.encoding ?? "utf-8",
    maxBuffer: opts?.maxBuffer ?? 1024 * 1024,
    timeout: opts?.timeout ?? 10000,
    stdio: opts?.stdio,
  });
}

// ── Pane operations ───────────────────────────────────────────

function send(
  sock: string,
  text: string,
  server?: string,
  opts?: { controlPath?: string; pane?: string },
) {
  const pane = opts?.pane || "yse:main";
  tmuxProc(
    ["-S", sock, "send-keys", "-t", pane, "--", text, "Enter"],
    server,
    { controlPath: opts?.controlPath, stdio: "ignore" },
  );
}

function capture(
  sock: string,
  server?: string,
  opts?: { controlPath?: string; pane?: string },
): string {
  const pane = opts?.pane || "yse:main";
  const r = tmuxProc(
    ["-S", sock, "capture-pane", "-p", "-J", "-S", "-", "-t", pane],
    server,
    { controlPath: opts?.controlPath, maxBuffer: 4 * 1024 * 1024 },
  );
  return r.stdout ?? "";
}

function ensureSession(
  sock: string,
  server?: string,
  dir?: string,
  controlPath?: string,
) {
  spawnSync("mkdir", ["-p", SOCKET_DIR], { stdio: "ignore" });
  if (server) {
    const sshArgs = controlPath
      ? ["-S", controlPath, server, `mkdir -p ${shQuote(SOCKET_DIR)}`]
      : ["-o", "RequestTTY=no", server, `mkdir -p ${shQuote(SOCKET_DIR)}`];
    spawnSync("ssh", sshArgs, { stdio: "ignore", timeout: 5000 });
  }
  const args = ["-f", "/dev/null", "-S", sock, "new-session", "-d", "-s", "yse", "-n", "main"];
  if (dir) args.push("-c", dir);
  args.push(SHELL);
  tmuxProc(args, server, { controlPath, stdio: "ignore" });
  tmuxProc(
    ["-S", sock, "rename-window", "-t", "yse:0", "main"],
    server,
    { controlPath, stdio: "ignore" },
  );
}

function renameMainToTask(
  sock: string,
  server?: string,
  controlPath?: string,
  _cmd?: string,
  _ps1?: string,
): string {
  const id = `task_${++taskCounter}`;
  const winName = `task-${taskCounter}`;
  tmuxProc(
    ["-S", sock, "rename-window", "-t", "yse:main", winName],
    server,
    { controlPath, stdio: "ignore" },
  );
  tmuxProc(
    ["-S", sock, "new-window", "-d", "-n", "main", SHELL],
    server,
    { controlPath, stdio: "ignore" },
  );
  return id;
}

// ── Execute command ────────────────────────────────────────────

function executeCommand(
  sock: string,
  cmd: string,
  dir?: string,
  server?: string,
  opts?: { controlPath?: string; signal?: AbortSignal },
): ExecResult {
  const PS1 = `__YSE_${crypto.randomUUID().slice(0, 8)}__`;
  const cd = dir ? `cd ${shQuote(dir)} && ` : "";

  send(sock, `PS1='${PS1}'; ${cd}(${cmd})`, server, { controlPath: opts?.controlPath });

  const start = Date.now();
  let lastChange = start;
  let prev = "";
  let partial = "";

  while (true) {
    if (opts?.signal?.aborted) {
      const taskId = renameMainToTask(sock, server, opts?.controlPath, cmd, PS1);
      tasks.set(taskId, {
        id: taskId,
        pane: `yse:task-${taskCounter}`,
        ps1: PS1,
        cmd,
        startTime: Date.now(),
      });
      const err = new Error(`命令被中断，已转入后台 task_id=${taskId}`);
      (err as Record<string, unknown>).details = { task_id: taskId };
      throw err;
    }

    const out = capture(sock, server, { controlPath: opts?.controlPath });
    const lines = out.split("\n");
    partial = out;

    if (lines.some((l) => l.trim() === PS1)) {
      const raw = capture(sock, server, { controlPath: opts?.controlPath });
      const allLines = raw.split("\n");
      const endIdx = allLines.findLastIndex((l) => l.trim() === PS1);
      const typedIdx = allLines.findLastIndex((l) => l.includes(`PS1='${PS1}'`), endIdx);
      if (typedIdx < 0 || endIdx < 0) {
        return { status: "error", message: "PARSE_ERR: prompt markers not found" };
      }
      return {
        status: "completed",
        output: allLines.slice(typedIdx + 1, endIdx).join("\n").trim(),
      };
    }

    if (out !== prev) {
      prev = out;
      lastChange = Date.now();
    } else if (Date.now() - lastChange > MAX_STALE_MS) {
      const taskId = renameMainToTask(sock, server, opts?.controlPath, cmd, PS1);
      tasks.set(taskId, {
        id: taskId,
        pane: `yse:task-${taskCounter}`,
        ps1: PS1,
        cmd,
        startTime: Date.now(),
      });
      return {
        status: "running",
        task_id: taskId,
        message: "命令仍在执行中，稍后使用 task_id 获取结果。",
        output: partial,
      };
    }

    spawnSync("sleep", [String(POLL_MS / 1000)], { stdio: "ignore" });
  }
}

// ── Query background task ──────────────────────────────────────

function queryTask(
  sock: string,
  taskId: string,
  server?: string,
  opts?: { controlPath?: string; signal?: AbortSignal },
): ExecResult {
  const task = tasks.get(taskId);
  if (!task) {
    return { status: "error", message: "任务不存在或已被清理" };
  }

  const start = Date.now();
  let lastChange = start;
  let prev = "";

  while (true) {
    if (opts?.signal?.aborted) throw new Error("aborted");

    const out = capture(sock, server, { controlPath: opts?.controlPath, pane: task.pane });
    const lines = out.split("\n");

    if (lines.some((l) => l.trim() === task.ps1)) {
      const raw = capture(sock, server, { controlPath: opts?.controlPath, pane: task.pane });
      const allLines = raw.split("\n");
      const endIdx = allLines.findLastIndex((l) => l.trim() === task.ps1);
      const typedIdx = allLines.findLastIndex((l) => l.includes(`PS1='${task.ps1}'`), endIdx);
      if (typedIdx < 0 || endIdx < 0) {
        tasks.delete(taskId);
        return { status: "error", message: "PARSE_ERR: prompt markers not found" };
      }
      tasks.delete(taskId);
      return {
        status: "completed",
        output: allLines.slice(typedIdx + 1, endIdx).join("\n").trim(),
      };
    }

    if (out !== prev) {
      prev = out;
      lastChange = Date.now();
    } else if (Date.now() - lastChange > MAX_STALE_MS) {
      return {
        status: "running",
        task_id: taskId,
        message: "命令仍在执行中",
        output: out,
      };
    }

    spawnSync("sleep", [String(POLL_MS / 1000)], { stdio: "ignore" });
  }
}

// ── Tool definition ────────────────────────────────────────────

export default tool({
  description: `Execute shell commands via tmux with background task support.
  使用 exec 工具运行 shell 命令：
  - "command": 要执行的命令（省略时返回后台任务列表）
  - "directory": 工作目录（可选）
  - "server": SSH 远程主机，如 user@host（可选）
  - "task_id": 查询后台任务结果（与 command 互斥）

  返回 JSON: { status: "completed"|"running"|"error", output?, task_id?, message? }
  命令超过 2 分钟无变化自动转入后台，可用 task_id 查询。
  人类 attach: tmux -S /tmp/yse-tmux/yse-<sessionID>.sock attach`,
  args: {
    command: tool.schema.string().optional().describe("要执行的 shell 命令"),
    directory: tool.schema.string().optional().describe("工作目录"),
    server: tool.schema.string().optional().describe("SSH 远程主机 (user@host)"),
    task_id: tool.schema.string().optional().describe("后台任务 ID"),
  },

  async execute(args, context) {
    const sid = sanitize(context.sessionID || "default");
    const sock = `${SOCKET_DIR}/yse-${sid}.sock`;
    const dir = args.directory || context.directory || context.worktree;
    const server = args.server;

    let controlPath: string | undefined;
    if (server) {
      controlPath = ensureSSHMaster(server, sid);
    }

    ensureSession(sock, server, dir, controlPath);

    if (args.task_id) {
      const result = queryTask(sock, args.task_id, server, {
        controlPath,
        signal: context.abort,
      });
      return JSON.stringify(result);
    }

    if (!args.command) {
      if (tasks.size === 0) return "暂无后台任务";
      const list = Array.from(tasks.values()).map((t) => ({
        task_id: t.id,
        command: t.cmd,
        elapsed_ms: Date.now() - t.startTime,
      }));
      return JSON.stringify(list);
    }

    const result = executeCommand(sock, args.command.trim(), dir, server, {
      controlPath,
      signal: context.abort,
    });
    return JSON.stringify(result);
  },
});
