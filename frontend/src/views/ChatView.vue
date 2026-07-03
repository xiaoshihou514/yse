<template>
  <div class="chat-shell">
    <!-- Contact list -->
    <div class="contact-panel">
      <div class="panel-header">
        <span class="panel-title">消息</span>
      </div>

      <t-input
        v-model="searchText"
        placeholder="搜索联系人"
        size="small"
        clearable
        style="margin: 0 8px; width: calc(100% - 16px); box-sizing: border-box;"
      />
      <div class="contact-list">
        <div
          v-for="c in filteredContacts"
          :key="c.address"
          :class="['contact-item', { active: selectedContact === c.address }]"
          @click="selectContact(c.address)"
        >
          <t-avatar size="40px">{{ initials(c.address) }}</t-avatar>
          <div class="contact-info">
            <div class="contact-name">{{ c.address }}</div>
            <div class="contact-preview">{{ c.lastText }}</div>
          </div>
        </div>
        <t-empty v-if="filteredContacts.length === 0" description="暂无联系人" />
      </div>
      <div class="connection-bar">
        <t-tag :theme="connected ? 'success' : 'default'" size="small">
          {{ connected ? '已连接' : '未连接' }}
        </t-tag>
      </div>
    </div>

    <!-- Chat area -->
    <div class="chat-panel" v-if="selectedContact">
      <div class="chat-topbar">
        <span class="topbar-name">{{ selectedContact }}</span>
      </div>
      <div class="message-area" ref="messagesContainer">
        <div
          v-for="msg in conversation"
          :key="msg.id"
          :class="['msg-row', msg.from === ownAddress ? 'row-self' : 'row-other']"
        >
          <div class="msg-bubble">
            <div class="msg-text" v-if="msg.text">{{ msg.text }}</div>
            <div class="msg-files" v-if="msg.files?.length">
              <t-link v-for="f in msg.files" :key="f.enc_name" theme="primary" size="small">
                {{ f.name }} ({{ formatSize(f.size) }})
              </t-link>
            </div>
            <div class="msg-time">{{ formatTime(msg.timestamp) }}</div>
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
        ></textarea>
        <div class="input-actions">
          <span class="input-hint">Enter 发送</span>
          <t-button
            :disabled="!inputText.trim()"
            size="small"
            @click="handleSend"
          >发送</t-button>
        </div>
      </div>
    </div>

    <!-- No conversation selected -->
    <div class="chat-panel chat-empty" v-else>
      <t-empty description="选择一个联系人开始聊天" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";


const store = useYseStore();
const inputText = ref("");
const selectedContact = ref("");
const searchText = ref("");
const messagesContainer = ref<HTMLElement | null>(null);
const inputRef = ref<HTMLTextAreaElement | null>(null);

const ownAddress = computed(() => store.config?.own_address ?? "me@yse.org");
const connected = computed(() => store.connected);

// Build contact list from messages + mappings
interface Contact {
  address: string;
  lastText: string;
  lastTime: number;
}
const contacts = computed<Contact[]>(() => {
  const map = new Map<string, Contact>();
  // From messages
  for (const m of store.sortedMessages) {
    const addr = m.from === ownAddress.value ? m.to : m.from;
    if (addr === ownAddress.value) continue;
    if (!map.has(addr) || m.timestamp > map.get(addr)!.lastTime) {
      map.set(addr, {
        address: addr,
        lastText: m.text ?? "(文件)",
        lastTime: m.timestamp,
      });
    }
  }
  // From mappings (contacts without messages yet)
  for (const m of store.config?.plugin_mappings ?? []) {
    if (!map.has(m.virtual_addr)) {
      map.set(m.virtual_addr, {
        address: m.virtual_addr,
        lastText: m.plugin_id ? `→ ${m.plugin_id}` : "",
        lastTime: 0,
      });
    }
  }
  return Array.from(map.values()).sort((a, b) => b.lastTime - a.lastTime);
});

const filteredContacts = computed(() => {
  if (!searchText.value) return contacts.value;
  const q = searchText.value.toLowerCase();
  return contacts.value.filter((c) => c.address.toLowerCase().includes(q));
});

const conversation = computed(() => {
  const addr = selectedContact.value;
  if (!addr) return [];
  return store.sortedMessages.filter(
    (m) => (m.from === addr && m.to === ownAddress.value) ||
           (m.from === ownAddress.value && m.to === addr),
  );
});

function initials(addr: string) {
  return addr.charAt(0).toUpperCase();
}

function selectContact(addr: string) {
  selectedContact.value = addr;
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
  // Auto-resize
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

async function scrollToBottom() {
  await nextTick();
  if (messagesContainer.value) {
    messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
  }
}

watch(selectedContact, scrollToBottom);

onMounted(async () => {
  await store.loadMessages();
});
</script>

<style scoped>
.chat-shell {
  display: flex;
  height: 100%;
}

/* ---- Contact panel ---- */
.contact-panel {
  width: 280px;
  min-width: 280px;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 12px 4px;
}
.panel-title {
  font-size: 18px;
  font-weight: 600;
}
.contact-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}
.contact-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  cursor: pointer;
  transition: background 0.1s;
}
.contact-item:hover {
  background: var(--td-bg-color-secondarycontainer);
}
.contact-item.active {
  background: var(--td-brand-color-light);
}
.contact-info {
  flex: 1;
  min-width: 0;
}
.contact-name {
  font-size: 14px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.contact-preview {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 2px;
}
.connection-bar {
  padding: 8px 12px;
  display: flex;
  justify-content: center;
  border-top: 1px solid var(--td-component-stroke);
}

/* ---- Chat panel ---- */
.chat-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.chat-empty {
  align-items: center;
  justify-content: center;
}
.chat-topbar {
  padding: 12px 20px;
  font-size: 16px;
  font-weight: 600;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.message-area {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.msg-row {
  display: flex;
}
.row-self {
  justify-content: flex-end;
}
.row-other {
  justify-content: flex-start;
}
.msg-bubble {
  max-width: 70%;
  padding: 8px 12px;
  border-radius: 12px;
  word-break: break-word;
  position: relative;
}
.row-self .msg-bubble {
  background: var(--td-brand-color);
  color: #fff;
  border-bottom-right-radius: 4px;
}
.row-other .msg-bubble {
  background: var(--td-bg-color-secondarycontainer);
  border-bottom-left-radius: 4px;
}
.msg-text {
  font-size: 14px;
  line-height: 1.45;
}
.msg-files {
  margin-top: 4px;
}
.msg-time {
  font-size: 11px;
  margin-top: 4px;
  opacity: 0.65;
  text-align: right;
}
.input-area {
  padding: 8px 16px 12px;
  border-top: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.chat-textarea {
  width: 100%;
  border: none;
  resize: none;
  outline: none;
  font-family: inherit;
  font-size: 14px;
  line-height: 1.5;
  padding: 6px 0;
  background: transparent;
  color: var(--td-text-color-primary);
}
.chat-textarea::placeholder {
  color: var(--td-text-color-placeholder);
}
.input-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 6px;
}
.input-hint {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
}
</style>
