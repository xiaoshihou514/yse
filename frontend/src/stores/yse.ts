import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Message, PluginConfig, YseConfig, LogEntry, ProcessInfo, SessionInfo } from "@/api/commands";
import * as api from "@/api/commands";
import { platform } from "@tauri-apps/plugin-os";

function generateId(): string {
  return Math.random().toString(16).slice(2, 10);
}

function deviceModel(): string {
  // Android WebView userAgent: "24069RA21C Build/AP2A.240805.005" → "24069RA21C"
  const m = navigator.userAgent.match(/Android\s+\d+(?:\.\d+)*;\s*([^;)]+)/);
  if (!m) return "";
  return m[1].split("/")[0].trim();
}

function resolveHostname(backendHostname: string): string {
  const h = backendHostname || localStorage.getItem("yse-hostname") || "";
  // On Android the kernel hostname is always "localhost" — use device model
  // or a persistent generated identifier instead.
  if (h && h !== "localhost") return h;
  const stored = localStorage.getItem("yse-hostname");
  if (stored) return stored;
  const p = platform();
  const model = deviceModel();
  if (model) {
    const safe = model.replace(/\s+/g, "-");
    localStorage.setItem("yse-hostname", safe);
    return safe;
  }
  const suffix = generateId();
  const name = `${p}-${suffix}`;
  localStorage.setItem("yse-hostname", name);
  return name;
}

export interface PendingMessage {
  id: string;
  from: string;
  to: string;
  text: string;
  timestamp: number;
  status: "sending" | "sent" | "failed";
  error?: string;
}

