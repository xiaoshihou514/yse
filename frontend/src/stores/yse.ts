import { defineStore } from "pinia";
import { ref, reactive, computed } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import type {
  Message,
  PluginConfig,
  YseConfig,
  ProcessInfo,
  SessionInfo,
} from "@/api/commands";
import * as api from "@/api/commands";
import { platform } from "@tauri-apps/plugin-os";
import { error, LogLevel } from "@tauri-apps/plugin-log";
import { hostnameFromAddr, nameFromAddr } from "@/utils/address";

function generateId(): string {
  return Math.random().toString(16).slice(2, 10);
}

function deviceModel(): string {
  // Android WebView userAgent: "...; 24069RA21C Build/AP2A.240805.005; ..." → "24069RA21C"
  const m = navigator.userAgent.match(
    /Android\s+\d+(?:\.\d+)?;\s*([^\s;)\/]+)/,
  );
  if (!m) return "";
  return m[1];
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

async function withLog<T>(
  label: string,
  fn: () => Promise<T>,
): Promise<T | undefined> {
  try {
    return await fn();
  } catch (e) {
    error(`${label} failed: ${String(e)}`);
  }
}

export const useYseStore = defineStore("yse", () => {
  const messages = ref<Message[]>([]);
  const pendingMessages = ref<PendingMessage[]>([]);
  const plugins = ref<PluginConfig[]>([]);
  const config = ref<YseConfig | null>(null);
  interface LogEntry {
    level: string;
    message: string;
    timestamp: number;
  }

  const logs = ref<LogEntry[]>([]);
  const connected = ref(false);
  const polling = ref(false);
  const hostnames = ref<string[]>([]);
  const selectedHostname = ref("");
  const hiddenAddresses = ref<Set<string>>(new Set());
  const processes = ref<ProcessInfo[]>([]);
  const sessions = ref<SessionInfo[]>([]);
  const localHostname = ref("");
  const _rt = JSON.parse(localStorage.getItem("yse-read-timestamps") || "{}");
  for (const k of Object.keys(_rt)) if (_rt[k] > 1e11) _rt[k] = Math.floor(_rt[k] / 1000);
  const readTimestamps = reactive<Record<string, number>>(_rt);
  const readVersion = ref(0);

  function markRead(addr: string) {
    readTimestamps[addr] = Math.floor(Date.now() / 1000);
    readVersion.value++;
    localStorage.setItem("yse-read-timestamps", JSON.stringify(readTimestamps));
  }

  function markAllRead() {
    const now = Math.floor(Date.now() / 1000);
    for (const m of messages.value) {
      if (m.from.includes("#")) readTimestamps[m.from] = now;
      if (m.to.includes("#")) readTimestamps[m.to] = now;
    }
    readVersion.value++;
    localStorage.setItem("yse-read-timestamps", JSON.stringify(readTimestamps));
  }

  const INITIAL_MSG_COUNT = 100;
  const MSG_PAGE_SIZE = 50;
  const messagesOffset = ref(0);
  const loadingMore = ref(false);

  const sortedMessages = computed(() =>
    [...messages.value].sort((a, b) => a.timestamp - b.timestamp),
  );

  async function loadMessages() {
    try {
      messagesOffset.value = 0;
      messages.value = await api.getMessages(INITIAL_MSG_COUNT, 0);
      messagesOffset.value = messages.value.length;
      // Clean up pending "sent" entries that have a matching real message
      pendingMessages.value = pendingMessages.value.filter((p) => {
        if (p.status !== "sent") return true;
        return !messages.value.some(
          (r) =>
            r.text === p.text && Math.abs(r.timestamp - p.timestamp) < 30000,
        );
      });
    } catch (e) {
      error(`loadMessages failed: ${String(e)}`);
    }
  }

  async function loadOlderMessages(): Promise<boolean> {
    if (loadingMore.value) return false;
    loadingMore.value = true;
    try {
      const older = await api.getMessages(MSG_PAGE_SIZE, messagesOffset.value);
      if (older.length === 0) return false;
      messagesOffset.value += older.length;
      messages.value = [...older, ...messages.value];
      return true;
    } catch (e) {
      error(`loadOlderMessages failed: ${String(e)}`);
      return false;
    } finally {
      loadingMore.value = false;
    }
  }

  async function loadPlugins() {
    if (platform() === "android") return;
    plugins.value =
      (await withLog("loadPlugins", () => api.listPlugins())) || plugins.value;
  }

  async function loadConfig() {
    config.value =
      (await withLog("loadConfig", () => api.getConfig())) || config.value;
  }

  async function saveConfigAndApply(cfg: YseConfig) {
    await api.saveConfig(cfg);
    config.value = cfg;
    await stopPolling();
    await startPolling();
  }

  async function renameContactDisplayName(addr: string, newName: string) {
    const cfg = config.value;
    if (!cfg) return;
    const mapping = cfg.plugin_mappings.find((m) => m.virtual_addr === addr);
    if (!mapping) {
      cfg.plugin_mappings.push({
        virtual_addr: addr,
        plugin_id: "",
        display_name: newName,
      });
    } else {
      mapping.display_name = newName;
    }
    await saveConfigAndApply(cfg);
  }

  async function loadLogs() {
    // Logs are fed by tauri-plugin-log Webview target events;
    // the frontend buffer holds up to 500 entries in memory.
  }

  async function sendMessage(
    to: string,
    text: string,
    files?: string[],
    meta?: Record<string, unknown>,
  ) {
    const own = config.value?.own_address ?? "";
    const pending: PendingMessage = {
      id: `pending_${generateId()}`,
      from: own,
      to,
      text,
      timestamp: Date.now(),
      status: "sending",
    };
    pendingMessages.value.push(pending);
    try {
      await api.sendMessage(to, text, files, meta);
      // Remove pending immediately to avoid showing two messages simultaneously
      pendingMessages.value = pendingMessages.value.filter(
        (p) => p.id !== pending.id,
      );
      setTimeout(loadMessages, 200);
    } catch (e) {
      pending.status = "failed";
      pending.error = String(e);
    }
  }

  async function handlePluginResponse(
    to: string,
    componentId: string,
    value: string,
  ) {
    await sendMessage(to, `[${componentId}] ${value}`, undefined, {
      plugin: { response: { component_id: componentId, value } },
    });
  }

  async function retryMessage(pending: PendingMessage) {
    pending.status = "sending";
    pending.error = undefined;
    try {
      await api.sendMessage(pending.to, pending.text);
      pendingMessages.value = pendingMessages.value.filter(
        (p) => p.id !== pending.id,
      );
      setTimeout(loadMessages, 200);
    } catch (e) {
      pending.status = "failed";
      pending.error = String(e);
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
    if (!config.value?.email_username) {
      polling.value = false;
      connected.value = false;
      return;
    }
    try {
      await api.startPolling();
      polling.value = true;
      connected.value = true;
    } catch (e) {
      error(`startPolling failed: ${String(e)}`);
      MessagePlugin.error(`IMAP 连接失败: ${e}`).catch(() => {});
    }
  }

  async function initializeApp() {
    // Plugins/sessions are desktop-only; individual loaders handle mobile skip.
    await api
      .autoStartPlugins()
      .catch((e) => error(`autoStartPlugins failed: ${String(e)}`));
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
      error(`stopPolling failed: ${String(e)}`);
    }
  }

  async function loadHostnames() {
    try {
      hostnames.value = await api.getKnownHostnames();
    } catch (e) {
      error(`loadHostnames failed: ${String(e)}`);
    }
  }

  async function loadHiddenAddresses() {
    try {
      const addrs = await api.getHiddenAddresses();
      hiddenAddresses.value = new Set(addrs);
    } catch (e) {
      error(`loadHiddenAddresses failed: ${String(e)}`);
    }
  }

  async function loadLocalHostname() {
    try {
      const backend = await api.getHostname();
      localHostname.value = resolveHostname(backend);
    } catch (e) {
      localHostname.value = resolveHostname("");
      error(`loadLocalHostname failed: ${String(e)}`);
    }
  }

  async function loadProcesses() {
    if (platform() === "android") return;
    try {
      processes.value = await api.listProcesses();
    } catch (e) {
      error(`loadProcesses failed: ${String(e)}`);
    }
  }

  async function loadSessions() {
    if (platform() === "android") return;
    try {
      sessions.value = await api.listSessions();
    } catch (e) {
      error(`loadSessions failed: ${String(e)}`);
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
    for (const addr of [msg.from, msg.to]) {
      const h = hostnameFromAddr(addr) || null;
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
      unlistenLogs = await listen<{ message: string; level: string }>(
        "log://log",
        (event) => {
          logs.value.push({
            level: (
              LogLevel[event.payload.level as unknown as number] ??
              String(event.payload.level)
            ).toLowerCase(),
            message: event.payload.message,
            timestamp: Date.now(),
          });
          if (logs.value.length > 500) {
            logs.value = logs.value.slice(logs.value.length - 500);
          }
        },
      );
    } catch {
      // Not in Tauri environment
    }
  }

  async function listenForMessages() {
    if (unlistenMessages) unlistenMessages();
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenMessages = await listen<Message>("new-message", (event) => {
        // Remove any pending message that matches this real message right away,
        // before loadMessages runs. Prevents brief double-display when the
        // event arrives before the send() promise resolves.
        const p = event.payload;
        const wasPending = pendingMessages.value.some(
          (x) =>
            x.text === p.text && Math.abs(x.timestamp - p.timestamp) < 30000,
        );
        pendingMessages.value = pendingMessages.value.filter(
          (x) =>
            !(x.text === p.text && Math.abs(x.timestamp - p.timestamp) < 30000),
        );
        // Incremental hostname update
        ingestHostnamesFromMessage(event.payload);
        if (messageReloadTimer) clearTimeout(messageReloadTimer);
        messageReloadTimer = setTimeout(loadMessages, 500);
        // System notification for external messages (not local echo)
        if (!wasPending && nameFromAddr(p.from) !== config.value?.own_address) {
          sendMessageNotification(p).catch(() => {});
        }
      });
    } catch {
      // Not in Tauri environment
    }
  }

  async function sendMessageNotification(msg: Message) {
    try {
      const { sendNotification, isPermissionGranted } =
        await import("@tauri-apps/plugin-notification");
      if (await isPermissionGranted()) {
        sendNotification({
          title: nameFromAddr(msg.from),
          body: msg.text?.slice(0, 200) ?? "(文件)",
        });
      }
    } catch {
      // Not in Tauri environment
    }
  }

  async function requestNotificationPermission() {
    if (platform() !== "android") return;
    try {
      const { isPermissionGranted, requestPermission } =
        await import("@tauri-apps/plugin-notification");
      if (!(await isPermissionGranted())) {
        await requestPermission();
      }
    } catch {
      // Not in Tauri environment
    }
  }

  return {
    messages,
    pendingMessages,
    plugins,
    config,
    logs,
    connected,
    polling,
    hostnames,
    selectedHostname,
    hiddenAddresses,
    processes,
    sessions,
    localHostname,
    readTimestamps,
    readVersion,
    markRead,
    markAllRead,
    sortedMessages,
    INITIAL_MSG_COUNT,
    messagesOffset,
    loadingMore,
    loadMessages,
    loadOlderMessages,
    loadPlugins,
    loadConfig,
    saveConfigAndApply,
    loadLogs,
    sendMessage,
    handlePluginResponse,
    retryMessage,
    togglePlugin,
    startPolling,
    initializeApp,
    stopPolling,
    listenForLogs,
    listenForMessages,
    requestNotificationPermission,
    loadHostnames,
    loadHiddenAddresses,
    loadLocalHostname,
    loadProcesses,
    loadSessions,
    toggleHide,
    deleteConversation,
    renameContactDisplayName,
  };
});
