/**
 * Test exec.ts SSH support against localhost.
 * Verifies remote execution produces the same output as local.
 *
 * Usage: tsx .opencode/tools/exec-ssh-test.ts
 */
import { spawnSync } from "child_process";
import * as crypto from "crypto";

const SERVER = process.env.TEST_SSH_SERVER || "localhost";
const SOCKET_DIR = "/tmp/yse-tmux-ssh-test";

function marker(): string {
  return `__YSE_${crypto.randomUUID().slice(0, 8)}__`;
}

function sanitize(s: string): string {
  return s.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 64);
}

// Replicate the EXACT tmux logic from bash.ts, but with SSH
function tmuxProc(args: string[], server?: string) {
  const [cmd, cmdArgs] = server
    ? ["ssh", ["-o", "StrictHostKeyChecking=no", server, ["tmux", ...args].map(shQuote).join(" ")] as string[]]
    : ["tmux", args as string[]];
  return spawnSync(cmd, cmdArgs, {
    encoding: "utf-8",
    maxBuffer: 1024 * 1024,
    timeout: 10000,
  });
}

function send(sock: string, text: string, server?: string) {
  tmuxProc(
    ["-S", sock, "send-keys", "-t", "yse:0.0", "--", text, "Enter"],
    server,
  );
}

function capture(sock: string, server?: string): string {
  return tmuxProc(
    ["-S", sock, "capture-pane", "-p", "-J", "-S", "-", "-t", "yse:0.0"],
    server,
  ).stdout ?? "";
}

function shQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

// ── Test each command both locally and via SSH ──────────────────

const TESTS = [
  { name: "echo", cmd: `echo hello_world_ssh` },
  { name: "pwd", cmd: `pwd` },
  { name: "whoami", cmd: `whoami` },
  { name: "multi-line", cmd: `printf 'line1\\nline2\\nline3'` },
  { name: "pipe", cmd: `echo 'a b c' | wc -w` },
  { name: "stderr", cmd: `echo out && echo err >&2` },
  { name: "env", cmd: `echo $HOME` },
  { name: "sleep", cmd: `echo start; sleep 0.1; echo end` },
  { name: "for loop", cmd: `for i in 1 2 3; do echo "$i"; done` },
];

function run() {
  let pass = 0;
  let fail = 0;

  for (const t of TESTS) {
    const sid = sanitize(t.name);
    const sock = `${SOCKET_DIR}/yse-${sid}.sock`;

    // Cleanup
    tmuxProc(["-S", sock, "kill-session", "-t", "yse"], SERVER);
    spawnSync("rm", ["-f", sock]);

    // Remote setup
    spawnSync("ssh", ["-o", "StrictHostKeyChecking=no", SERVER, `mkdir -p ${shQuote(SOCKET_DIR)}`],
      { stdio: "ignore", timeout: 5000 },
    );
    tmuxProc(
      ["-f", "/dev/null", "-S", sock, "new-session", "-d", "-s", "yse", "exec /bin/bash"],
      SERVER,
    );
    spawnSync("sleep", ["0.3"], { stdio: "ignore" });

    // Run via SSH
    const START = marker();
    const END = marker();
    send(sock, `clear; echo ${START}`, SERVER);

    // Wait for START
    const waitStart = Date.now();
    while (Date.now() - waitStart < 5000) {
      const o = capture(sock, SERVER);
      if (o.includes(START)) break;
      spawnSync("sleep", ["0.15"]);
    }

    send(sock, `${t.cmd}; echo ${END}`, SERVER);

    // Wait for END
    const waitEnd = Date.now();
    while (Date.now() - waitEnd < 10000) {
      const o = capture(sock, SERVER);
      const lines = o.split("\n");
      if (lines.some((l) => l.trim() === END)) break;
      const merged = lines.findLastIndex(
        (l) => l.trimEnd().endsWith(END) && !l.includes(`echo ${END}`),
      );
      if (merged >= 0) break;
      spawnSync("sleep", ["0.15"]);
    }

    const raw = capture(sock, SERVER);

    // Reference: remote (SSH) or local bash
    const ref = SERVER === "localhost"
      ? spawnSync("/bin/bash", ["-c", t.cmd], { encoding: "utf-8" })
      : spawnSync("ssh", ["-o", "StrictHostKeyChecking=no", SERVER, t.cmd], { encoding: "utf-8" });
    const expected = ((ref.stdout ?? "") + (ref.stderr ?? "")).trim();

    // Parse output
    const lines = raw.split("\n");
    const iStart = lines.findLastIndex((l) => l.includes(START));
    const iEnd = lines.findLastIndex((l) => l.includes(END));

    let got = "";
    if (iStart >= 0 && iEnd >= 0 && iEnd > iStart) {
      const between = lines.slice(iStart + 1, iEnd + 1);
      const afterCmd = between.slice(1);
      const promptRe = /[\w@][\w@:\-.\/~]*[$#%] ?$/;
      const clean: string[] = [];
      for (let i = 0; i < afterCmd.length; i++) {
        const l = afterCmd[i];
        if (i === afterCmd.length - 1) {
          const content = l.replace(END, "").trim();
          if (content) clean.push(content);
          continue;
        }
        const m = l.match(promptRe);
        const isPure = m && (m.index === 0 || l.trimEnd() === m[0]);
        if (isPure) continue;
        clean.push(m && m.index! > 0 ? l.slice(0, m.index) : l);
      }
      got = clean.join("\n").trim();
    } else {
      got = `[PARSE_ERR] iStart=${iStart} iEnd=${iEnd}`;
    }

    const ok = got === expected || (expected === "" && got === "");
    process.stdout.write(`${ok ? "✓" : "✗"} ${t.name}\n`);
    if (!ok) {
      fail++;
      process.stdout.write(`  exp: ${JSON.stringify(expected)}\n  got: ${JSON.stringify(got)}\n`);
    } else {
      pass++;
    }

    // Cleanup
    tmuxProc(["-S", sock, "kill-session", "-t", "yse"], SERVER);
    spawnSync("rm", ["-f", sock]);
  }

  process.stdout.write(`\nSSH: ${pass}/${pass + fail}  Fail: ${fail}\n`);
  process.exit(fail > 0 ? 1 : 0);
}

run();
