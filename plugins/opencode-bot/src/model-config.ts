import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { parse, stringify } from "smol-toml";
import type { ModelConfig } from "./opencode.js";

function configPath(): string {
  const base = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  return path.join(base, "yse", "opencode.toml");
}

export function loadModelConfig(): ModelConfig {
  const fp = configPath();
  try {
    const raw = fs.readFileSync(fp, "utf-8");
    const parsed = parse(raw) as Record<string, any>;
    const defaultModel = parsed.default
      ? { modelId: parsed.default.modelId ?? "", providerId: parsed.default.providerId ?? "", variant: parsed.default.variant }
      : undefined;
    const fallback: Array<{ modelId: string; providerId: string; variant?: string }> = [];
    if (Array.isArray(parsed.fallback)) {
      for (const f of parsed.fallback) {
        if (f.modelId && f.providerId) {
          fallback.push({ modelId: f.modelId, providerId: f.providerId, variant: f.variant });
        }
      }
    }
    return { defaultModel, fallbackChain: fallback };
  } catch {
    return { fallbackChain: [] };
  }
}

export function saveModelConfig(cfg: ModelConfig): void {
  const fp = configPath();
  const data: Record<string, any> = {};
  if (cfg.defaultModel) {
    data.default = { modelId: cfg.defaultModel.modelId, providerId: cfg.defaultModel.providerId };
    if (cfg.defaultModel.variant) data.default.variant = cfg.defaultModel.variant;
  }
  data.fallback = cfg.fallbackChain.map((f) => {
    const entry: Record<string, string> = { modelId: f.modelId, providerId: f.providerId };
    if (f.variant) entry.variant = f.variant;
    return entry;
  });
  const dir = path.dirname(fp);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(fp, stringify(data));
}

export { configPath as modelConfigPath };