export const useYseStore = defineStore("yse", () => {
  const messages = ref<Message[]>([]);
  const pendingMessages = ref<PendingMessage[]>([]);
  const plugins = ref<PluginConfig[]>([]);
  const config = ref<YseConfig | null>(null);
  const logs = ref<LogEntry[]>([]);
  const connected = ref(false);
  const polling = ref(false);
  const hostnames = ref<string[]>([]);
  const selectedHostname = ref("");
  const hiddenAddresses = ref<Set<string>>(new Set());
  const processes = ref<ProcessInfo[]>([]);
  const sessions = ref<SessionInfo[]>([]);
  const localHostname = ref("");

  const sortedMessages = computed(() =>
    [...messages.value].sort((a, b) => a.timestamp - b.timestamp),
  );

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

  async function sendMessage(to: string, text: string, meta?: Record<string, unknown>) {
    const own = config.value?.own_address ?? "";
    const pending: PendingMessage = {
      id: "pending_" + generateId(),
      from: own,
      to,
      text,
      timestamp: Date.now(),
      status: "sending",
    };
    pendingMessages.value.push(pending);
    try {
      await api.sendMessage(to, text, undefined, meta);
      (pending as any).status = "sent";
      // Refresh real messages after a short delay to pick up the server-side copy
      setTimeout(loadMessages, 300);
    } catch (e) {
      (pending as any).status = "failed";
      (pending as any).error = String(e);
    }
    // Remove from pending after a short delay if sent
    if ((pending as any).status === "sent") {
      setTimeout(() => {
        pendingMessages.value = pendingMessages.value.filter((p) => p.id !== pending.id);
      }, 1000);
    }
  }

  async function handlePluginResponse(to: string, componentId: string, value: string) {
    await sendMessage(to, `[${componentId}] ${value}`, {
      plugin: { response: { component_id: componentId, value } },
    });
  }

  async function retryMessage(pending: PendingMessage) {
    (pending as any).status = "sending";
    (pending as any).error = undefined;
    try {
      await api.sendMessage(pending.to, pending.text);
      (pending as any).status = "sent";
      setTimeout(loadMessages, 300);
    } catch (e) {
      (pending as any).status = "failed";
      (pending as any).error = String(e);
    }
    if ((pending as any).status === "sent") {
      setTimeout(() => {
        pendingMessages.value = pendingMessages.value.filter((p) => p.id !== pending.id);
      }, 1000);
    }
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
    // Load configs and contact hashes, but don't auto-start plugins.
    // Plugins are started on demand by SessionRegistry when messages arrive.
    await api.autoStartPlugins().catch((e) => console.error("autoStartPlugins failed:", e));
    await startPolling();
    await loadHostnames();
    await loadHiddenAddresses();
    await loadLocalHostname();
    await loadProcesses();
    await loadSessions();
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

  async function loadHostnames() {
    try {
      hostnames.value = await api.getKnownHostnames();
    } catch (e) {
      console.error("loadHostnames failed:", e);
    }
  }

  async function loadHiddenAddresses() {
    try {
      const addrs = await api.getHiddenAddresses();
      hiddenAddresses.value = new Set(addrs);
    } catch (e) {
      console.error("loadHiddenAddresses failed:", e);
    }
  }

  async function loadLocalHostname() {
    try {
      const backend = await api.getHostname();
      localHostname.value = resolveHostname(backend);
    } catch (e) {
      localHostname.value = resolveHostname("");
      console.error("loadLocalHostname failed:", e);
    }
  }

  async function loadProcesses() {
    try {
      processes.value = await api.listProcesses();
    } catch (e) {
      console.error("loadProcesses failed:", e);
    }
  }

  async function loadSessions() {
    try {
      sessions.value = await api.listSessions();
    } catch (e) {
      console.error("loadSessions failed:", e);
    }
  }

  async function deleteConversation(address: string) {
    await api.deleteConversation(address);
    messages.value = messages.value.filter(
      (m) => m.from !== address && m.to !== address,
    );
  }

  async function toggleHide(address: string) {
    const isHidden = hiddenAddresses.value.has(address);
    await api.toggleHideConversation(address, !isHidden);
    if (isHidden) {
      hiddenAddresses.value.delete(address);
    } else {
      hiddenAddresses.value.add(address);
    }
    // Force reactivity
    hiddenAddresses.value = new Set(hiddenAddresses.value);
  }

  /** Incrementally update hostnames from a new message */
  function ingestHostnamesFromMessage(msg: Message) {
    const extract = (addr: string) => {
      const idx = addr.lastIndexOf("@");
      return idx >= 0 ? addr.slice(idx + 1) : null;
    };
    for (const addr of [msg.from, msg.to]) {
      const h = extract(addr);
      if (h && !hostnames.value.includes(h)) {
        hostnames.value.push(h);
      }
    }
  }

  let unlistenLogs: (() => void) | null = null;
  let unlistenMessages: (() => void) | null = null;
  let messageReloadTimer: ReturnType<typeof setTimeout> | null = null;

  async function listenForLogs() {
    if (unlistenLogs) unlistenLogs();
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenLogs = await listen<LogEntry>("log-entry", (event) => {
        logs.value.push(event.payload);
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
      unlistenMessages = await listen<Message>("new-message", (event) => {
        // Incremental hostname update
        ingestHostnamesFromMessage(event.payload);
        if (messageReloadTimer) clearTimeout(messageReloadTimer);
        messageReloadTimer = setTimeout(loadMessages, 500);
      });
    } catch {
      // Not in Tauri environment
    }
  }

  return {
    messages, pendingMessages, plugins, config, logs, connected, polling,
    hostnames, selectedHostname, hiddenAddresses, processes, sessions, localHostname,
    sortedMessages,
    loadMessages, loadPlugins, loadConfig, saveConfigAndApply, loadLogs,
    sendMessage, handlePluginResponse, retryMessage,
    togglePlugin, startPolling, initializeApp, stopPolling,
    listenForLogs, listenForMessages,
    loadHostnames, loadHiddenAddresses, loadLocalHostname,
    loadProcesses, loadSessions, toggleHide, deleteConversation,
  };
});
