<template>
  <div class="chat-shell">
    <!-- Contact + Chat split -->
    <div class="chat-body">
      <!-- Contact panel -->
      <div
        :class="[
          'contact-panel',
          { 'contact-overlay': isMobile && selectedContact },
        ]"
      >
        <div class="panel-header">
          <div class="panel-left">
            <img v-if="isMobile" src="/icon.png" class="panel-icon" alt="盐水鹅" />
            <span v-else class="panel-title">联系人</span>
            <button
              class="panel-mark-read"
              :class="{ 'has-unread': hasUnreadContact }"
              @click="store.markAllRead()"
              title="全部已读"
            >
              <svg viewBox="0 0 24 24" width="16" height="16">
                <path
                  fill="currentColor"
                  d="m9.55 18l-5.7-5.7l1.425-1.425L9.55 15.15l9.175-9.175L20.15 7.4z"
                />
              </svg>
            </button>
          </div>
          <t-select
            v-model="selectedKey"
            :options="hostnameOptions"
            size="small"
            :style="{ width: '130px' }"
            placeholder="全部对话"
          />
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
            :class="[
              'contact-item',
              { active: selectedContact === c.address, hidden: c.hidden },
            ]"
            @click="selectContact(c.address)"
            @contextmenu.prevent="onContactContext($event, c)"
            @touchstart.passive="onTouchStart($event, c)"
            @touchend="onTouchEnd"
            @touchmove="onTouchMove"
          >
            <div class="contact-avatar">
              <div class="avatar-box">
                <img
                  v-if="loadAvatar(c.address)"
                  :src="loadAvatar(c.address)!"
                  class="avatar-img"
                />
                <span v-else class="avatar-initial">{{
                  initial(c.address)
                }}</span>
              </div>
              <span v-if="c.hasNew" class="new-dot"></span>
            </div>
            <div class="contact-info">
              <div class="contact-row1">
                <span class="contact-name">{{ displayName(c.address) }}</span>
                <span class="contact-hostname">{{ hostnameLabel(c) }}</span>
              </div>
              <div class="contact-row2">
                <span class="contact-text">{{ c.lastText || "" }}</span>
                <span class="contact-time">{{
                  c.lastTime ? formatTime(c.lastTime) : ""
                }}</span>
              </div>
            </div>
          </div>
          <t-empty
            v-if="filteredContacts.length === 0"
            description="暂无联系人"
          />

          <!-- Hidden section -->
          <div v-if="hiddenContacts.length > 0" class="hidden-section">
            <div class="hidden-toggle" @click="showHidden = !showHidden">
              <span>隐藏对话 ({{ hiddenContacts.length }})</span>
              <span class="toggle-arrow">
                <ChevronDownIcon v-if="showHidden" />
                <ChevronRightIcon v-else />
              </span>
            </div>
            <template v-if="showHidden">
              <div
                v-for="c in hiddenContacts"
                :key="c.address"
                :class="[
                  'contact-item',
                  'hidden-item',
                  { active: selectedContact === c.address },
                ]"
                @click="selectContact(c.address)"
                @contextmenu.prevent="onContactContext($event, c)"
                @touchstart.passive="onTouchStart($event, c)"
                @touchend="onTouchEnd"
                @touchmove="onTouchMove"
              >
                <div class="contact-avatar">
                  <div class="avatar-box">
                    <img
                      v-if="loadAvatar(c.address)"
                      :src="loadAvatar(c.address)!"
                      class="avatar-img"
                    />
                    <span v-else class="avatar-initial">{{
                      initial(c.address)
                    }}</span>
                  </div>
                  <span v-if="c.hasNew" class="new-dot"></span>
                </div>
                <div class="contact-info">
                  <div class="contact-row1">
                    <span class="contact-name">{{
                      displayName(c.address)
                    }}</span>
                    <span class="contact-hostname">{{ hostnameLabel(c) }}</span>
                  </div>
                  <div class="contact-row2">
                    <span class="contact-text">{{ c.lastText || "" }}</span>
                    <span class="contact-time">{{
                      c.lastTime ? formatTime(c.lastTime) : ""
                    }}</span>
                  </div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>

      <!-- Chat area -->
      <div
        :class="['chat-panel', { 'chat-full': isMobile }]"
        v-if="selectedContact || !isMobile"
      >
        <template v-if="selectedContact">
          <div class="chat-topbar">
            <span v-if="isMobile" class="back-btn" @click="selectedContact = ''"
              ><ChevronLeftIcon
            /></span>
            <span class="topbar-name">{{ displayName(selectedContact) }}</span>
            <span class="topbar-more" @click="openContactActions"
              ><t-icon name="ellipsis"
            /></span>
          </div>
          <div
            class="message-area"
            ref="messagesContainer"
            @touchstart.passive="onMessageTouchStart"
            @touchmove.passive="onMessageTouchMove"
            @touchend="onMessageTouchEnd"
          >
            <div
              class="pull-indicator"
              :style="{ height: pullOffset + 'px', opacity: Math.min(pullOffset / 60, 1) }"
            >
              <span v-if="pullRefreshing" class="pull-spinner"></span>
              <span v-else class="pull-text">{{
                pullOffset > 60 ? "释放刷新" : "下拉刷新"
              }}</span>
            </div>
            <div
              v-for="msg in conversation"
              :key="msg.id"
              :class="[
                'msg-row',
                nameFromAddr(msg.from) === ownAddress
                  ? 'row-self'
                  : 'row-other',
              ]"
            >
              <div
                class="msg-bubble"
                @contextmenu.prevent="onBubbleContext($event, msg)"
                @touchstart.passive="onBubbleTouchStart($event, msg)"
                @touchend="onBubbleTouchEnd"
                @touchmove="onBubbleTouchMove"
              >
                <div
                  class="msg-text"
                  v-if="msg.text"
                  v-html="renderMarkdown(msg.text)"
                ></div>
                <PluginComponent
                  v-if="(msg.meta as PluginMeta)?.plugin?.component"
                  :comp="(msg.meta as PluginMeta)!.plugin!.component!"
                  @respond="
                    (value: string) => handleComponentResponse(msg, value)
                  "
                />
                <div class="msg-files" v-if="msg.files?.length">
                  <t-link
                    v-for="f in msg.files"
                    :key="f.enc_name"
                    theme="primary"
                    size="small"
                  >
                    {{ f.name }} ({{ formatSize(f.size) }})
                  </t-link>
                </div>
                <div class="msg-time">{{ formatTime(msg.timestamp) }}</div>
              </div>
              <!-- Pending status indicators (outside bubble, left side for self) -->
              <div v-if="isPending(msg)" class="msg-indicator">
                <span
                  v-if="(msg as PendingDisplayMessage).__status === 'sending'"
                  class="msg-spinner"
                ></span>
                <span
                  v-else-if="
                    (msg as PendingDisplayMessage).__status === 'failed'
                  "
                  class="msg-retry"
                  @click.stop="retryMessage(msg as PendingDisplayMessage)"
                  title="点击重试"
                  >⚠</span
                >
              </div>
            </div>
          </div>
          <div
            :class="[
              'input-area',
              { 'keyboard-open': isKeyboardOpen && isMobile },
            ]"
          >
            <textarea
              ref="inputRef"
              v-model="inputText"
              placeholder="输入消息..."
              rows="1"
              class="chat-textarea"
              @keydown="onInputKeydown"
              @focus="onInputFocus"
              @blur="onInputBlur"
              @input="autoResizeTextarea"
            ></textarea>
            <t-button
              class="send-btn"
              :disabled="!inputText.trim()"
              size="small"
              @click="handleSend"
              >发送</t-button
            >
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
      <div class="ctx-item" @click="copyCtxText">
        {{ ctxContact ? "复制地址" : "复制" }}
      </div>
      <div v-if="ctxContact" class="ctx-sep"></div>
      <div v-if="ctxContact" class="ctx-item" @click="toggleCtxHide">
        {{ ctxContact?.hidden ? "取消隐藏" : "隐藏对话" }}
      </div>
      <div v-if="!ctxContact && ctxMenu.text" class="ctx-sep"></div>
      <div
        v-if="!ctxContact && ctxMenu.text"
        class="ctx-item"
        @click="openForwardDialog"
      >
        转发
      </div>
      <div v-if="ctxContact" class="ctx-sep"></div>
      <div
        v-if="ctxContact"
        class="ctx-item ctx-danger"
        @click="deleteCtxConversation"
      >
        删除对话
      </div>
    </div>

    <!-- Rename dialog -->
    <t-dialog
      v-model:visible="renameDialog.visible"
      header="修改显示名称"
      :close-on-overlay-click="true"
      @confirm="confirmRename"
    >
      <t-input
        v-model="renameDialog.name"
        placeholder="联系人显示名称"
        @keydown.enter="confirmRename"
      />
    </t-dialog>

    <!-- Forward dialog -->
    <t-dialog
      v-model:visible="forwardVisible"
      header="转发消息"
      width="400px"
      :close-on-overlay-click="true"
      :footer="false"
    >
      <t-input
        v-model="forwardSearch"
        placeholder="搜索联系人..."
        size="small"
        clearable
        style="margin-bottom: 8px"
      />
      <div class="forward-list" :style="{ maxHeight: isMobile ? '40vh' : '300px' }">
        <div
          v-for="c in forwardContacts"
          :key="c.address"
          class="forward-contact"
          @click="toggleForwardSelect(c.address)"
        >
          <t-checkbox
            :checked="forwardSelected.has(c.address)"
            :style="{ pointerEvents: 'none' }"
          />
          <span class="forward-name">{{ resolveDisplayName(c.address) }}</span>
          <span class="forward-hostname">{{
            c.hostname ? "@" + c.hostname : ""
          }}</span>
        </div>
        <t-empty v-if="forwardContacts.length === 0" description="暂无联系人" />
      </div>
      <div class="forward-actions">
        <t-button size="small" variant="text" @click="selectAllForward"
          >全选</t-button
        >
        <t-button size="small" variant="text" @click="clearForward"
          >清空</t-button
        >
      </div>
      <t-textarea
        v-model="forwardExtraText"
        placeholder="添加附加内容（可选）"
        :autosize="{ minRows: 2, maxRows: 4 }"
        style="margin-top: 8px"
      />
      <t-space style="margin-top: 12px; justify-content: flex-end; width: 100%">
        <t-button variant="outline" @click="forwardVisible = false"
          >取消</t-button
        >
        <t-button
          theme="primary"
          :disabled="forwardSelected.size === 0"
          @click="confirmForward"
        >
          发送 ({{ forwardSelected.size }})
        </t-button>
      </t-space>
    </t-dialog>

    <!-- Chat settings panel: right sidebar (desktop) / full-page (mobile) -->
    <div
      v-if="showSettings"
      class="settings-backdrop"
      @click.self="showSettings = false"
    >
      <div :class="['settings-panel', { 'settings-mobile': isMobile }]">
        <div class="settings-header">
          <span class="settings-back" @click="showSettings = false">
            <ChevronLeftIcon v-if="isMobile" />
            <CloseIcon v-else />
          </span>
          <span class="settings-title">{{
            resolveDisplayName(selectedContact!)
          }}</span>
          <span class="settings-spacer"></span>
        </div>
        <div class="settings-body">
          <div class="settings-group">
            <div class="settings-group-label">信息</div>
            <div
              class="settings-item"
              @click="openRenameDialog(selectedContact!)"
            >
              <span class="settings-item-label">显示名称</span>
              <span class="settings-item-value">{{
                resolveDisplayName(selectedContact!)
              }}</span>
              <span class="settings-item-arrow"><ChevronRightIcon /></span>
            </div>
            <div
              class="settings-item"
              @click="pickAvatar(selectedContact!)"
            >
              <span class="settings-item-label">修改头像</span>
              <div class="avatar-preview-sm">
                <img
                  v-if="loadAvatar(selectedContact!)"
                  :src="loadAvatar(selectedContact!)!"
                  class="avatar-preview-img"
                />
                <span v-else class="avatar-preview-txt">{{
                  initial(selectedContact!)
                }}</span>
              </div>
              <span class="settings-item-arrow"><ChevronRightIcon /></span>
            </div>
          </div>
          <div class="settings-group">
            <div class="settings-group-label">操作</div>
            <div class="settings-item" @click="toggleSettingsHide">
              <span class="settings-item-label">{{
                isContactHidden ? "取消隐藏" : "隐藏对话"
              }}</span>
            </div>
            <div
              class="settings-item settings-item-danger"
              @click="deleteSettingsConversation"
            >
              <span class="settings-item-label">删除对话</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  ref,
  computed,
  onMounted,
  onUnmounted,
  nextTick,
  watch,
} from "vue";
import { useRoute } from "vue-router";
import MarkdownIt from "markdown-it";
import hljs from "highlight.js";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { mobileChatOpen } from "@/composables/useChatOpen";
import PluginComponent from "@/components/PluginComponent.vue";
import type { PluginMeta } from "@/types/plugin";
import type { PendingDisplayMessage } from "@/api/commands";
import { parseAddress, hostnameFromAddr, nameFromAddr } from "@/utils/address";
import { showError } from "@/utils/helpers";
import { MessagePlugin } from "tdesign-vue-next";
import {
  ChevronLeftIcon,
  CloseIcon,
  ChevronDownIcon,
  ChevronRightIcon,
} from "tdesign-icons-vue-next";

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function highlightCode(str: string, lang: string): string {
  if (lang && hljs.getLanguage(lang)) {
    try {
      return `<pre class="hljs"><code>${hljs.highlight(str, { language: lang, ignoreIllegals: true }).value}</code></pre>`;
    } catch {
      /* fall through */
    }
  }
  return `<pre class="hljs"><code>${escapeHtml(str)}</code></pre>`;
}

