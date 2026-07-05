import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Message, PluginConfig, YseConfig, LogEntry } from "@/api/commands";
import * as api from "@/api/commands";

export const useYseStore = defineStore("yse", () => {
  // --- State ---
  const messages = ref<Message[]>([]);
  const plugins = ref<PluginConfig[]>([]);
  const config = ref<YseConfig | null>(null);
  const logs = ref<LogEntry[]>([]);
  const connected = ref(false);
  const polling = ref(false);

  // --- Getters ---
  const sortedMessages = computed(() =>
    [...messages.value].sort((a, b) => a.timestamp - b.timestamp),
  );

  // --- Actions ---
  async function loadMessages() {
    try {
      messages.value = await api.getMessages(100);
    } catch (e) {
      console.error("loadMessages failed:", e);
    }
  }

  async function loadPlugins() {
    try {
      plugins.value = await api.listPlugins();
    } catch (e) {
      console.error("loadPlugins failed:", e);
    }
  }

  async function loadConfig() {
    try {
      config.value = await api.getConfig();
    } catch (e) {
      console.error("loadConfig failed:", e);
    }
  }

  async function saveConfigAndApply(cfg: YseConfig) {
    await api.saveConfig(cfg);
    config.value = cfg;
  }

  async function loadLogs() {
    try {
      logs.value = await api.getLogs(200);
    } catch (e) {
      console.error("loadLogs failed:", e);
    }
  }

  async function sendMessage(to: string, text: string) {
    await api.sendMessage(to, text);
    await loadMessages();
  }

  async function togglePlugin(id: string, enable: boolean) {
    if (enable) {
      await api.startPlugin(id);
    } else {
      await api.stopPlugin(id);
    }
    await loadPlugins();
  }

  async function startPolling() {
    try {
      await api.startPolling();
      polling.value = true;
      connected.value = true;
    } catch (e) {
      console.error("startPolling failed:", e);
    }
  }

  async function initializeApp() {
    // Auto-start plugins and IMAP polling on Tauri's permanent runtime.
    // This is called from App.vue onMounted, after the Tauri app is fully
    // initialized and the permanent tokio runtime is active. Spawned tasks
    // (IMAP loop, plugin stdout readers) will NOT be cancelled.
    await api.autoStartPlugins().catch((e) => console.error("autoStartPlugins failed:", e));
    await startPolling();
  }

  async function stopPolling() {
    try {
      await api.stopPolling();
      polling.value = false;
      connected.value = false;
    } catch (e) {
      console.error("stopPolling failed:", e);
    }
  }

  let unlistenLogs: (() => void) | null = null;
  let unlistenMessages: (() => void) | null = null;
  let messageReloadTimer: ReturnType<typeof setTimeout> | null = null;

  async function listenForLogs() {
    // Clean up previous listener
    if (unlistenLogs) unlistenLogs();

    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenLogs = await listen<LogEntry>("log-entry", (event) => {
        logs.value.push(event.payload);
        // Keep last 500 entries
        if (logs.value.length > 500) {
          logs.value = logs.value.slice(logs.value.length - 500);
        }
      });
    } catch {
      // Not in Tauri environment
    }
  }

  async function listenForMessages() {
    if (unlistenMessages) unlistenMessages();
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenMessages = await listen("new-message", () => {
        if (messageReloadTimer) clearTimeout(messageReloadTimer);
        messageReloadTimer = setTimeout(loadMessages, 500);
      });
    } catch {
      // Not in Tauri environment
    }
  }

  return {
    messages,
    plugins,
    config,
    logs,
    connected,
    polling,
    sortedMessages,
    loadMessages,
    loadPlugins,
    loadConfig,
    saveConfigAndApply,
    loadLogs,
    sendMessage,
    togglePlugin,
    startPolling,
    initializeApp,
    stopPolling,
    listenForLogs,
    listenForMessages,
  };
});
