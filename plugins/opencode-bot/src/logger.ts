import * as fs from "fs";
import * as path from "path";

let logFile: string | null = null;

export function setLogFile(filePath: string) {
  logFile = filePath;
  fs.mkdirSync(path.dirname(logFile), { recursive: true });
  log(`log file: ${logFile}`);
}

export function log(message: string) {
  const ts = new Date().toISOString();
  const formatted = `[${ts}] [opencode-bot] ${message}\n`;
  process.stderr.write(formatted);
  if (logFile) {
    try {
      fs.appendFileSync(logFile, formatted);
    } catch {}
  }
}

export function logToFile(message: string) {
  if (logFile) {
    const ts = new Date().toISOString();
    const formatted = `[${ts}] [opencode-bot] ${message}\n`;
    try {
      fs.appendFileSync(logFile, formatted);
    } catch {}
  }
}
