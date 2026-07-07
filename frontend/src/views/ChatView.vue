<template>
  <div class="chat-shell">
    <!-- Hostname tabs -->
    <div class="hostname-tabs" v-if="tabs.length > 0">
      <div class="tabs-track">
        <div
          v-for="tab in tabs"
          :key="tab.key"
          :class="['tab-chip', { active: selectedKey === tab.key }]"
          @click="selectedKey = tab.key"
        >
          <span class="tab-label">{{ tab.label }}</span>
          <span class="tab-count">{{ tab.count }}</span>
        </div>
      </div>
    </div>

    <!-- Contact + Chat split -->
    <div class="chat-body">
      <!-- Contact panel -->
      <div :class="['contact-panel', { 'contact-overlay': isMobile && selectedContact }]">
        <div class="panel-header">
          <span class="panel-title">联系人</span>
        </div>

        <t-input
          v-model="searchText"
          placeholder="搜索名称或主机名"
          size="small"
          clearable
          class="search-input"
        />

        <div class="contact-list">
          <div
            v-for="c in filteredContacts"
            :key="c.address"
            :class="['contact-item', { active: selectedContact === c.address, hidden: c.hidden }]"
            @click="selectContact(c.address)"
            @contextmenu.prevent="onContactContext($event, c)"
            @touchstart.passive="onTouchStart($event, c)"
            @touchend="onTouchEnd"
            @touchmove="onTouchMove"
          >
            <t-avatar size="40px">{{ initial(c.address) }}</t-avatar>
            <div class="contact-info">
              <div class="contact-name">{{ displayName(c.address) }}</div>
              <div class="contact-sub">{{ subLabel(c) }}</div>
            </div>
          </div>
          <t-empty v-if="filteredContacts.length === 0" description="暂无联系人" />

          <!-- Hidden section -->
          <div v-if="hiddenContacts.length > 0" class="hidden-section">
            <div class="hidden-toggle" @click="showHidden = !showHidden">
              <span>隐藏对话 ({{ hiddenContacts.length }})</span>
              <span class="toggle-arrow">{{ showHidden ? '▼' : '▶' }}</span>
            </div>
            <template v-if="showHidden">
              <div
                v-for="c in hiddenContacts"
                :key="c.address"
                :class="['contact-item', 'hidden-item', { active: selectedContact === c.address }]"
                @click="selectContact(c.address)"
                @contextmenu.prevent="onContactContext($event, c)"
                @touchstart.passive="onTouchStart($event, c)"
                @touchend="onTouchEnd"
                @touchmove="onTouchMove"
              >
                <t-avatar size="40px">{{ initial(c.address) }}</t-avatar>
                <div class="contact-info">
                  <div class="contact-name">{{ displayName(c.address) }}</div>
                  <div class="contact-sub">{{ subLabel(c) }}</div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>

      <!-- Chat area -->
      <div :class="['chat-panel', { 'chat-full': isMobile }]" v-if="selectedContact || !isMobile">
        <template v-if="selectedContact">
          <div class="chat-topbar">
            <span v-if="isMobile" class="back-btn" @click="selectedContact = ''">←</span>
            <span class="topbar-name">{{ displayName(selectedContact) }}</span>
          </div>
          <div class="message-area" ref="messagesContainer">
            <div
              v-for="msg in conversation"
              :key="msg.id"
              :class="['msg-row', msg.from === ownAddress ? 'row-self' : 'row-other']"
            >
              <div class="msg-bubble" @contextmenu.prevent="onBubbleContext($event, msg)">
                <div class="msg-text" v-if="msg.text" v-html="renderMarkdown(msg.text)"></div>
                <PluginComponent
                  v-if="(msg.meta as PluginMeta)?.plugin?.component"
                  :comp="(msg.meta as PluginMeta)!.plugin!.component!"
                  @respond="(value: string) => handleComponentResponse(msg, value)"
                />
                <div class="msg-files" v-if="msg.files?.length">
                  <t-link v-for="f in msg.files" :key="f.enc_name" theme="primary" size="small">
                    {{ f.name }} ({{ formatSize(f.size) }})
                  </t-link>
                </div>
                <div class="msg-time">{{ formatTime(msg.timestamp) }}</div>
                <!-- Pending status indicators -->
                <div v-if="(msg as any).__pending" class="msg-status">
                  <span v-if="(msg as any).__status === 'sending'" class="msg-spinner"></span>
                  <span v-else-if="(msg as any).__status === 'failed'" class="msg-retry" @click.stop="retryMessage(msg as any)" title="点击重试">⚠</span>
                </div>
              </div>
            </div>
          </div>
          <div class="input-area">
            <textarea
              ref="inputRef"
              v-model="inputText"
              placeholder="输入消息..."
              rows="1"
              class="chat-textarea"
              @keydown="onInputKeydown"
              @focus="onInputFocus"
            ></textarea>
            <div class="input-actions">
              <span class="input-hint">Enter 发送</span>
              <t-button :disabled="!inputText.trim()" size="small" @click="handleSend">发送</t-button>
            </div>
          </div>
        </template>
        <div class="chat-panel chat-empty" v-else>
          <t-empty description="选择一个联系人开始聊天" />
        </div>
      </div>
    </div>

    <!-- Context menu -->
    <div
      v-if="ctxMenu.visible"
      class="context-menu"
      :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }"
    >
      <div class="ctx-item" @click="copyCtxText">{{ ctxContact ? '复制地址' : '复制' }}</div>
      <div class="ctx-sep"></div>
      <div class="ctx-item" @click="toggleCtxHide">{{ ctxContact?.hidden ? '取消隐藏' : '隐藏对话' }}</div>
      <div v-if="ctxContact" class="ctx-sep"></div>
      <div v-if="ctxContact" class="ctx-item ctx-danger" @click="deleteCtxConversation">删除对话</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from "vue";
