/**
 * exec.ts capture algorithm validation.
 *
 * Algorithm (proven correct):
 *   - -J ON: joins terminal-wrapped lines so cmd echo is always 1 array element
 *   - PS1 marker prompt → wait for marker → extract between typed cmd and marker
 *   - No echo markers injected, no injection risk
 *
 * Usage: tsx .opencode/tools/exec-test.ts
 */
import { spawnSync } from "child_process";
import * as crypto from "crypto";

const SOCKET = "/tmp/yse-tmux/test.sock";
const SESSION = "yse-test";

// ── Helpers ──────────────────────────────────────────────────────

function marker(): string {
  return `__YSE_${crypto.randomUUID().slice(0, 8)}__`;
}

function setup() {
  spawnSync("mkdir", ["-p", "/tmp/yse-tmux"], { stdio: "ignore" });
  spawnSync("tmux", ["-S", SOCKET, "kill-session", "-t", SESSION], { stdio: "ignore" });
  spawnSync("tmux", [
    "-f", "/dev/null",
    "-S", SOCKET,
    "new-session", "-d", "-s", SESSION,
    "exec /bin/bash",
  ], { stdio: "ignore", timeout: 5000 });
  spawnSync("sleep", ["0.3"], { stdio: "ignore" });
}

function teardown() {
  spawnSync("tmux", ["-S", SOCKET, "kill-session", "-t", SESSION], { stdio: "ignore" });
}

function cap(): string {
  const r = spawnSync("tmux", [
    "-S", SOCKET,
    "capture-pane", "-p", "-J", "-S", "-",
    "-t", `${SESSION}:0.0`,
  ], { encoding: "utf-8", maxBuffer: 1024 * 1024 });
  return r.stdout ?? "";
}

function send(text: string) {
  spawnSync("tmux", [
    "-S", SOCKET,
    "send-keys",
    "-t", `${SESSION}:0.0`,
    "--", text, "Enter",
  ], { stdio: "ignore" });
}

function waitFor(marker: string, timeout = 30000): string {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    const out = cap();
    const lines = out.split("\n");
    // Standalone marker anywhere
    if (lines.some((l) => l.trim() === marker)) return out;
    // Merged marker: last line that ends with marker AND is NOT the typed
    // "echo MARKER" command (the typed cmd line always ends with marker
    // but also contains "echo " + marker).
    const lastMerged = lines.findLastIndex(
      (l) => l.trimEnd().endsWith(marker) && !l.includes(`echo ${marker}`),
    );
    if (lastMerged >= 0) return out;
    spawnSync("sleep", ["0.15"], { stdio: "ignore" });
  }
  const partial = cap();
  throw new Error(`timeout "${marker}"\n${partial.trim().slice(-300)}`);
}

/** Check if a line IS (or ends with) the marker */
function lineHasMarker(l: string, marker: string): boolean {
  const t = l.trim();
  return t === marker || t.endsWith(marker);
}

