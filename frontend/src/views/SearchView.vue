<template>
  <div class="search-backdrop" @click.self="goBack" @keydown.esc="goBack">
    <div class="search-panel">
      <!-- Top bar -->
      <div class="search-topbar">
        <span class="search-back" @click="goBack"><ChevronLeftIcon /></span>
        <span class="search-title">搜索聊天记录</span>
      </div>

<<<<<<< Updated upstream
      <!-- Search input -->
      <div class="search-input-bar">
        <t-input
          v-model="searchText"
          placeholder="搜索消息内容..."
          clearable
          autofocus
          @enter="doSearch"
        >
          <template #prefix-icon><t-icon name="search" /></template>
        </t-input>
      </div>

      <!-- Tabs -->
      <div class="search-tabs">
        <div
          v-for="tab in tabs"
          :key="tab.key"
          :class="['tab-item', { active: activeTab === tab.key }]"
          @click="activeTab = tab.key"
        >
          {{ tab.label }}
        </div>
      </div>

      <!-- Main body: left-right split -->
      <div class="search-body">
        <!-- Left: message list -->
        <div class="search-results" ref="resultsRef" @scroll="onResultsScroll">
          <div v-if="loading" class="search-status">加载中...</div>
          <template v-else>
            <div v-if="loadingMore" class="search-status">加载更早消息...</div>
            <div
              v-for="msg in filteredMessages"
              :key="msg.id"
              class="search-msg-item"
              @click="goToMessage(msg)"
              @contextmenu.prevent="openCtxMenu($event, msg)"
            >
              <div class="search-msg-avatar">
                <ContactAvatar :address="msg.from" />
              </div>
              <div class="search-msg-body">
                <div class="search-msg-header">
                  <span class="search-msg-sender">{{
                    nameFromAddr(msg.from)
                  }}</span>
                  <span class="search-msg-time">{{
                    formatTime(msg.timestamp)
                  }}</span>
                </div>
                <div
                  class="search-msg-preview"
                  v-html="highlightText(msg.text || '(无文本内容)')"
                />
              </div>
            </div>
            <div v-if="store.messages.length === 0" class="search-status">
              暂无消息记录
            </div>
            <div
              v-else-if="filteredMessages.length === 0"
              class="search-status"
            >
              未找到匹配的消息
            </div>
          </template>
          <div ref="scrollEnd" />
        </div>

        <!-- Right: filter panel -->
        <div class="search-filters" v-if="!isMobile">
          <div class="filter-header">
            <span class="filter-title">筛选</span>
            <span class="filter-reset" @click="resetFilters">重置</span>
          </div>

          <div class="filter-item">
            <label>发送人</label>
            <t-input
              :value="selectedSender"
              placeholder="点击选择"
              readonly
              @click="showPicker = 'sender'"
            >
              <template #suffix-icon><ChevronDownIcon /></template>
            </t-input>
          </div>

          <div class="filter-item">
            <label>时间</label>
            <div class="filter-time-range">
              <t-date-picker
                v-model="dateStart"
                placeholder="开始"
                allow-input
                clearable
              />
              <span class="filter-time-sep">—</span>
              <t-date-picker
                v-model="dateEnd"
                placeholder="截止"
                allow-input
                clearable
              />
            </div>
          </div>

          <div class="filter-item">
            <label>@用户</label>
            <t-input
              :value="selectedMention"
              placeholder="点击选择"
              readonly
              @click="showPicker = 'mention'"
            >
              <template #suffix-icon><ChevronDownIcon /></template>
            </t-input>
          </div>
        </div>
      </div>