import { useRoute } from "vue-router";
import { MessagePlugin } from "tdesign-vue-next";
import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { mobileChatOpen } from "@/composables/useChatOpen";
import PluginComponent from "@/components/PluginComponent.vue";
import type { PluginMeta } from "@/types/plugin";

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function highlightCode(str: string, lang: string): string {
  if (lang && hljs.getLanguage(lang)) {
    try {
      return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`;
    } catch { /* fall through */ }
  }
  return `<pre class="hljs"><code>${escapeHtml(str)}</code></pre>`;
}

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
  highlight: highlightCode,
});

// Only allow safe URL protocols in markdown links — reject javascript: etc.
md.validateLink = function (url: string): boolean {
  return /^(https?:\/\/|mailto:)/i.test(url);
};

function renderMarkdown(text: string): string {
  return md.render(text);
}

function parseAddress(addr: string) {
  const at = addr.lastIndexOf("@");
  if (at < 0) return { name: addr, hash: "", hostname: "" };
  const hostname = addr.slice(at + 1);
  const local = addr.slice(0, at);
  const hashIdx = local.indexOf("#");
  if (hashIdx < 0) return { name: local, hash: "", hostname };
  return {
    name: local.slice(0, hashIdx),
    hash: local.slice(hashIdx + 1),
    hostname,
  };
}

function hostnameFromAddr(addr: string) {
  return parseAddress(addr).hostname;
}

const route = useRoute();
const store = useYseStore();
const isMobile = useIsMobile();
const inputText = ref("");
const selectedContact = ref("");

// On mobile, track if a chat is open to hide the tab bar
const chatOpenOnMobile = computed(() => isMobile.value && !!selectedContact.value);

// Intercept hardware back — clear selected contact instead of navigating away
function onPopState() {
  if (selectedContact.value) {
    selectedContact.value = "";
    // Prevent default navigation by re-pushing state
    history.pushState(null, "", route.fullPath);
  }
}
onMounted(() => {
  history.pushState(null, "", route.fullPath);
  window.addEventListener("popstate", onPopState);
});
onUnmounted(() => window.removeEventListener("popstate", onPopState));

// Sync mobileChatOpen so App.vue can hide the tab bar
watch(selectedContact, (v) => { mobileChatOpen.value = isMobile.value && !!v; }, { immediate: true });

const searchText = ref("");
const messagesContainer = ref<HTMLElement | null>(null);
const inputRef = ref<HTMLTextAreaElement | null>(null);
const selectedKey = ref("all");
const showHidden = ref(false);
const ctxContact = ref<{ address: string; hidden: boolean } | null>(null);

const ctxMenu = ref<{ visible: boolean; x: number; y: number; text: string }>({
  visible: false, x: 0, y: 0, text: "",
});

function onBubbleContext(e: MouseEvent, msg: { text?: string }) {
  ctxMenu.value = { visible: true, x: e.clientX, y: e.clientY, text: msg.text ?? "" };
  ctxContact.value = null;
}

function onContactContext(e: MouseEvent, c: { address: string; hidden: boolean }) {
  ctxMenu.value = { visible: true, x: e.clientX, y: e.clientY, text: "" };
  ctxContact.value = { address: c.address, hidden: c.hidden };
}

function copyCtxText() {
  if (ctxContact.value) {
    navigator.clipboard.writeText(ctxContact.value.address);
  } else if (ctxMenu.value.text) {
    navigator.clipboard.writeText(ctxMenu.value.text);
  }
  ctxMenu.value.visible = false;
}

async function deleteCtxConversation() {
  if (ctxContact.value) {
    await store.deleteConversation(ctxContact.value.address);
  }
  ctxMenu.value.visible = false;
}

async function toggleCtxHide() {
  if (ctxContact.value) {
    await store.toggleHide(ctxContact.value.address);
  }
  ctxMenu.value.visible = false;
}

// ---- Long press for mobile ----
let longPressTimer: ReturnType<typeof setTimeout> | null = null;
let longPressContact: { address: string; hidden: boolean } | null = null;
let touchStartY = 0;

function onTouchStart(e: TouchEvent, c: { address: string; hidden: boolean }) {
  longPressContact = c;
  touchStartY = e.touches[0].clientY;
  longPressTimer = setTimeout(() => {
    if (longPressContact) {
      const touch = e.changedTouches?.[0] ?? e.touches[0];
      onContactContext(
        { clientX: touch.clientX, clientY: touch.clientY, preventDefault: () => {} } as MouseEvent,
        longPressContact,
      );
    }
    longPressTimer = null;
  }, 500);
}

function onTouchEnd() {
  if (longPressTimer) {
    clearTimeout(longPressTimer);
    longPressTimer = null;
  }
  longPressContact = null;
}

function onTouchMove(e: TouchEvent) {
  if (longPressTimer && Math.abs(e.touches[0].clientY - touchStartY) > 10) {
    clearTimeout(longPressTimer);
    longPressTimer = null;
    longPressContact = null;
  }
}

document.addEventListener("click", () => {
  if (ctxMenu.value.visible) ctxMenu.value.visible = false;
});

const ownAddress = computed(() => store.config?.own_address ?? "me");
const connected = computed(() => store.connected);

interface Contact {
  address: string;
  lastText: string;
  lastTime: number;
  hostname: string;
  hidden: boolean;
}

const contacts = computed<Contact[]>(() => {
  const map = new Map<string, Contact>();
  for (const m of store.sortedMessages) {
    const addr = m.from === ownAddress.value ? m.to : m.from;
    if (addr === ownAddress.value) continue;
    if (!map.has(addr) || m.timestamp > map.get(addr)!.lastTime) {
      map.set(addr, {
        address: addr,
        lastText: m.text ?? "(文件)",
        lastTime: m.timestamp,
        hostname: hostnameFromAddr(addr),
        hidden: store.hiddenAddresses.has(addr),
      });
    }
  }
  // Include contacts from plugin_mappings that have no messages yet
  for (const m of store.config?.plugin_mappings ?? []) {
    const addr = m.virtual_addr;
    if (!map.has(addr)) {
      map.set(addr, {
        address: addr,
        lastText: "",
        lastTime: 0,
        hostname: hostnameFromAddr(addr),
        hidden: store.hiddenAddresses.has(addr),
      });
    }
  }
  return Array.from(map.values()).sort((a, b) => b.lastTime - a.lastTime);
});

const tabs = computed(() => {
  const groups = new Map<string, number>();
  for (const c of visibleContacts.value) {
    const h = c.hostname || "未知";
    groups.set(h, (groups.get(h) || 0) + 1);
  }
  // Always show "all" tab
  const allCount = Array.from(groups.values()).reduce((a, b) => a + b, 0);
  const result: { key: string; label: string; count: number }[] = [
    { key: "all", label: "全部对话", count: allCount },
  ];
  // Only show hostname tabs that have contacts
  for (const [key, count] of groups) {
    if (count > 0) {
      result.push({ key, label: key, count });
    }
  }
  return result;
});

const visibleContacts = computed(() =>
  contacts.value.filter((c) => !c.hidden),
);

const hiddenContacts = computed(() =>
  contacts.value.filter((c) => c.hidden),
);

const filteredContacts = computed(() => {
  let list =
    selectedKey.value === "all"
      ? visibleContacts.value
      : visibleContacts.value.filter((c) => c.hostname === selectedKey.value);

  if (!searchText.value) return list;
  const q = searchText.value.toLowerCase();
  return list.filter(
    (c) =>
      c.address.toLowerCase().includes(q) ||
      c.hostname.toLowerCase().includes(q),
  );
});

const conversation = computed(() => {
  const addr = selectedContact.value;
  if (!addr) return [];
  const real = store.sortedMessages.filter(
    (m) =>
      (m.from === addr && m.to === ownAddress.value) ||
      (m.from === ownAddress.value && m.to === addr),
  );
  const pending = store.pendingMessages
    .filter((p) => p.to === addr && (p.status === "sending" || p.status === "failed"))
    .map((p) => ({
      ...p,
      __pending: true,
      __status: p.status,
      protocol: "pending",
      files: undefined,
      meta: undefined,
    }));
  return [...real, ...pending];
});

function initial(addr: string) {
  const p = parseAddress(addr);
  return (p.name.charAt(0) || "?").toUpperCase();
}

function displayName(addr: string) {
  const p = parseAddress(addr);
  return p.name || addr;
}

function subLabel(c: Contact) {
  const p = parseAddress(c.address);
  if (p.hostname) return `@${p.hostname}`;
  return c.lastText;
}

function selectContact(addr: string) {
  selectedContact.value = addr;
}

function onInputFocus() {
  // On mobile, scroll the input area into view so it's not hidden by the keyboard
  setTimeout(() => {
    document.querySelector(".input-area")?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, 300);
}

function formatTime(ts: number) {
  const d = new Date(ts);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  return d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
}

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function onInputKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    handleSend();
  }
  const el = inputRef.value;
  if (el) {
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 120) + "px";
  }
}

async function handleSend() {
  if (!inputText.value.trim() || !selectedContact.value) return;
  try {
    await store.sendMessage(selectedContact.value, inputText.value.trim());
    inputText.value = "";
    await scrollToBottom();
  } catch (e) {
    await MessagePlugin.error(`发送失败: ${e}`);
  }
}

async function handleComponentResponse(msg: { from: string; to: string }, value: string) {
  const contact = msg.from === ownAddress.value ? msg.to : msg.from;
  await store.handlePluginResponse(contact, "", value);
  await scrollToBottom();
}

function retryMessage(msg: any) {
  store.retryMessage(msg);
}

async function scrollToBottom() {
  await nextTick();
  if (messagesContainer.value) {
    messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
  }
}

watch(selectedContact, scrollToBottom);

onMounted(async () => {
  await store.loadMessages();
  store.listenForMessages();
  store.listenForLogs();
});
</script>

<style scoped>
.chat-shell { display: flex; flex-direction: column; height: 100%; }

/* ---- Hostname tabs ---- */
.hostname-tabs {
  padding: 10px 16px 0;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  flex-shrink: 0;
}
.tabs-track {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  scrollbar-width: none;
  padding-bottom: 10px;
}
.tabs-track::-webkit-scrollbar { display: none; }
.tab-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  border-radius: 20px;
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 500;
  border: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-secondarycontainer);
  color: var(--td-text-color-secondary);
  transition: all 0.2s cubic-bezier(0.25, 0.46, 0.45, 0.94);
  flex-shrink: 0;
}
.tab-chip:hover {
  border-color: var(--td-brand-color);
  color: var(--td-brand-color);
  background: var(--td-brand-color-light);
}
.tab-chip.active {
  background: var(--td-brand-color);
  border-color: var(--td-brand-color);
  color: #fff;
  box-shadow: 0 2px 8px rgba(var(--td-brand-color-rgb), 0.25);
}
.tab-label { line-height: 1; }
.tab-count {
  font-size: 11px;
  font-weight: 600;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(0,0,0,0.08);
  line-height: 1;
}
.tab-chip.active .tab-count {
  background: rgba(255,255,255,0.2);
}

/* ---- Chat body ---- */
.chat-body { flex: 1; display: flex; overflow: hidden; }

/* ---- Contact panel ---- */
.contact-panel {
  width: 280px; min-width: 280px;
  display: flex; flex-direction: column;
  border-right: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.panel-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 12px 12px 4px;
}
.panel-title { font-size: 16px; font-weight: 600; }
.search-input { margin: 4px 8px; width: calc(100% - 16px); box-sizing: border-box; }
.contact-list { flex: 1; overflow-y: auto; padding: 4px 0; }
.contact-item {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 12px; cursor: pointer;
  transition: background 0.1s;
}
.contact-item:hover { background: var(--td-bg-color-secondarycontainer); }
.contact-item.active { background: var(--td-brand-color-light); }
.contact-item.hidden { opacity: 0.5; }
.contact-info { flex: 1; min-width: 0; }
.contact-name {
  font-size: 14px; font-weight: 500;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.contact-sub {
  font-size: 11px; color: var(--td-text-color-placeholder);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  margin-top: 2px;
}

.hidden-section { border-top: 1px solid var(--td-component-stroke); margin-top: 4px; }
.hidden-toggle {
  display: flex; justify-content: space-between; align-items: center;
  padding: 8px 12px; cursor: pointer; font-size: 13px; color: var(--td-text-color-placeholder);
}
.hidden-toggle:hover { background: var(--td-bg-color-secondarycontainer); }
.toggle-arrow { font-size: 10px; }
.hidden-item { opacity: 0.55; }
.hidden-item:hover { opacity: 0.8; }

/* ---- Chat panel ---- */
.chat-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.chat-empty { align-items: center; justify-content: center; }
.chat-topbar {
  padding: 12px 20px; font-size: 16px; font-weight: 600;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  display: flex; align-items: center; gap: 8px;
}
.back-btn { font-size: 20px; cursor: pointer; line-height: 1; user-select: none; }
.message-area {
  flex: 1; overflow-y: auto;
  padding: 16px 20px;
  display: flex; flex-direction: column; gap: 8px;
}
.msg-row { display: flex; }
.row-self { justify-content: flex-end; }
.row-other { justify-content: flex-start; }
.msg-bubble {
  max-width: 70%; padding: 8px 12px;
  border-radius: 12px; word-break: break-word; position: relative;
}
.row-self .msg-bubble {
  background: var(--td-brand-color); color: #fff;
  border-bottom-right-radius: 4px;
}
.row-other .msg-bubble {
  background: var(--td-bg-color-secondarycontainer);
  border-bottom-left-radius: 4px;
}
.msg-text { font-size: 14px; line-height: 1.6; }
.msg-text :deep(pre) { margin: 6px 0; padding: 8px 10px; border-radius: 6px; overflow-x: auto; font-size: 13px; background: var(--td-bg-color-component); }
.msg-text :deep(code) { font-family: ui-monospace, monospace; font-size: 0.9em; }
.msg-text :deep(p) { margin: 4px 0; }
.msg-text :deep(ul), .msg-text :deep(ol) { margin: 4px 0; padding-left: 20px; }
.msg-text :deep(table) { border-collapse: collapse; margin: 6px 0; font-size: 13px; width: 100%; }
.msg-text :deep(th), .msg-text :deep(td) { border: 1px solid var(--td-component-stroke); padding: 4px 8px; text-align: left; }
.msg-text :deep(th) { background: var(--td-bg-color-secondarycontainer); font-weight: 600; }
.msg-text :deep(blockquote) { margin: 4px 0; padding-left: 10px; border-left: 3px solid var(--td-brand-color); color: var(--td-text-color-placeholder); }
.msg-text :deep(a) { color: var(--td-brand-color); text-decoration: underline; }
.msg-text :deep(img) { max-width: 100%; border-radius: 6px; }
.msg-text :deep(input[type="checkbox"]) { margin-right: 4px; }
.msg-files { margin-top: 4px; }
.msg-time { font-size: 11px; margin-top: 4px; opacity: 0.65; text-align: right; }
.msg-status { display: inline-flex; align-items: center; margin-left: 6px; }
.msg-spinner {
  display: inline-block; width: 12px; height: 12px;
  border: 2px solid var(--td-text-color-placeholder);
  border-top-color: var(--td-brand-color);
  border-radius: 50%; animation: spin 0.6s linear infinite;
}
.msg-retry { cursor: pointer; font-size: 14px; color: var(--td-warning-color); }
@keyframes spin { to { transform: rotate(360deg); } }
.input-area {
  padding: 8px 16px 12px;
  border-top: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.chat-textarea {
  width: 100%; border: none; resize: none; outline: none;
  font-family: inherit; font-size: 14px; line-height: 1.5;
  padding: 6px 0; background: transparent; color: var(--td-text-color-primary);
}
.chat-textarea::placeholder { color: var(--td-text-color-placeholder); }
.input-actions {
  display: flex; justify-content: space-between; align-items: center;
  margin-top: 6px;
}
.input-hint { font-size: 12px; color: var(--td-text-color-placeholder); }

.context-menu {
  position: fixed; z-index: 9999;
  background: var(--td-bg-color-container);
  border: 1px solid var(--td-component-stroke);
  border-radius: 8px; box-shadow: var(--td-shadow-2);
  padding: 4px 0; min-width: 120px;
}
.ctx-item { padding: 6px 16px; font-size: 14px; cursor: pointer; }
.ctx-item:hover { background: var(--td-bg-color-secondarycontainer); }
.ctx-danger { color: var(--td-error-color); }
.ctx-danger:hover { background: var(--td-error-color-light, rgba(255,77,79,0.08)); }
.ctx-sep { height: 1px; background: var(--td-component-stroke); margin: 4px 8px; }

/* Mobile */
@media (max-width: 767px) {
  .contact-panel { width: 100%; min-width: 0; }
.chat-panel {
  position: fixed; top: 0; left: 0; right: 0; bottom: 0; z-index: 10;
  padding-top: env(safe-area-inset-top, 0);
  background: var(--td-bg-color-page);
}
  .message-area { padding-bottom: 56px; }
}
</style>
