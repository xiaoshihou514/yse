import { tool } from "@opencode-ai/plugin";
import { execSync } from "child_process";

const INSTANT = new Set([
  "cd", "pwd", "ls", "eza", "tree", "grep", "rg",
  "cat", "head", "tail", "wc", "sort", "uniq",
  "echo", "printf", "which", "type", "env", "export",
  "true", "false", "exit", "source", ".",
]);

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

    const session = `yse-${Math.random().toString(36).slice(2, 6)}`;
    const socket = "/tmp/yse-tmux/yse.sock";
    execSync(`mkdir -p /tmp/yse-tmux`, { stdio: "ignore" });
    execSync(`tmux -f /dev/null -S ${socket} new-session -d -s ${session}`, { stdio: "ignore" });
    execSync(`tmux -S ${socket} send-keys -t ${session}:0.0 -- ${JSON.stringify(cmd)} Enter`, { stdio: "ignore" });
    await new Promise((r) => setTimeout(r, 1000));

    while (true) {
      await new Promise((r) => setTimeout(r, 2000));
      const panePid = execSync(
        `tmux -S ${socket} display-message -p -t ${session}:0.0 '#{pane_pid}'`,
        { encoding: "utf-8" },
      ).toString().trim();
      const children = execSync(
        `ps --ppid ${panePid} --no-headers 2>/dev/null`,
        { encoding: "utf-8" },
      ).toString().trim();
      const output = execSync(
        `tmux -S ${socket} capture-pane -p -J -t ${session}:0.0 -S -1000`,
        { encoding: "utf-8", maxBuffer: 1024 * 1024 },
      ).toString();
      if (!children) {
        execSync(`tmux -S ${socket} kill-session -t ${session}`, { stdio: "ignore" });
        return output.trim();
      }
    }
  },
});