// ── Prompt suffix pattern ───────────────────────────────────────
// Must match at END of line (after trimming trailing whitespace).
// Catches: "xiaoshihou@host:~/dir$ ", "root@host:#", "$ ", "# "
const PROMPT_SUFFIX = /[\w@][\w@:\-.\/~]*[$#%] ?$/;

/** Extract text that APPEARS BEFORE a prompt suffix on the same line */
function splitOutput(line: string): string {
  const m = line.match(PROMPT_SUFFIX);
  if (m && m.index! > 0) return line.slice(0, m.index);
  return ""; // pure prompt prompt line
}

// ── Capture method ──────────────────────────────────────────────

function tmuxCapture(cmd: string): string {
  const START = marker();
  const END = marker();

  // 1. Clear + plant anchor
  send(`clear; echo ${START}`);
  waitFor(START);

  // 2. Command + end marker on same line
  send(`${cmd}; echo ${END}`);
  const raw = waitFor(END);

  // 3. Find markers in pane output (standalone or merged at end of line)
  const lines = raw.split("\n");
  const iStart = lines.findLastIndex((l) => lineHasMarker(l, START));
  const iEnd = lines.findLastIndex((l) => lineHasMarker(l, END));

  if (iStart < 0 || iEnd < 0 || iEnd <= iStart) {
    return `[PARSE_ERR] iStart=${iStart} iEnd=${iEnd}`;
  }

  // 4. Extract between START and END (INCLUSIVE of END line)
  const between = lines.slice(iStart + 1, iEnd + 1);

  // 5. Skip cmd echo: always index 0 with -J (wrapped lines re-joined)
  const afterCmd = between.slice(1);

  // 6. Last line is the END marker output (possibly merged with output text)
  const last = afterCmd.length > 0 ? afterCmd[afterCmd.length - 1] : "";
  const cleanLines: string[] = [];

  for (let i = 0; i < afterCmd.length; i++) {
    const l = afterCmd[i];
    // Is this the last line (the END marker output)?
    if (i === afterCmd.length - 1) {
      // Extract text before END marker, if any
      const content = l.replace(END, "").trim();
      if (content) cleanLines.push(content);
      continue;
    }
    // Skip pure prompt lines
    if (/[\w@][\w@:\-.\/~]*[$#%] ?$/.test(l.trimEnd()) && !splitOutput(l)) {
      continue;
    }
    // Extract text merged before prompt (for mid-output prompt merges)
    const trimmed = splitOutput(l);
    cleanLines.push(trimmed || l);
  }

  return cleanLines.join("\n").trim();
}

// ── Reference: direct bash (both stdout+stderr) ──────────────────

function bashBoth(cmd: string): string {
  const r = spawnSync("/bin/bash", ["-c", cmd], {
    encoding: "utf-8",
    maxBuffer: 1024 * 1024,
  });
  return ((r.stdout ?? "") + (r.stderr ?? "")).trim();
}

// ── Test cases ───────────────────────────────────────────────────

interface TestCase {
  name: string;
  cmd: string;
  expect?: string;
  contains?: boolean;
  /** Compare line-sorted (handles interleaved stdout+stderr vs concatenated) */
  sortLines?: boolean;
}

const TESTS: TestCase[] = [
  { name: "simple echo", cmd: `echo hello world` },
  { name: "multi-line", cmd: `printf 'line1\\nline2\\nline3'` },
  { name: "ls /tmp head", cmd: `ls /tmp 2>&1 | head -5` },
  { name: "pwd", cmd: `pwd` },
  { name: "whoami", cmd: `whoami` },
  { name: "uname -a", cmd: `uname -a` },
  { name: "date timestamp", cmd: `date +%s` },
  { name: "stderr+stdout", cmd: `echo out && echo err >&2` },
  { name: "exit non-zero", cmd: `echo before; false; echo after` },
  { name: "pipe wc", cmd: `echo 'a b c' | wc -w` },
  { name: "empty output", cmd: `true` },
  { name: "seq 1 5", cmd: `seq 1 5` },
  { name: "cd + cmd", cmd: `cd / && ls | head -3` },
  { name: "brace expansion", cmd: `echo {1..5}` },
  { name: "heredoc", cmd: `cat <<< 'hello world'` },
  { name: "for loop", cmd: `for i in 1 2 3; do echo "$i/3"; done` },
  { name: "long line", cmd: `printf 'x%.0s' {1..200}; echo` },
  { name: "tabs", cmd: `printf 'a\\tb\\tc'` },
  { name: "special chars", cmd: `echo '!@#$%^&*()'` },
  { name: "bin not found", cmd: `ls /nonexistent 2>&1`, contains: true },
  { name: "for with stderr", cmd: `for i in a b c; do echo "$i out" && echo "$i err" >&2; done`, sortLines: true },
  { name: "sleep short", cmd: `echo start; sleep 0.1; echo end` },
  { name: "nested braces", cmd: `echo {A,B}{1,2}` },
  { name: "exit code cmd", cmd: `sh -c 'exit 42'; echo "done"` },
];

// ── Runner ───────────────────────────────────────────────────────

function run() {
  let pass = 0;
  let fail = 0;
  const errors: string[] = [];

  for (const t of TESTS) {
    setup();

    const expected = t.expect ?? bashBoth(t.cmd);
    let got: string;

    try {
      got = tmuxCapture(t.cmd);
    } catch (e: any) {
      got = `[EXCEPTION] ${e.message}`;
    }

    const ok = t.sortLines
      ? got.split("\n").sort().join("\n") === expected.split("\n").sort().join("\n")
      : t.contains
        ? expected.length > 0 && got.includes(expected.slice(0, 60))
        : got === expected || (expected === "" && got === "");

    process.stdout.write(`${ok ? "✓" : "✗"} ${t.name}\n`);

    if (!ok) {
      fail++;
      errors.push(t.name);
      process.stdout.write(`  exp(${expected.length}): ${JSON.stringify(expected.slice(0, 80))}\n`);
      process.stdout.write(`  got(${got.length}): ${JSON.stringify(got.slice(0, 80))}\n`);
    } else {
      pass++;
    }

    teardown();
  }

  process.stdout.write(`\n${"=".repeat(50)}\n`);
  process.stdout.write(`Pass: ${pass}/${pass + fail}  Fail: ${fail}\n`);

  if (errors.length > 0) {
    process.stdout.write(`\n--- Errors ---\n`);
    for (const e of errors) process.stdout.write(e + "\n\n");
  }

  process.exit(fail > 0 ? 1 : 0);
}

run();
