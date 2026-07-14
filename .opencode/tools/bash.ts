import { tool } from "@opencode-ai/plugin";
import { execSync } from "child_process";

const INSTANT = new Set([
  "cd", "pwd", "ls", "eza", "tree", "grep", "rg",
  "cat", "head", "tail", "wc", "sort", "uniq",
  "echo", "printf", "which", "type", "env", "export",
  "true", "false", "exit", "source", ".",
]);
const SOCKET = process.env.YSE_TMUX_SOCK || "/tmp/yse-tmux/yse.sock";
const SESSION = "yse";

function ensureSession() {
  execSync(`mkdir -p /tmp/yse-tmux`, { stdio: "ignore" });
  execSync(`tmux -f /dev/null -S ${SOCKET} new-session -d -s ${SESSION} 'exec /bin/bash' 2>/dev/null || true`, { stdio: "ignore" });
}

export default tool({
  description: "Execute shell commands.",
  args: {
    command: tool.schema.string().describe("Shell command to execute"),
  },
  async execute(args) {
    const cmd = args.command.trim();
    const first = cmd.split(/\s+/)[0];

    if (INSTANT.has(first)) {
      return execSync(cmd, { encoding: "utf-8", maxBuffer: 1024 * 1024 }).toString();
    }

    ensureSession();
    const name = cmd.slice(0, 50).replace(/[^a-zA-Z0-9 _\/\.-]/g, "").trim() || "bash";
    const windowId = execSync(
      `tmux -S ${SOCKET} new-window -P -F "#{window_id}" -t ${SESSION} -n "${name}" 'exec /bin/bash'`,
      { encoding: "utf-8" },
    ).toString().trim();
    execSync(`tmux -S ${SOCKET} send-keys -t ${windowId} -- ${JSON.stringify(cmd)} Enter`, { stdio: "ignore" });
    await new Promise((r) => setTimeout(r, 1000));

    while (true) {
      await new Promise((r) => setTimeout(r, 2000));
      const panePid = execSync(
        `tmux -S ${SOCKET} display-message -p -t ${windowId} '#{pane_pid}'`,
        { encoding: "utf-8" },
      ).toString().trim();
      const children = execSync(
        `ps --ppid ${panePid} --no-headers 2>/dev/null`,
        { encoding: "utf-8" },
      ).toString().trim();
      const output = execSync(
        `tmux -S ${SOCKET} capture-pane -p -J -t ${windowId} -S -1000`,
        { encoding: "utf-8", maxBuffer: 1024 * 1024 },
      ).toString();
      if (!children) {
        return output.trim();
      }
    }
  },
});