=======
    <!-- Search input -->
    <div class="search-input-bar">
      <t-input
        v-model="searchText"
        placeholder="搜索消息内容..."
        clearable
        autofocus
        @enter="doSearch"
      >
        <template #prefix-icon><t-icon name="search" /></template>
      </t-input>
    </div>

    <!-- Tabs -->
    <div class="search-tabs">
      <div
        v-for="tab in tabs"
        :key="tab.key"
        :class="['tab-item', { active: activeTab === tab.key }]"
        @click="activeTab = tab.key"
      >
        {{ tab.label }}
      </div>
    </div>

    <!-- Main body: left-right split -->
    <div class="search-body">
      <!-- Left: message list -->
      <div class="search-results" ref="resultsRef" @scroll="onResultsScroll">
        <div v-if="loading" class="search-status">加载中...</div>
        <template v-else>
          <div v-if="loadingMore" class="search-status">加载更早消息...</div>
          <div
            v-for="msg in filteredMessages"
            :key="msg.id"
            class="search-msg-item"
            @click="goToMessage(msg)"
            @contextmenu.prevent="openCtxMenu($event, msg)"
          >
            <div class="search-msg-avatar">
              <ContactAvatar :address="msg.from" />
            </div>
            <div class="search-msg-body">
              <div class="search-msg-header">
                <span class="search-msg-sender">{{ nameFromAddr(msg.from) }}</span>
                <span class="search-msg-time">{{ formatTime(msg.timestamp) }}</span>
              </div>
              <div class="search-msg-preview" v-html="highlightText(msg.text || '(无文本内容)')" />
            </div>
          </div>
          <div v-if="store.messages.length === 0" class="search-status">
            暂无消息记录
          </div>
          <div v-else-if="filteredMessages.length === 0" class="search-status">
            未找到匹配的消息
          </div>
        </template>
        <div ref="scrollEnd" />
      </div>

      <!-- Right: filter panel -->
      <div class="search-filters" v-if="!isMobile">
        <div class="filter-header">
          <span class="filter-title">筛选</span>
          <span class="filter-reset" @click="resetFilters">重置</span>
        </div>

        <div class="filter-item">
          <label>发送人</label>
          <t-input
            :value="selectedSender"
            placeholder="点击选择"
            readonly
            @click="showPicker = 'sender'"
          >
            <template #suffix-icon><ChevronDownIcon /></template>
          </t-input>
        </div>

        <div class="filter-item">
          <label>时间</label>
          <div class="filter-time-range">
            <t-date-picker
              v-model="dateStart"
              placeholder="开始"
              allow-input
              clearable
            />
            <span class="filter-time-sep">—</span>
            <t-date-picker
              v-model="dateEnd"
              placeholder="截止"
              allow-input
              clearable
            />
          </div>
        </div>

        <div class="filter-item">
          <label>@用户</label>
          <t-input
            :value="selectedMention"
            placeholder="点击选择"
            readonly
            @click="showPicker = 'mention'"
          >
            <template #suffix-icon><ChevronDownIcon /></template>
          </t-input>
        </div>
      </div>
    </div>
>>>>>>> Stashed changes

      <div v-if="loadingMore" class="search-loading-bar"></div>
    </div>

    <ContextMenu
<<<<<<< Updated upstream
      :visible="ctxMenu.visible"
      :x="ctxMenu.x"
      :y="ctxMenu.y"
      :is-contact="false"
      :message-text="ctxMsgText"
      @copy="copyCtxText"
      @forward="
        forwardVisible = true;
        ctxMenu.visible = false;
      "
      @close="ctxMenu.visible = false"
    />

    <ForwardDialog
      :visible="forwardVisible"
      :contacts="visibleContacts"
      @close="forwardVisible = false"
      @confirm="confirmForward"
    />

    <!-- Contact picker modal -->
    <Teleport to="body">
      <div
        v-if="showPicker"
        class="picker-overlay"
        @click.self="showPicker = ''"
      >
=======
        :visible="ctxMenu.visible"
        :x="ctxMenu.x"
        :y="ctxMenu.y"
        :is-contact="false"
        :message-text="ctxMsgText"
        @copy="copyCtxText"
        @forward="forwardVisible = true; ctxMenu.visible = false"
        @close="ctxMenu.visible = false"
      />

      <ForwardDialog
        :visible="forwardVisible"
        :contacts="visibleContacts"
        @close="forwardVisible = false"
        @confirm="confirmForward"
      />

    <!-- Contact picker modal -->
    <Teleport to="body">
      <div v-if="showPicker" class="picker-overlay" @click.self="showPicker = ''">
