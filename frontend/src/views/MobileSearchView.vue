<template>
  <div class="mobile-search">
    <!-- Top: back + search -->
    <div class="ms-header">
      <span class="ms-back" @click="goBack"><ChevronLeftIcon /></span>
      <div class="ms-search-box">
        <t-input
          v-model="searchText"
          placeholder="搜索消息内容..."
          clearable
          autofocus
        >
          <template #prefix-icon><t-icon name="search" /></template>
        </t-input>
      </div>
    </div>

    <!-- Tabs -->
    <div class="ms-tabs">
      <div
        v-for="tab in tabs"
        :key="tab.key"
        :class="['ms-tab', { active: activeTab === tab.key }]"
        @click="activeTab = tab.key"
      >
        {{ tab.label }}
      </div>
    </div>

    <!-- Filters -->
    <div class="ms-filters">
      <t-select
        v-model="senderFilter"
        :options="senderOptions"
        placeholder="发送人"
        clearable
        class="ms-filter-select"
      />
      <t-select
        v-model="timeFilter"
        :options="timeOptions"
        placeholder="时间"
        clearable
        class="ms-filter-select"
      />
    </div>

    <!-- Message list -->
    <div class="ms-list" ref="listRef" @scroll="onListScroll">
      <div v-if="loadingMore" class="ms-loading">加载更早消息...</div>
      <div
        v-for="msg in filteredMessages"
        :key="msg.id"
        class="ms-msg-item"
        @click="goToMessage(msg)"
      >
        <ContactAvatar :address="msg.from" />
        <div class="ms-msg-body">
          <div class="ms-msg-header">
            <span class="ms-msg-sender">
<<<<<<< Updated upstream
              {{ nameFromAddr(msg.from) }}<span class="ms-msg-arrow">&gt;</span
              >{{ resolveName(msg.to) }}
=======
              {{ nameFromAddr(msg.from) }}<span class="ms-msg-arrow">&gt;</span>{{ resolveName(msg.to) }}
>>>>>>> Stashed changes
            </span>
            <span class="ms-msg-time">{{ formatTime(msg.timestamp) }}</span>
          </div>
          <div class="ms-msg-preview">
<<<<<<< Updated upstream
            {{ msg.text || (msg.files?.length ? "[文件]" : "") || "(无内容)" }}
=======
            {{ msg.text || (msg.files?.length ? '[文件]' : '') || '(无内容)' }}
>>>>>>> Stashed changes
          </div>
        </div>
      </div>
      <div v-if="filteredMessages.length === 0" class="ms-empty">
<<<<<<< Updated upstream
        {{ searchText ? "未找到匹配的消息" : "暂无消息记录" }}
=======
        {{ searchText ? '未找到匹配的消息' : '暂无消息记录' }}
>>>>>>> Stashed changes
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { useRouter } from "vue-router";
import { ChevronLeftIcon } from "tdesign-icons-vue-next";
import { useYseStore } from "@/stores/yse";
import { nameFromAddr, parseAddress } from "@/utils/address";
import ContactAvatar from "@/components/ContactAvatar.vue";
import type { Message } from "@/api/commands";

const router = useRouter();
const store = useYseStore();

// ── State ──
const searchText = ref("");
const activeTab = ref("chat");
const senderFilter = ref("");
const timeFilter = ref("");
const loadingMore = ref(false);
const allLoaded = ref(false);
const listRef = ref<HTMLElement | null>(null);

const tabs = [
  { key: "chat", label: "聊天记录" },
  { key: "files", label: "文件" },
  { key: "links", label: "链接" },
];

// ── Filter options ──
const senderOptions = computed(() => {
  const addrs = new Set<string>();
  for (const msg of store.messages as Message[]) {
    if (msg.from) addrs.add(nameFromAddr(msg.from));
  }
<<<<<<< Updated upstream
  return Array.from(addrs)
    .sort()
    .map((n) => ({ label: n, value: n }));
=======
  return Array.from(addrs).sort().map((n) => ({ label: n, value: n }));
>>>>>>> Stashed changes
});

const timeOptions = [
  { label: "今天", value: "today" },
  { label: "最近 7 天", value: "7d" },
  { label: "最近 30 天", value: "30d" },
  { label: "今年", value: "year" },
];