const md = new MarkdownIt({
  html: false,
  breaks: true,
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

const route = useRoute();
const store = useYseStore();
const isMobile = useIsMobile();
const inputText = ref("");
const selectedContact = ref("");

// On mobile, track if a chat is open to hide the tab bar
const chatOpenOnMobile = computed(
  () => isMobile.value && !!selectedContact.value,
);

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
watch(
  selectedContact,
  (v) => {
    mobileChatOpen.value = isMobile.value && !!v;
  },
  { immediate: true },
);

const searchText = ref("");
const messagesContainer = ref<HTMLElement | null>(null);
const inputRef = ref<HTMLTextAreaElement | null>(null);
const selectedKey = ref("local");
const showHidden = ref(false);
const ctxContact = ref<{ address: string; hidden: boolean } | null>(null);

// Track keyboard visibility for mobile safe area handling
const isKeyboardOpen = ref(false);

const pullOffset = ref(0);
const pullRefreshing = ref(false);
let pullStartY = 0;

const renameDialog = ref({ visible: false, name: "" });
const showSettings = ref(false);

const isContactHidden = computed(() =>
  selectedContact.value
    ? store.hiddenAddresses.has(selectedContact.value)
    : false,
);

const ctxMenu = ref<{ visible: boolean; x: number; y: number; text: string }>({
  visible: false,
  x: 0,
  y: 0,
  text: "",
});

const forwardVisible = ref(false);
const forwardSearch = ref("");
const forwardSelected = ref(new Set<string>());
const forwardExtraText = ref("");

function onBubbleContext(e: MouseEvent, msg: { text?: string }) {
  ctxMenu.value = {
    visible: true,
    x: e.clientX,
    y: e.clientY,
    text: msg.text ?? "",
  };
  ctxContact.value = null;
}

function onContactContext(
  e: MouseEvent,
  c: { address: string; hidden: boolean },
) {
  ctxMenu.value = { visible: true, x: e.clientX, y: e.clientY, text: "" };
  ctxContact.value = { address: c.address, hidden: c.hidden };
}

function copyCtxText() {
  if (ctxContact.value) {
    navigator.clipboard.writeText(ctxContact.value.address);
    MessagePlugin.success("已复制地址").catch(() => {});
  } else if (ctxMenu.value.text) {
    navigator.clipboard.writeText(ctxMenu.value.text);
    MessagePlugin.success("已复制").catch(() => {});
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

const forwardContacts = computed(() => {
  const list = visibleContacts.value.filter(
    (c) => !c.hidden || store.hiddenAddresses.has(c.address),
  );
  if (!forwardSearch.value) return list;
  const q = forwardSearch.value.toLowerCase();
  return list.filter(
    (c) =>
      resolveDisplayName(c.address).toLowerCase().includes(q) ||
      (c.hostname || "").toLowerCase().includes(q),
  );
});

function openForwardDialog() {
  forwardSelected.value = new Set();
  forwardExtraText.value = "";
  forwardSearch.value = "";
  ctxMenu.value.visible = false;
  forwardVisible.value = true;
}

function toggleForwardSelect(addr: string) {
  const s = new Set(forwardSelected.value);
  if (s.has(addr)) {
    s.delete(addr);
  } else {
    s.add(addr);
  }
  forwardSelected.value = s;
}

function selectAllForward() {
  forwardSelected.value = new Set(
    forwardContacts.value.map((c) => c.address),
  );
}

function clearForward() {
  forwardSelected.value = new Set();
}

async function confirmForward() {
  const originalText = ctxMenu.value.text;
  const extra = forwardExtraText.value.trim();
  const forwardText = extra
    ? `${originalText}\n---\n${extra}`
    : originalText;
  const targets = [...forwardSelected.value];
  for (const to of targets) {
    await store.sendMessage(to, forwardText);
  }
  forwardVisible.value = false;
  MessagePlugin.success(
    targets.length > 1 ? `已转发给 ${targets.length} 个联系人` : "已转发",
  );
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
      selectContact(longPressContact.address);
      showSettings.value = true;
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

// ---- Long press for messages (mobile copy) ----
let bubbleLongPressTimer: ReturnType<typeof setTimeout> | null = null;
let bubbleLongPressMsg: { text?: string } | null = null;
let bubbleTouchStartY = 0;

function onBubbleTouchStart(e: TouchEvent, msg: { text?: string }) {
  bubbleLongPressMsg = msg;
  bubbleTouchStartY = e.touches[0].clientY;
  bubbleLongPressTimer = setTimeout(() => {
    if (bubbleLongPressMsg) {
      const t = e.touches[0];
      ctxMenu.value = {
        visible: true,
        x: t.clientX,
        y: t.clientY,
        text: bubbleLongPressMsg.text ?? "",
      };
      ctxContact.value = null;
    }
    bubbleLongPressTimer = null;
  }, 500);
}

function onBubbleTouchEnd() {
  if (bubbleLongPressTimer) {
    clearTimeout(bubbleLongPressTimer);
    bubbleLongPressTimer = null;
  }
  bubbleLongPressMsg = null;
}

function onBubbleTouchMove(e: TouchEvent) {
  if (
    bubbleLongPressTimer &&
    Math.abs(e.touches[0].clientY - bubbleTouchStartY) > 10
  ) {
    clearTimeout(bubbleLongPressTimer);
    bubbleLongPressTimer = null;
    bubbleLongPressMsg = null;
  }
}

function openContactActions() {
  showSettings.value = true;
}

function openRenameDialog(addr: string) {
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === addr,
  );
  renameDialog.value = {
    visible: true,
    name: mapping?.display_name || parseAddress(addr).name,
  };
}

async function confirmRename() {
  if (!selectedContact.value || !renameDialog.value.name.trim()) return;
  await store.renameContactDisplayName(
    selectedContact.value,
    renameDialog.value.name.trim(),
  );
  renameDialog.value.visible = false;
}

async function toggleSettingsHide() {
  if (!selectedContact.value) return;
  await store.toggleHide(selectedContact.value);
  showSettings.value = false;
}

async function deleteSettingsConversation() {
  if (!selectedContact.value) return;
  const addr = selectedContact.value;
  showSettings.value = false;
  selectedContact.value = "";
  await store.deleteConversation(addr);
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
  hasNew?: boolean;
  lastIsSelf?: boolean;
}

const contacts = computed<Contact[]>(() => {
  void store.readVersion; // depend on readVersion so markRead/markAllRead triggers recompute
  const ownName = ownAddress.value;
  const map = new Map<string, Contact>();
  // 文件传输助手 — always present, address = ownName (no hostname, shared across devices)
  const selfAddr = ownName;
  for (const m of store.sortedMessages) {
    const addr = nameFromAddr(m.from) === ownName ? m.to : m.from;
    if (nameFromAddr(addr) === ownName) {
      // Self-addressed messages go into the 文件传输助手 conversation
      map.set(selfAddr, {
        address: selfAddr,
        lastText: m.text ?? "(文件)",
        lastTime: m.timestamp,
        hostname: "文件传输助手",
        hidden: store.hiddenAddresses.has(selfAddr),
        lastIsSelf: true,
      });
      continue;
    }
    const isSelf = nameFromAddr(m.from) === ownName;
    if (!map.has(addr) || m.timestamp > map.get(addr)!.lastTime) {
      map.set(addr, {
        address: addr,
        lastText: m.text ?? "(文件)",
        lastTime: m.timestamp,
        hostname: hostnameFromAddr(addr),
        hidden: store.hiddenAddresses.has(addr),
        lastIsSelf: isSelf,
      });
    }
  }
  // Ensure 文件传输助手 always has an entry
  if (!map.has(selfAddr)) {
    map.set(selfAddr, {
      address: selfAddr,
      lastText: "",
      lastTime: 0,
      hostname: "文件传输助手",
      hidden: store.hiddenAddresses.has(selfAddr),
    });
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
  const result = Array.from(map.values()).map((c) => {
    const ts = store.readTimestamps[c.address] ?? 0;
    return {
      ...c,
      hasNew:
        selectedContact.value !== c.address &&
        c.lastTime > ts &&
        !c.lastIsSelf,
    };
  });
  return result.sort((a, b) => b.lastTime - a.lastTime);
});

const hostnameOptions = computed(() => {
  const mappings = store.config?.plugin_mappings ?? [];
  const mapAddrs = new Set(mappings.map((m) => m.virtual_addr));
  const localCount = visibleContacts.value.filter((c) =>
    mapAddrs.has(c.address),
  ).length;
  const groups = new Map<string, number>();
  for (const c of visibleContacts.value) {
    const h = c.hostname || "未知";
    groups.set(h, (groups.get(h) || 0) + 1);
  }
  const result: { label: string; value: string }[] = [
    { label: `本机添加 (${localCount})`, value: "local" },
    { label: `全部对话 (${visibleContacts.value.length})`, value: "all" },
  ];
  for (const [key, count] of groups) {
    result.push({ label: `${key} (${count})`, value: key });
  }
  return result;
});

const visibleContacts = computed(() => contacts.value.filter((c) => !c.hidden));

const hasUnreadContact = computed(() => visibleContacts.value.some((c) => c.hasNew));

const hiddenContacts = computed(() => contacts.value.filter((c) => c.hidden));

const filteredContacts = computed(() => {
  let list: Contact[];
  if (selectedKey.value === "local") {
    const mapAddrs = new Set(
      (store.config?.plugin_mappings ?? []).map((m) => m.virtual_addr),
    );
    list = visibleContacts.value.filter((c) => mapAddrs.has(c.address));
  } else if (selectedKey.value === "all") {
    list = visibleContacts.value;
  } else {
    list = visibleContacts.value.filter(
      (c) => c.hostname === selectedKey.value,
    );
  }

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
  const ownName = ownAddress.value;
  const isSelf = nameFromAddr(addr) === ownName;
  const real = store.sortedMessages.filter((m) => {
    const mFrom = nameFromAddr(m.from);
    const mTo = nameFromAddr(m.to);
    if (isSelf) {
      // 文件传输助手 — only messages where BOTH sides are self
      return mFrom === ownName && mTo === ownName;
    }
    return (
      (m.from === addr && mTo === ownName) ||
      (mFrom === ownName && m.to === addr)
    );
  });
  const pending = store.pendingMessages
    .filter(
      (p) =>
        p.to === addr &&
        (p.status === "sending" ||
          p.status === "failed" ||
          p.status === "sent"),
    )
    .map((p) => ({
      ...p,
      __pending: true,
      __status: p.status,
      protocol: "pending",
      files: undefined,
      meta: undefined,
    }))
    // Dedup sent pending: skip if a real message with same text+timestamp already exists
    .filter((p) => {
      if (p.__status !== "sent") return true;
      return !real.some(
        (r) =>
          r.text &&
          p.text &&
          r.text === p.text &&
          Math.abs(r.timestamp - p.timestamp) < 5000,
      );
    });
  return [...real, ...pending].sort((a, b) => a.timestamp - b.timestamp);
});

function resolveDisplayName(addr: string) {
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === addr,
  );
  if (mapping?.display_name) return mapping.display_name;
  const p = parseAddress(addr);
  return p.name || addr;
}

function initial(addr: string) {
  const p = parseAddress(addr);
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === addr,
  );
  const name = mapping?.display_name || p.name;
  return (name.charAt(0) || "?").toUpperCase();
}

function avatarKey(addr: string) {
  const p = parseAddress(addr);
  return `yse-avatar-${p.name}@${p.hostname}`;
}

function loadAvatar(addr: string): string | null {
  return localStorage.getItem(avatarKey(addr));
}

function saveAvatar(addr: string, dataUrl: string) {
  localStorage.setItem(avatarKey(addr), dataUrl);
}

async function pickAvatar(addr: string) {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = "image/*";
  input.onchange = () => {
    const f = input.files?.[0];
    if (!f) return;
    const reader = new FileReader();
    reader.onload = () => {
      saveAvatar(addr, reader.result as string);
    };
    reader.readAsDataURL(f);
  };
  input.click();
}

function displayName(addr: string) {
  return resolveDisplayName(addr);
}

function hostnameLabel(c: Contact) {
  return c.hostname ? `@${c.hostname}` : "";
}

function selectContact(addr: string) {
  selectedContact.value = addr;
  store.markRead(addr);
}

let msgTouchStartX = 0;
let msgTouchStartY = 0;
let msgTouchIsSwipe = false;

function onMessageTouchStart(e: TouchEvent) {
  const t = e.touches[0];
  msgTouchStartX = t.clientX;
  msgTouchStartY = t.clientY;
  msgTouchIsSwipe = false;
  // Pull-to-refresh: only active when scrolled to top
  if (messagesContainer.value && messagesContainer.value.scrollTop <= 0) {
    pullStartY = t.clientY;
  }
}

function onMessageTouchMove(e: TouchEvent) {
  const t = e.touches[0];
  const dx = t.clientX - msgTouchStartX;
  const dy = t.clientY - msgTouchStartY;

  // Detect horizontal swipe (right) for back navigation
  if (Math.abs(dx) > 10 && Math.abs(dx) > Math.abs(dy) * 1.5 && !msgTouchIsSwipe) {
    msgTouchIsSwipe = true;
    pullOffset.value = 0;
    pullStartY = 0;
  }
  if (msgTouchIsSwipe) return;

  // Pull-to-refresh: only vertical down movement at scroll top
  if (!pullStartY) return;
  const deltaY = t.clientY - pullStartY;
  if (deltaY <= 0) {
    pullOffset.value = 0;
    return;
  }
  pullOffset.value = Math.min(deltaY * 0.5, 100);
}

async function onMessageTouchEnd(e: TouchEvent) {
  if (msgTouchIsSwipe) {
    const dx = e.changedTouches[0].clientX - msgTouchStartX;
    if (dx > 100 && isMobile.value && selectedContact.value) {
      selectedContact.value = "";
    }
  }
  // Pull-to-refresh
  if (pullOffset.value > 60 && !pullRefreshing.value) {
    pullRefreshing.value = true;
    await store.loadMessages();
    pullRefreshing.value = false;
  }
  pullOffset.value = 0;
  pullStartY = 0;
  msgTouchStartX = 0;
  msgTouchStartY = 0;
}

function onInputFocus() {
  isKeyboardOpen.value = true;
  // On mobile, scroll the input area into view so it's not hidden by the keyboard
  setTimeout(() => {
    document
      .querySelector(".input-area")
      ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, 300);
}

function onInputBlur() {
  // Small delay so the keyboard has time to start dismissing
  setTimeout(() => {
    isKeyboardOpen.value = false;
  }, 150);
}

function formatTime(ts: number) {
  const d = new Date(ts);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay)
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  return d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
}

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function autoResizeTextarea() {
  const el = inputRef.value;
  if (!el) return;
  const prevHeight = el.offsetHeight;
  el.style.height = "";
  const newH = Math.min(el.scrollHeight, 120);
  // Ignore tiny fluctuations (< 6px) to prevent single-line wobble
  if (Math.abs(newH - prevHeight) > 6) {
    el.style.height = newH + "px";
  } else {
    el.style.height = prevHeight + "px";
  }
}

function onInputKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    handleSend();
  }
}

async function handleSend() {
  if (!inputText.value.trim() || !selectedContact.value) return;
  try {
    await store.sendMessage(selectedContact.value, inputText.value.trim());
    inputText.value = "";
    if (inputRef.value) inputRef.value.style.height = "auto";
    await scrollToBottom();
  } catch (e) {
    showError("发送", e);
  }
}

async function handleComponentResponse(
  msg: { from: string; to: string },
  value: string,
) {
  const contact =
    nameFromAddr(msg.from) === ownAddress.value ? msg.to : msg.from;
  await store.handlePluginResponse(contact, "", value);
  await scrollToBottom();
}

function isPending(msg: unknown): msg is PendingDisplayMessage {
  return (
    typeof msg === "object" &&
    msg !== null &&
    "__pending" in msg &&
    (msg as PendingDisplayMessage).__pending === true
  );
}

function retryMessage(msg: PendingDisplayMessage) {
  const pending = store.pendingMessages.find((p) => p.id === msg.id);
  if (pending) store.retryMessage(pending);
}

async function scrollToBottom() {
  await nextTick();
  if (messagesContainer.value) {
    messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
  }
}

watch(selectedContact, scrollToBottom);

// On mobile, mark the conversation as read when leaving it.
// This catches messages that arrived via polling while viewing.
watch(selectedContact, (newVal, oldVal) => {
  if (!newVal && oldVal && isMobile.value) {
    store.markRead(oldVal);
  }
});

onMounted(async () => {
  await store.loadMessages();
  store.listenForMessages();
  store.listenForLogs();
});
</script>

<style scoped>
.chat-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* ---- Chat body ---- */
.chat-body {
  flex: 1;
  display: flex;
  overflow: hidden;
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
  padding: 12px 12px 4px;
}
.panel-left {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  min-width: 0;
}
.panel-icon {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.6);
  padding: 4px;
  flex-shrink: 0;
}
.panel-title {
  font-size: 16px;
  font-weight: 600;
}
.panel-mark-read {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 15px;
  background: transparent;
  color: var(--td-text-color-placeholder);
  cursor: pointer;
  transition: all 0.15s;
  font-family: inherit;
  flex-shrink: 0;
}
.panel-mark-read:hover {
  background: var(--td-bg-color-secondarycontainer);
  color: var(--td-brand-color);
}
.panel-mark-read.has-unread {
  color: var(--td-success-color);
  background: var(--td-success-color-1);
}
.search-input {
  margin: 4px 8px;
  width: calc(100% - 16px);
  box-sizing: border-box;
}
.contact-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}
.contact-list:hover {
  scrollbar-color: var(--td-component-stroke) transparent;
}
.contact-list::-webkit-scrollbar {
  width: 6px;
}
.contact-list::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 3px;
}
.contact-list:hover::-webkit-scrollbar-thumb {
  background: var(--td-component-stroke);
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
.contact-item.hidden {
  opacity: 0.5;
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
  flex: 1;
  min-width: 0;
}
.contact-row1 {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.contact-row2 {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 3px;
}
.contact-hostname {
  font-size: 10px;
  color: var(--td-text-color-placeholder);
  white-space: nowrap;
  max-width: 80px;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 0;
  margin-left: 6px;
}
.contact-text {
  font-size: 12px;
  color: var(--td-text-color-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  min-width: 0;
}
.contact-time {
  font-size: 11px;
  color: var(--td-text-color-placeholder);
  white-space: nowrap;
  flex-shrink: 0;
  margin-left: 6px;
}
.contact-avatar {
  position: relative;
  flex-shrink: 0;
}
.avatar-box {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--td-bg-color-secondarycontainer);
}
.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.avatar-initial {
  font-size: 16px;
  font-weight: 600;
  color: var(--td-brand-color);
  user-select: none;
}
.new-dot {
  --yse-dot-color: #2a52be;
  position: absolute;
  top: -2px;
  right: -2px;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--yse-dot-color);
  box-shadow: 0 0 6px 2px var(--yse-dot-color);
}
.contact-item.hasNew {
  background: var(--td-brand-color-light);
}

.hidden-section {
  border-top: 1px solid var(--td-component-stroke);
  margin-top: 4px;
}
.hidden-toggle {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
  color: var(--td-text-color-placeholder);
}
.hidden-toggle:hover {
  background: var(--td-bg-color-secondarycontainer);
}
.toggle-arrow {
  font-size: 10px;
}
.hidden-item {
  opacity: 0.55;
}
.hidden-item:hover {
  opacity: 0.8;
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
  display: flex;
  align-items: center;
  gap: 8px;
}
.back-btn {
  font-size: 20px;
  cursor: pointer;
  line-height: 1;
  user-select: none;
}
.message-area {
  flex: 1;
  overflow-y: auto;
  padding: 4px 20px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.pull-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  transition: height 0.15s ease;
  flex-shrink: 0;
}
.pull-text {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
}
.pull-spinner {
  width: 18px;
  height: 18px;
  border: 2px solid var(--td-component-stroke);
  border-top-color: var(--td-brand-color);
  border-radius: 50%;
  animation: pullSpin 0.6s linear infinite;
}
@keyframes pullSpin {
  to {
    transform: rotate(360deg);
  }
}
.msg-row {
  display: flex;
  align-items: flex-end;
  gap: 6px;
}
.row-self {
  flex-direction: row-reverse;
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
  font-size: 15px;
  line-height: 1.6;
}
.msg-text :deep(pre) {
  margin: 6px 0;
  padding: 8px 10px;
  border-radius: 6px;
  overflow-x: auto;
  font-size: 13px;
  background: var(--td-bg-color-component);
}
.msg-text :deep(code) {
  font-family: ui-monospace, monospace;
  font-size: 0.9em;
}
.msg-text :deep(p) {
  margin: 4px 0;
}
.msg-text :deep(ul),
.msg-text :deep(ol) {
  margin: 4px 0;
  padding-left: 20px;
}
.msg-text :deep(table) {
  border-collapse: collapse;
  margin: 6px 0;
  font-size: 13px;
  width: 100%;
}
.msg-text :deep(th),
.msg-text :deep(td) {
  border: 1px solid var(--td-component-stroke);
  padding: 4px 8px;
  text-align: left;
}
.msg-text :deep(th) {
  background: var(--td-bg-color-secondarycontainer);
  font-weight: 600;
}
.msg-text :deep(blockquote) {
  margin: 4px 0;
  padding-left: 10px;
  border-left: 3px solid var(--td-brand-color);
  color: var(--td-text-color-placeholder);
}
.msg-text :deep(a) {
  color: var(--td-brand-color);
  text-decoration: underline;
}
.msg-text :deep(img) {
  max-width: 100%;
  border-radius: 6px;
}
.msg-text :deep(input[type="checkbox"]) {
  margin-right: 4px;
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
.msg-indicator {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding-bottom: 8px;
}
.msg-spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid var(--td-text-color-placeholder);
  border-top-color: var(--td-brand-color);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
.msg-retry {
  cursor: pointer;
  font-size: 14px;
  color: var(--td-warning-color);
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* ---- Input area ---- */
.input-area {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 12px calc(10px + env(safe-area-inset-bottom, 0px));
  border-top: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  transition: padding-bottom 0.2s ease;
}

/* When keyboard is open on mobile, remove safe area inset */
.input-area.keyboard-open {
  padding-bottom: 10px;
}

.chat-textarea {
  flex: 1;
  resize: none;
  outline: none;
  font-family: inherit;
  font-size: 16px;
  line-height: 1.5;
  padding: 10px 12px;
  color: var(--td-text-color-primary);
  min-height: 44px;
  max-height: 120px;
  border: none;
  border-radius: 8px;
  background: var(--td-bg-color-secondarycontainer);
  box-sizing: border-box;
}
.chat-textarea::placeholder {
  color: var(--td-text-color-placeholder);
}

/* ---- Send button ---- */
.send-btn {
  align-self: flex-start;
  height: 44px;
  display: flex;
  align-items: center;
  font-size: 18px;
  font-weight: 500;
  padding: 0 15px;
  border-radius: 8px;
  flex-shrink: 0;
}

.context-menu {
  position: fixed;
  z-index: 9999;
  background: var(--td-bg-color-container);
  border: 1px solid var(--td-component-stroke);
  border-radius: 8px;
  box-shadow: var(--td-shadow-2);
  padding: 4px 0;
  min-width: 120px;
}
.ctx-item {
  padding: 6px 16px;
  font-size: 14px;
  cursor: pointer;
}
.ctx-item:hover {
  background: var(--td-bg-color-secondarycontainer);
}
.ctx-danger {
  color: var(--td-error-color);
}
.ctx-danger:hover {
  background: var(--td-error-color-light, rgba(255, 77, 79, 0.08));
}
.ctx-sep {
  height: 1px;
  background: var(--td-component-stroke);
  margin: 4px 8px;
}

/* Mobile */
@media (max-width: 767px) {
  .contact-panel {
    width: 100%;
    min-width: 0;
  }
  .chat-panel {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 10;
    padding-top: env(safe-area-inset-top, 0);
    background: var(--td-bg-color-page);
  }
  .msg-bubble {
    user-select: none;
    -webkit-user-select: none;
  }
  .message-area {
    padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px));
  }

  .input-area {
    padding: 10px 10px calc(10px + env(safe-area-inset-bottom, 0px));
    gap: 10px;
  }
  .input-area.keyboard-open {
    padding-bottom: 10px;
  }
  .chat-textarea {
    min-height: 44px;
    font-size: 16px;
  }
  .send-btn {
    height: 44px;
    min-height: 44px;
    font-size: 16px;
    padding: 0 16px;
  }
}

/* ---- Topbar more button ---- */
.topbar-more {
  cursor: pointer;
  padding: 0 4px;
  border-radius: 6px;
  font-size: 18px;
  color: var(--td-text-color-secondary);
  display: flex;
  align-items: center;
  margin-left: auto;
  transition: background 0.2s;
}
.topbar-more:hover {
  background: var(--td-bg-color-secondarycontainer);
}

/* ---- Settings panel ---- */
.settings-backdrop {
  position: fixed;
  inset: 0;
  z-index: 2000;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  justify-content: flex-end;
}
.settings-panel {
  width: 340px;
  max-width: 85vw;
  height: 100%;
  background: var(--td-bg-color-container);
  display: flex;
  flex-direction: column;
  animation: slideRight 0.25s ease;
  box-shadow: -2px 0 12px rgba(0, 0, 0, 0.1);
}
.settings-panel.settings-mobile {
  width: 100%;
  max-width: 100%;
  animation: slideUp 0.3s ease;
}
.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  border-bottom: 1px solid var(--td-component-stroke);
  flex-shrink: 0;
}
.settings-back {
  cursor: pointer;
  color: var(--td-brand-color);
  font-size: 16px;
  padding: 4px 0;
}
.settings-title {
  font-size: 17px;
  font-weight: 600;
  color: var(--td-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  text-align: center;
}
.settings-spacer {
  width: 60px;
}
.settings-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0 12px;
}
.settings-group {
  margin-bottom: 8px;
}
.settings-group-label {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  padding: 8px 16px 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.settings-item {
  display: flex;
  align-items: center;
  padding: 14px 16px;
  cursor: pointer;
  transition: background 0.15s;
  background: var(--td-bg-color-container);
}
.settings-item:active {
  background: var(--td-bg-color-secondarycontainer);
}
.settings-item-label {
  font-size: 16px;
  color: var(--td-text-color-primary);
  flex: 1;
}
.settings-item-value {
  font-size: 14px;
  color: var(--td-text-color-placeholder);
  margin-right: 8px;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.settings-item-arrow {
  font-size: 18px;
  color: var(--td-text-color-placeholder);
  display: flex;
  align-items: center;
}
.settings-item-danger .settings-item-label {
  color: var(--td-error-color);
}
.avatar-preview-sm {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--td-bg-color-secondarycontainer);
  margin-right: 4px;
}
.avatar-preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.avatar-preview-txt {
  font-size: 12px;
  font-weight: 600;
  color: var(--td-brand-color);
}
@keyframes slideRight {
  from {
    transform: translateX(100%);
  }
  to {
    transform: translateX(0);
  }
}
@keyframes slideUp {
  from {
    transform: translateY(100%);
  }
  to {
    transform: translateY(0);
  }
}

/* Forward dialog */
.forward-list {
  overflow-y: auto;
  border: 1px solid var(--td-component-stroke);
  border-radius: 6px;
}
.forward-contact {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  cursor: pointer;
  border-bottom: 1px solid var(--td-component-stroke);
  transition: background 0.1s;
}
.forward-contact:last-child {
  border-bottom: none;
}
.forward-contact:active {
  background: var(--td-bg-color-secondarycontainer);
}
.forward-name {
  font-size: 14px;
  color: var(--td-text-color-primary);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.forward-hostname {
  font-size: 11px;
  color: var(--td-text-color-placeholder);
  flex-shrink: 0;
}
.forward-actions {
  display: flex;
  gap: 8px;
  padding: 4px 0;
}
</style>