>>>>>>> Stashed changes
        <ContactPicker
          :title="showPicker === 'sender' ? '选择发送人' : '选择用户'"
          :multiple="showPicker !== 'sender'"
          :max="showPicker === 'sender' ? 1 : 10"
          :filter-addr="contactAddr"
          @confirm="onPickerConfirm"
          @cancel="showPicker = ''"
        />
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useRouter } from "vue-router";
import { ChevronLeftIcon, ChevronDownIcon } from "tdesign-icons-vue-next";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { nameFromAddr } from "@/utils/address";
import ContactAvatar from "@/components/ContactAvatar.vue";
import ContactPicker from "@/components/ContactPicker.vue";
import ContextMenu from "@/components/ContextMenu.vue";
import ForwardDialog from "@/components/ForwardDialog.vue";
import type { Message } from "@/api/commands";

<<<<<<< Updated upstream
const props = withDefaults(
  defineProps<{
    contactAddr?: string;
  }>(),
  {
    contactAddr: "",
  },
);
=======
const props = withDefaults(defineProps<{
  contactAddr?: string;
}>(), {
  contactAddr: "",
});
>>>>>>> Stashed changes

const emit = defineEmits<{
  close: [];
  jump: [msgId: string, contactAddr: string];
}>();

const router = useRouter();
const store = useYseStore();
const isMobile = useIsMobile();

// ── State ──
const searchText = ref("");
const activeTab = ref("chat");
const loading = ref(false);
const loadingMore = ref(false);
const allLoaded = ref(false);
const resultsRef = ref<HTMLElement | null>(null);
const showPicker = ref<"" | "sender" | "mention">("");
const dateStart = ref("");
const dateEnd = ref("");
const selectedSender = ref("");
const selectedMention = ref("");
const forwardVisible = ref(false);
const ctxMsgText = ref("");
const ctxMenu = ref({ visible: false, x: 0, y: 0 });

const tabs = [
  { key: "chat", label: "聊天记录" },
  { key: "files", label: "文件" },
  { key: "images", label: "图片" },
];

onMounted(async () => {
  // Load all messages for search
  loading.value = true;
  while (await store.loadOlderMessages()) {
    // Keep loading until no more messages
  }
  loading.value = false;
  document.addEventListener("click", onDocClick);
});
onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
});
function onDocClick() {
  if (ctxMenu.value.visible) ctxMenu.value.visible = false;
}

// ── Data ──

