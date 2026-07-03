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

  async function stopPolling() {
    try {
      await api.stopPolling();
      polling.value = false;
      connected.value = false;
    } catch (e) {
      console.error("stopPolling failed:", e);
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
    stopPolling,
  };
});
