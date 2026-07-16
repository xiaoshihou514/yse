import { describe, it, expect, vi } from "vitest";
import { resolveModelChain } from "./opencode.js";
import type { ModelConfig } from "./opencode.js";

// ---------------------------------------------------------------------------
// resolveModelChain
// ---------------------------------------------------------------------------

describe("resolveModelChain", () => {
  const defaultConfig: ModelConfig = {
    defaultModel: { modelId: "default-m", providerId: "default-p" },
    fallbackChain: [
      { modelId: "fb1", providerId: "fb1-p" },
      { modelId: "fb2", providerId: "fb2-p" },
    ],
  };

  const emptyConfig: ModelConfig = {
    defaultModel: undefined,
    fallbackChain: [],
  };

  it("manual mode with full spec → single-entry chain", () => {
    const result = resolveModelChain(
      { modelMode: "manual", modelId: "m", providerId: "p", modelVariant: "v1" },
      defaultConfig,
    );
    expect(result).toEqual([{ modelId: "m", providerId: "p", variant: "v1" }]);
  });

  it("manual mode without modelId → falls back to global", () => {
    const result = resolveModelChain(
      { modelMode: "manual", modelId: undefined, providerId: "p" },
      defaultConfig,
    );
    // manual but missing modelId → not a valid manual → global chain
    expect(result).toEqual([
      { modelId: "default-m", providerId: "default-p" },
      { modelId: "fb1", providerId: "fb1-p" },
      { modelId: "fb2", providerId: "fb2-p" },
    ]);
  });

  it("global mode with default + fallbacks → [default, ...fallbacks]", () => {
    const result = resolveModelChain(
      { modelMode: "global" },
      defaultConfig,
    );
    expect(result).toEqual([
      { modelId: "default-m", providerId: "default-p" },
      { modelId: "fb1", providerId: "fb1-p" },
      { modelId: "fb2", providerId: "fb2-p" },
    ]);
  });

  it("global mode with default only → single entry", () => {
    const config: ModelConfig = {
      defaultModel: { modelId: "m", providerId: "p" },
      fallbackChain: [],
    };
    const result = resolveModelChain({ modelMode: "global" }, config);
    expect(result).toEqual([{ modelId: "m", providerId: "p" }]);
  });

  it("global mode with fallbacks only → just fallbacks", () => {
    const config: ModelConfig = {
      defaultModel: undefined,
      fallbackChain: [{ modelId: "fb", providerId: "fb-p" }],
    };
    const result = resolveModelChain({ modelMode: "global" }, config);
    expect(result).toEqual([{ modelId: "fb", providerId: "fb-p" }]);
  });

  it("global mode with nothing → empty-spec chain (server decides)", () => {
    const result = resolveModelChain(
      { modelMode: "global" },
      emptyConfig,
    );
    expect(result).toEqual([{ modelId: "", providerId: "" }]);
  });

  it("modelMode undefined treated as global", () => {
    const result = resolveModelChain(
      {},
      defaultConfig,
    );
    expect(result).toEqual([
      { modelId: "default-m", providerId: "default-p" },
      { modelId: "fb1", providerId: "fb1-p" },
      { modelId: "fb2", providerId: "fb2-p" },
    ]);
  });

  it("does not mutate global config", () => {
    const config: ModelConfig = {
      defaultModel: { modelId: "m", providerId: "p" },
      fallbackChain: [],
    };
    const result = resolveModelChain({ modelMode: "global" }, config);
    expect(result).toHaveLength(1);
    expect(config.fallbackChain).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// State machine integration — simulated user flow
// ---------------------------------------------------------------------------

describe("model config state machine", () => {
  function makeSession(overrides: any = {}) {
    return { mode: "sdk" as const, sessionId: "s1", modelMode: "global" as const, ...overrides };
  }

  function makeState(sessions: any = {}, modelConfig: ModelConfig = { defaultModel: undefined, fallbackChain: [] }) {
    return { sessions, modelConfig };
  }

  it("initial state → global, server picks", () => {
    const state = makeState({ alice: makeSession() });
    const us = state.sessions.alice;
    const chain = resolveModelChain(us, state.modelConfig);
    expect(chain).toEqual([{ modelId: "", providerId: "" }]);
  });

  it("set global default → chain uses default", () => {
    const state = makeState({ alice: makeSession() });
    state.modelConfig.defaultModel = { modelId: "deepseek", providerId: "opencode" };
    const chain = resolveModelChain(state.sessions.alice, state.modelConfig);
    expect(chain).toEqual([{ modelId: "deepseek", providerId: "opencode" }]);
  });

  it("add fallback → chain includes fallback", () => {
    const state = makeState({ alice: makeSession() });
    state.modelConfig.defaultModel = { modelId: "deepseek", providerId: "opencode" };
    state.modelConfig.fallbackChain.push({ modelId: "gpt4", providerId: "openai" });
    const chain = resolveModelChain(state.sessions.alice, state.modelConfig);
    expect(chain).toEqual([
      { modelId: "deepseek", providerId: "opencode" },
      { modelId: "gpt4", providerId: "openai" },
    ]);
  });

  it("session set manual → overrides global", () => {
    const state = makeState({ alice: makeSession() });
    state.modelConfig.defaultModel = { modelId: "deepseek", providerId: "opencode" };
    state.modelConfig.fallbackChain.push({ modelId: "gpt4", providerId: "openai" });

    const us = state.sessions.alice;
    us.modelMode = "manual";
    us.modelId = "claude";
    us.providerId = "anthropic";

    const chain = resolveModelChain(us, state.modelConfig);
    expect(chain).toEqual([{ modelId: "claude", providerId: "anthropic" }]);
  });

  it("session override clear → back to global", () => {
    const state = makeState({ alice: makeSession() });
    state.modelConfig.defaultModel = { modelId: "deepseek", providerId: "opencode" };

    const us = state.sessions.alice;
    us.modelMode = "manual";
    us.modelId = "claude";
    us.providerId = "anthropic";

    // Clear override
    us.modelMode = "global";
    delete us.modelId;
    delete us.providerId;

    const chain = resolveModelChain(us, state.modelConfig);
    expect(chain).toEqual([{ modelId: "deepseek", providerId: "opencode" }]);
  });

  it("fallback chain remove nth item", () => {
    const chain = [
      { modelId: "a", providerId: "p" },
      { modelId: "b", providerId: "p" },
      { modelId: "c", providerId: "p" },
    ];
    chain.splice(1, 1); // remove index 1 (second item)
    expect(chain).toEqual([
      { modelId: "a", providerId: "p" },
      { modelId: "c", providerId: "p" },
    ]);
  });

  it("multiple sessions — independent modelModes", () => {
    const state = makeState({
      alice: makeSession({ modelMode: "global" }),
      bob: makeSession({ modelMode: "manual", modelId: "claude", providerId: "anthropic" }),
    });
    state.modelConfig.defaultModel = { modelId: "deepseek", providerId: "opencode" };

    const aliceChain = resolveModelChain(state.sessions.alice, state.modelConfig);
    expect(aliceChain).toEqual([{ modelId: "deepseek", providerId: "opencode" }]);

    const bobChain = resolveModelChain(state.sessions.bob, state.modelConfig);
    expect(bobChain).toEqual([{ modelId: "claude", providerId: "anthropic" }]);
  });

  it("serialization roundtrip", () => {
    const state = makeState({
      alice: makeSession({ modelMode: "manual", modelId: "m", providerId: "p" }),
    });
    state.modelConfig.defaultModel = { modelId: "d", providerId: "dp" };
    state.modelConfig.fallbackChain.push({ modelId: "fb", providerId: "fp" });

    const serialized = JSON.parse(JSON.stringify(state));
    expect(serialized.modelConfig.defaultModel).toEqual({ modelId: "d", providerId: "dp" });
    expect(serialized.modelConfig.fallbackChain).toEqual([{ modelId: "fb", providerId: "fp" }]);
    expect(serialized.sessions.alice.modelMode).toBe("manual");
  });

  it("clear default model", () => {
    const state = makeState({ alice: makeSession() });
    state.modelConfig.defaultModel = { modelId: "d", providerId: "dp" };
    state.modelConfig.defaultModel = undefined;
    const chain = resolveModelChain(state.sessions.alice, state.modelConfig);
    expect(chain).toEqual([{ modelId: "", providerId: "" }]);
  });

  it("clear fallback chain", () => {
    const state = makeState({ alice: makeSession() });
    state.modelConfig.fallbackChain.push({ modelId: "fb", providerId: "fp" });
    state.modelConfig.fallbackChain = [];
    const chain = resolveModelChain(state.sessions.alice, state.modelConfig);
    expect(chain).toEqual([{ modelId: "", providerId: "" }]);
  });
});