// ── Filtering ──
const filteredMessages = computed(() => {
  let msgs = store.messages as Message[];

  // Tab filter
  if (activeTab.value === "files") {
    msgs = msgs.filter((m) => m.files && m.files.length > 0);
  } else if (activeTab.value === "images") {
<<<<<<< Updated upstream
    msgs = msgs.filter((m) =>
      m.files?.some((f) => f.mime.startsWith("image/")),
=======
    msgs = msgs.filter(
      (m) => m.files?.some((f) => f.mime.startsWith("image/")),
>>>>>>> Stashed changes
    );
  }

  // Search text
  if (searchText.value.trim()) {
    const q = searchText.value.trim().toLowerCase();
    msgs = msgs.filter(
      (m) =>
        (m.text && m.text.toLowerCase().includes(q)) ||
        nameFromAddr(m.from).toLowerCase().includes(q),
    );
  }

  // Sender filter
  if (selectedSender.value) {
    const senderName = nameFromAddr(selectedSender.value);
<<<<<<< Updated upstream
    msgs = msgs.filter((m) => nameFromAddr(m.from) === senderName);
=======
    msgs = msgs.filter(
      (m) => nameFromAddr(m.from) === senderName,
    );
>>>>>>> Stashed changes
  }

  // Time filter
  if (dateStart.value) {
    const startTs = new Date(dateStart.value).getTime();
    if (!isNaN(startTs)) {
      msgs = msgs.filter((m) => m.timestamp >= startTs);
    }
  }
  if (dateEnd.value) {
    const endTs = new Date(dateEnd.value).getTime() + 86400000; // end of day
    if (!isNaN(endTs)) {
      msgs = msgs.filter((m) => m.timestamp <= endTs);
    }
  }

  // Sort by time descending
  return [...msgs].sort((a, b) => b.timestamp - a.timestamp);
});

// ── Methods ──
function doSearch() {
  // Reactive, nothing extra needed
}

async function onResultsScroll() {
  const el = resultsRef.value;
  if (!el || loadingMore.value || allLoaded.value) return;
  // Detect when scrolled to the top (oldest messages)
  if (el.scrollTop <= 0) {
    loadingMore.value = true;
    const hasMore = await store.loadOlderMessages();
    if (!hasMore) allLoaded.value = true;
    loadingMore.value = false;
  }
}

function resetFilters() {
  selectedSender.value = "";
  selectedMention.value = "";
  dateStart.value = "";
  dateEnd.value = "";
}

function onPickerConfirm(selected: string[]) {
  if (showPicker.value === "sender") {
    selectedSender.value = selected[0] || "";
  } else {
    selectedMention.value = selected.join(", ");
  }
  showPicker.value = "";
}

function goToMessage(msg: Message) {
  emit("jump", msg.id, msg.from);
}

function goBack() {
  emit("close");
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const isToday =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (isToday) {
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  }
  return d.toLocaleDateString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function highlightText(text: string): string {
  if (!searchText.value.trim()) return escapeHtml(text);
  const q = searchText.value.trim();
  const regex = new RegExp(`(${escapeRegex(q)})`, "gi");
  return escapeHtml(text).replace(
    regex,
    '<mark class="search-highlight">$1</mark>',
  );
}

const visibleContacts = computed(() => {
  const seen = new Set<string>();
<<<<<<< Updated upstream
  return (store.messages as Message[])
    .filter((m) => {
      // Only show contacts relevant to the current conversation
      if (props.contactAddr) {
        if (m.from !== props.contactAddr && m.to !== props.contactAddr)
          return false;
      }
      const key = m.from + m.to;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .map((m) => ({
      address: m.from === props.contactAddr ? m.to : m.from,
      hostname: "",
      hidden: false,
    }));
=======
  return (store.messages as Message[]).filter((m) => {
    // Only show contacts relevant to the current conversation
    if (props.contactAddr) {
      if (m.from !== props.contactAddr && m.to !== props.contactAddr) return false;
    }
    const key = m.from + m.to;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  }).map((m) => ({
    address: m.from === props.contactAddr ? m.to : m.from,
    hostname: "",
    hidden: false,
  }));
>>>>>>> Stashed changes
});

async function confirmForward(targets: string[], extraText: string) {
  const text = extraText
    ? `${ctxMsgText.value}\n---\n${extraText}`
    : ctxMsgText.value;
  for (const to of targets) {
    await store.sendMessage(to, text);
  }
  forwardVisible.value = false;
}

function openCtxMenu(e: MouseEvent, msg: Message) {
  ctxMsgText.value = msg.text || "";
  ctxMenu.value = { visible: true, x: e.clientX, y: e.clientY };
}
function copyCtxText() {
  if (ctxMsgText.value) {
    navigator.clipboard.writeText(ctxMsgText.value);
  }
  ctxMenu.value.visible = false;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
</script>

<style scoped>
.search-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
}
.search-panel {
  width: 780px;
  max-width: 90vw;
  max-height: 85vh;
  min-height: 400px;
  background: var(--td-bg-color-page);
  border-radius: 12px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.25);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: searchFadeIn 0.2s ease;
}
@keyframes searchFadeIn {
<<<<<<< Updated upstream
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
=======
  from { opacity: 0; transform: scale(0.97); }
  to { opacity: 1; transform: scale(1); }
>>>>>>> Stashed changes
}

/* Top bar */
.search-topbar {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  flex-shrink: 0;
}
.search-back {
  font-size: 20px;
  cursor: pointer;
  line-height: 1;
  color: var(--td-brand-color);
  display: flex;
  align-items: center;
  padding: 0 8px 0 0;
}
.search-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--td-text-color-primary);
}

/* Search input */
.search-input-bar {
  padding: 8px 16px;
  background: var(--td-bg-color-container);
  flex-shrink: 0;
}

/* Tabs */
.search-tabs {
  display: flex;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  flex-shrink: 0;
}
.tab-item {
  padding: 8px 20px;
  cursor: pointer;
  font-size: 14px;
  color: var(--td-text-color-secondary);
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
  user-select: none;
}
.tab-item:hover {
  color: var(--td-text-color-primary);
  background: var(--td-bg-color-secondarycontainer);
}
.tab-item.active {
  color: var(--td-brand-color);
  border-bottom-color: var(--td-brand-color);
  font-weight: 500;
}

/* Main body */
.search-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  background: var(--td-bg-color-page);
}