// ── Filtering ──
const filteredMessages = computed(() => {
  let msgs = store.messages as Message[];

  // Tab filter
  if (activeTab.value === "files") {
    msgs = msgs.filter((m) => m.files && m.files.length > 0);
  } else if (activeTab.value === "links") {
<<<<<<< Updated upstream
    msgs = msgs.filter((m) => m.text && /https?:\/\/[^\s]+/.test(m.text));
=======
    msgs = msgs.filter(
      (m) => m.text && /https?:\/\/[^\s]+/.test(m.text),
    );
>>>>>>> Stashed changes
  }

  // Search text
  const q = searchText.value.trim().toLowerCase();
  if (q) {
    msgs = msgs.filter(
      (m) =>
        (m.text && m.text.toLowerCase().includes(q)) ||
        nameFromAddr(m.from).toLowerCase().includes(q),
    );
  }

  // Sender filter
  if (senderFilter.value) {
    msgs = msgs.filter(
<<<<<<< Updated upstream
      (m) =>
        nameFromAddr(m.from).toLowerCase() === senderFilter.value.toLowerCase(),
=======
      (m) => nameFromAddr(m.from).toLowerCase() === senderFilter.value.toLowerCase(),
>>>>>>> Stashed changes
    );
  }

  // Time filter
  if (timeFilter.value) {
    const now = Date.now();
    let startTs = 0;
    switch (timeFilter.value) {
      case "today":
        const today = new Date();
        today.setHours(0, 0, 0, 0);
        startTs = today.getTime();
        break;
      case "7d":
        startTs = now - 7 * 86400000;
        break;
      case "30d":
        startTs = now - 30 * 86400000;
        break;
      case "year":
        const y = new Date();
        y.setMonth(0, 1);
        y.setHours(0, 0, 0, 0);
        startTs = y.getTime();
        break;
    }
    if (startTs > 0) {
      msgs = msgs.filter((m) => m.timestamp >= startTs);
    }
  }

  // Sort by time descending
  return [...msgs].sort((a, b) => b.timestamp - a.timestamp);
});

// ── Methods ──
function resolveName(addr: string): string {
  const name = nameFromAddr(addr);
  return name === "me" ? "我" : name;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const isToday =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (isToday) {
<<<<<<< Updated upstream
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
=======
    return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
>>>>>>> Stashed changes
  }
  return d.toLocaleDateString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function goToMessage(msg: Message) {
  router.push({ path: "/", query: { contact: msg.from } });
}

async function onListScroll() {
  const el = listRef.value;
  if (!el || loadingMore.value || allLoaded.value) return;
  if (el.scrollTop < 80) {
    loadingMore.value = true;
    const hasMore = await store.loadOlderMessages();
    if (!hasMore) allLoaded.value = true;
    loadingMore.value = false;
  }
}

function goBack() {
  router.back();
}
</script>

<style scoped>
.mobile-search {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--td-bg-color-page);
  padding-top: env(safe-area-inset-top, 0);
  padding-bottom: env(safe-area-inset-bottom, 0);
}

/* Header */
.ms-header {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  gap: 8px;
  background: var(--td-bg-color-container);
  border-bottom: 1px solid var(--td-component-stroke);
  flex-shrink: 0;
}
.ms-back {
  font-size: 22px;
  cursor: pointer;
  color: var(--td-brand-color);
  display: flex;
  align-items: center;
  padding: 4px 0;
}
.ms-search-box {
  flex: 1;
}

/* Tabs */
.ms-tabs {
  display: flex;
  background: var(--td-bg-color-container);
  border-bottom: 1px solid var(--td-component-stroke);
  flex-shrink: 0;
}
.ms-tab {
  flex: 1;
  text-align: center;
  padding: 10px 0;
  font-size: 14px;
  color: var(--td-text-color-secondary);
  border-bottom: 2px solid transparent;
  cursor: pointer;
  user-select: none;
  transition: all 0.2s;
}
.ms-tab.active {
  color: var(--td-brand-color);
  border-bottom-color: var(--td-brand-color);
  font-weight: 500;
}

/* Filters */
.ms-filters {
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  background: var(--td-bg-color-container);
  border-bottom: 1px solid var(--td-component-stroke);
  flex-shrink: 0;
}
.ms-filter-select {
  flex: 1;
}

/* Message list */
.ms-list {
  flex: 1;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}
.ms-msg-item {
  display: flex;
  padding: 12px;
  gap: 10px;
  cursor: pointer;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  transition: background 0.15s;
}
.ms-msg-item:active {
  background: var(--td-bg-color-secondarycontainer);
}
.ms-msg-body {
  flex: 1;
  min-width: 0;
}
.ms-msg-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 4px;
}
.ms-msg-sender {
  font-size: 14px;
  font-weight: 500;
  color: var(--td-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ms-msg-arrow {
  margin: 0 4px;
  color: var(--td-text-color-placeholder);
  font-size: 12px;
}
.ms-msg-time {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  flex-shrink: 0;
  margin-left: 6px;
}
.ms-msg-preview {
  font-size: 13px;
  color: var(--td-text-color-secondary);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ms-empty {
  text-align: center;
  padding: 60px 16px;
  color: var(--td-text-color-placeholder);
  font-size: 14px;
  background: var(--td-bg-color-page);
}
.ms-loading {
  text-align: center;
  padding: 12px;
  color: var(--td-text-color-placeholder);
  font-size: 13px;
}
</style>
