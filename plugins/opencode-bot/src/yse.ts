import { createInterface } from "readline";

export interface YseMessage {
  method: string;
  params: {
    from: string;
    to: string;
    text?: string;
    meta?: { plugin?: { response?: { value: string } } };
  };
}

export function parseStdin(): AsyncGenerator<YseMessage> {
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
  const iter = rl[Symbol.asyncIterator]();
  const generator = async function* () {
    for await (const line of iter) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      try {
        const msg: YseMessage = JSON.parse(trimmed);
        yield msg;
      } catch {
        process.stderr.write(`[opencode-bot] invalid JSON: ${trimmed}\n`);
      }
    }
  };
  return generator();
}

export function sendResponse(to: string, text: string): void {
  const out = JSON.stringify({
    method: "send",
    params: { from: "opencode@yse", to, text },
  });
  process.stdout.write(out + "\n");
}

export function sendList(
  to: string,
  text: string,
  title: string,
  options: { label: string; value: string; description?: string }[],
): void {
  const out = JSON.stringify({
    method: "send",
    params: {
      from: "opencode@yse",
      to,
      text,
      meta: {
        plugin: {
          component: {
            type: "list",
            title,
            options,
          },
        },
      },
    },
  });
  process.stdout.write(out + "\n");
}

export function sendLog(level: string, msg: string): void {
  const out = JSON.stringify({
    method: "log",
    params: { level, msg },
  });
  process.stdout.write(out + "\n");
}