/* Left: results */
.search-results {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
  background: var(--td-bg-color-page);
}
.search-msg-item {
  display: flex;
  padding: 10px 16px;
  cursor: pointer;
  transition: background 0.15s;
  border-bottom: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.search-msg-item:hover {
  background: var(--td-bg-color-secondarycontainer);
}
.search-msg-avatar {
  flex-shrink: 0;
  margin-right: 10px;
}
.search-msg-body {
  flex: 1;
  min-width: 0;
}
.search-msg-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 4px;
}
.search-msg-sender {
  font-size: 14px;
  font-weight: 500;
  color: var(--td-text-color-primary);
}
.search-msg-time {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  flex-shrink: 0;
  margin-left: 8px;
}
.search-msg-preview {
  font-size: 13px;
  color: var(--td-text-color-secondary);
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
.search-highlight {
  background: var(--td-warning-color-2);
  color: inherit;
  padding: 0 1px;
  border-radius: 2px;
}
.search-status {
  text-align: center;
  padding: 40px 16px;
  color: var(--td-text-color-placeholder);
  font-size: 14px;
}

/* Right: filters */
.search-filters {
  width: 280px;
  border-left: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  padding: 16px;
  overflow-y: auto;
  flex-shrink: 0;
}
.filter-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}
.filter-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--td-text-color-primary);
}
.filter-reset {
  font-size: 13px;
  color: var(--td-brand-color);
  cursor: pointer;
  user-select: none;
}
.filter-reset:hover {
  opacity: 0.8;
}
.filter-item {
  margin-bottom: 16px;
}
.filter-item label {
  display: block;
  font-size: 13px;
  color: var(--td-text-color-secondary);
  margin-bottom: 6px;
}
.filter-time-range {
  display: flex;
  align-items: center;
  gap: 6px;
}
.filter-time-range > :deep(.t-date-picker) {
  flex: 1;
  min-width: 0;
}
.filter-time-sep {
  color: var(--td-text-color-placeholder);
  flex-shrink: 0;
}

/* Picker overlay */
.search-loading-bar {
  height: 3px;
  flex-shrink: 0;
<<<<<<< Updated upstream
  background: linear-gradient(
    90deg,
    var(--td-brand-color) 30%,
    transparent 70%
  );
=======
  background: linear-gradient(90deg, var(--td-brand-color) 30%, transparent 70%);
>>>>>>> Stashed changes
  background-size: 200% 100%;
  animation: searchLoading 1.2s ease infinite;
}
@keyframes searchLoading {
<<<<<<< Updated upstream
  0% {
    background-position: 100% 0;
  }
  100% {
    background-position: -100% 0;
  }
=======
  0% { background-position: 100% 0; }
  100% { background-position: -100% 0; }
>>>>>>> Stashed changes
}

.picker-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}
</style>
