// ---------------------------------------------------------------------------
// Tauri invoke wrappers for yse-core commands
// These are typed stubs; actual Rust commands to be implemented in desktop/mobile.
// ---------------------------------------------------------------------------

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

// --- Types shared with Rust core -----------------------------------------

export interface ProcessInfo {
  id: string;
  name: string;
  exec_path: string;
  args: string[];
  state: string;
  start_time: number | null;
  restart_count: number;
  last_exit: string | null;
}

export interface SessionInfo {
  hash: string;
  plugin_id: string;
  name: string;
}

export interface Message {
  protocol: string;
  from: string;
  to: string;
  timestamp: number;
  id: string;
  text?: string;
  files?: FileAttachment[];
  meta?: Record<string, unknown>;
}

export interface FileAttachment {
  name: string;
  mime: string;
  size: number;
  enc_name: string;
}

export interface PluginConfig {
  id: string;
  name: string;
  exec_path: string;
  args: string[];
  enabled: boolean;
}

export interface YseConfig {
  email_imap_server: string;
  email_imap_port: number;
  email_smtp_server: string;
  email_smtp_port: number;
  email_username: string;
  email_password: string;
  own_address: string;
  crypto_password: string;
  plugin_mappings: { virtual_addr: string; plugin_id: string }[];
}

export interface LogEntry {
  level: "info" | "warn" | "error" | "debug";
  message: string;
  timestamp: number;
}

// --- Tauri commands -------------------------------------------------------

export async function sendMessage(to: string, text: string, files?: string[], meta?: Record<string, unknown>): Promise<void> {
  return invoke("send_message", { to, text, files, meta });
}

export async function getMessages(limit = 50, offset = 0): Promise<Message[]> {
  return invoke("get_messages", { limit, offset });
}

export async function getConfig(): Promise<YseConfig> {
  return invoke("get_config");
}

export async function saveConfig(config: YseConfig): Promise<void> {
  return invoke("save_config", { config });
}

export async function autoStartPlugins(): Promise<void> {
  return invoke("auto_start_plugins");
}

export async function startPolling(): Promise<void> {
  return invoke("start_polling");
}

export async function stopPolling(): Promise<void> {
  return invoke("stop_polling");
}

export async function listPlugins(): Promise<PluginConfig[]> {
  return invoke("list_plugins");
}

export async function startPlugin(id: string): Promise<void> {
  return invoke("start_plugin", { id });
}

export async function stopPlugin(id: string): Promise<void> {
  return invoke("stop_plugin", { id });
}

export async function getLogs(limit = 100): Promise<LogEntry[]> {
  return invoke("get_logs", { limit });
}

export async function testEmail(server: string, port: number, username: string, password: string): Promise<string> {
  return invoke("test_email", { server, port, username, password });
}

export async function listProcesses(): Promise<ProcessInfo[]> {
  return invoke("list_processes");
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke("list_sessions");
}

export async function getHostname(): Promise<string> {
  return invoke("get_hostname");
}

export async function toggleHideConversation(address: string, hidden: boolean): Promise<void> {
  return invoke("toggle_hide_conversation", { address, hidden });
}

export async function getHiddenAddresses(): Promise<string[]> {
  return invoke("get_hidden_addresses");
}

export async function getContactHashes(): Promise<[string, string][]> {
  return invoke("get_contact_hashes");
}

export async function getKnownHostnames(): Promise<string[]> {
  return invoke("get_known_hostnames");
}

export async function deleteConversation(address: string): Promise<void> {
  return invoke("delete_conversation", { address });
}
