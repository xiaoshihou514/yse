<template>
  <div class="chat-shell">
    <div class="chat-body">
      <div
        :class="['contact-panel', { 'contact-overlay': isMobile && selectedContact }]"
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
                <path fill="currentColor" d="m9.55 18l-5.7-5.7l1.425-1.425L9.55 15.15l9.175-9.175L20.15 7.4z" />
              </svg>
            </button>
          </div>
          <t-select v-model="selectedKey" :options="hostnameOptions" size="small" :style="{ width: '130px' }" placeholder="全部对话" />
        </div>

        <t-input v-model="searchText" placeholder="搜索名称或主机名" size="small" clearable class="search-input" />

        <div class="contact-list" ref="contactListRef"
             @touchstart.passive="onPullStart"
             @touchmove.passive="onPullMove"
             @touchend="onPullEnd">
          <div class="pull-indicator" :style="{ height: pullOffset + 'px', opacity: Math.min(pullOffset / 60, 1) }">
            <span v-if="pullRefreshing" class="pull-spinner"></span>
            <span v-else class="pull-text">{{ pullOffset > 60 ? "释放刷新" : "下拉刷新" }}</span>
          </div>
          <ContactListItem
            v-for="c in filteredContacts"
            :key="c.address"
            :contact="c"
            :active="selectedContact === c.address"
            @select="selectContact(c.address)"
            @context="onContactContext"
            @touchStart="onTouchStart"
            @touchEnd="onTouchEnd"
            @touchMove="onTouchMove"
          />
          <t-empty v-if="filteredContacts.length === 0" description="暂无联系人" />

          <div v-if="hiddenContacts.length > 0" class="hidden-section">
            <div class="hidden-toggle" @click="showHidden = !showHidden">
              <span>隐藏对话 ({{ hiddenContacts.length }})</span>
              <span class="toggle-arrow">
                <ChevronDownIcon v-if="showHidden" />
                <ChevronRightIcon v-else />
              </span>
            </div>
            <template v-if="showHidden">
              <ContactListItem
                v-for="c in hiddenContacts"
                :key="c.address"
                :contact="c"
                :active="selectedContact === c.address"
                class="hidden-item"
                @select="selectContact(c.address)"
                @context="onContactContext"
                @touchStart="onTouchStart"
                @touchEnd="onTouchEnd"
                @touchMove="onTouchMove"
              />
            </template>
          </div>
        </div>
      </div>

      <div :class="['chat-panel', { 'chat-full': isMobile }]" v-if="selectedContact || !isMobile">
        <template v-if="selectedContact">
          <div class="chat-topbar">
            <span v-if="isMobile" class="back-btn" @click="selectedContact = ''"><ChevronLeftIcon /></span>
            <span class="topbar-name">{{ resolveDisplayName(selectedContact) }}</span>
            <span class="topbar-more" @click="showSettings = true"><t-icon name="ellipsis" /></span>
          </div>
          <div class="message-area" ref="messagesContainer"
               @touchstart.passive="onSwipeStart"
               @touchmove.passive="onSwipeMove"
               @touchend="onSwipeEnd">
            <div v-for="msg in conversation" :key="msg.id"
                 :class="['msg-row', nameFromAddr(msg.from) === ownAddress ? 'row-self' : 'row-other']">
              <MessageBubble
                :message="msg"
                :alignment="nameFromAddr(msg.from) === ownAddress ? 'self' : 'other'"
                :is-pending="isPending(msg)"
                :pending-status="isPending(msg) ? (msg as any).__status : undefined"
                :pending-msg="msg"
                @bubbleContext="onBubbleContext"
                @bubbleTouchStart="onBubbleTouchStart"
                @bubbleTouchEnd="onBubbleTouchEnd"
                @bubbleTouchMove="onBubbleTouchMove"
                @respond="(v: string) => handleComponentResponse(msg, v)"
                @retry="retryMessage"
              />
            </div>
          </div>
          <ChatInput v-if="!(showSettings && !isMobile)" v-model="inputText" @send="handleSend" />
        </template>
        <div class="chat-panel chat-empty" v-else>
          <t-empty description="选择一个联系人开始聊天" />
        </div>
      </div>
    </div>

    <ContextMenu
      :visible="ctxMenu.visible"
      :x="ctxMenu.x"
      :y="ctxMenu.y"
      :is-contact="!!ctxContact"
      :is-hidden="ctxContact?.hidden"
      :message-text="ctxMenu.text"
      @copy="copyCtxText"
      @toggleHide="toggleCtxHide"
      @forward="openForwardDialog"
      @delete="deleteCtxConversation"
    />

    <t-dialog v-model:visible="renameDialog.visible" header="修改显示名称"
              :close-on-overlay-click="true" @confirm="confirmRename">
      <t-input v-model="renameDialog.name" placeholder="联系人显示名称" @keydown.enter="confirmRename" />
    </t-dialog>

    <ChatSettingsPanel
      v-if="selectedContact"
      :visible="showSettings"
      :address="selectedContact"
      :is-hidden="store.hiddenAddresses.has(selectedContact)"
      @close="showSettings = false"
      @rename="openRenameDialog(selectedContact!)"
      @changeAvatar="pickAvatar(selectedContact!)"
      @toggleHide="toggleSettingsHide"
      @delete="deleteSettingsConversation"
    />

    <ForwardDialog
      :visible="forwardVisible"
      :contacts="visibleContacts"
      @close="forwardVisible = false"
      @confirm="confirmForward"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from "vue";
import { useRoute } from "vue-router";
import ContactListItem from "@/components/ContactListItem.vue";
import ContextMenu from "@/components/ContextMenu.vue";
import ChatSettingsPanel from "@/components/ChatSettingsPanel.vue";
import ForwardDialog from "@/components/ForwardDialog.vue";
import MessageBubble from "@/components/MessageBubble.vue";
import ChatInput from "@/components/ChatInput.vue";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { mobileChatOpen } from "@/composables/useChatOpen";
import { parseAddress, hostnameFromAddr, nameFromAddr } from "@/utils/address";
import { pickAvatar } from "@/composables/useAvatar";
import { MessagePlugin } from "tdesign-vue-next";
import { ChevronLeftIcon, ChevronDownIcon, ChevronRightIcon } from "tdesign-icons-vue-next";


const route = useRoute();
const store = useYseStore();
const isMobile = useIsMobile();
const inputText = ref("");
const selectedContact = ref("");

let msgTouchStartX = 0;
let msgTouchStartY = 0;
let msgTouchIsSwipe = false;

function onPopState() {
  if (selectedContact.value) {
    selectedContact.value = "";
    history.pushState(null, "", route.fullPath);
  }
}
onMounted(() => { history.pushState(null, "", route.fullPath); window.addEventListener("popstate", onPopState); });
onUnmounted(() => window.removeEventListener("popstate", onPopState));

watch(selectedContact, (v) => { mobileChatOpen.value = isMobile.value && !!v; }, { immediate: true });

const searchText = ref("");
const messagesContainer = ref<HTMLElement | null>(null);
const contactListRef = ref<HTMLElement | null>(null);
const selectedKey = ref("local");
const showHidden = ref(false);
const ctxContact = ref<{ address: string; hidden: boolean } | null>(null);
const pullOffset = ref(0);
const pullRefreshing = ref(false);
let pullStartY = 0;
const renameDialog = ref({ visible: false, name: "" });
const showSettings = ref(false);

const ctxMenu = ref<{ visible: boolean; x: number; y: number; text: string }>({ visible: false, x: 0, y: 0, text: "" });

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
    MessagePlugin.success("已复制地址").catch(() => {});
  } else if (ctxMenu.value.text) {
    navigator.clipboard.writeText(ctxMenu.value.text);
    MessagePlugin.success("已复制").catch(() => {});
  }
  ctxMenu.value.visible = false;
}
async function deleteCtxConversation() {
  if (ctxContact.value) { await store.deleteConversation(ctxContact.value.address); }
  ctxMenu.value.visible = false;
}
async function toggleCtxHide() {
  if (ctxContact.value) { await store.toggleHide(ctxContact.value.address); }
  ctxMenu.value.visible = false;
}

const forwardVisible = ref(false);

function openForwardDialog() {
  ctxMenu.value.visible = false;
  forwardVisible.value = true;
}
async function confirmForward(targets: string[], extraText: string) {
  const text = extraText ? `${ctxMenu.value.text}\n---\n${extraText}` : ctxMenu.value.text;
  for (const to of targets) { await store.sendMessage(to, text); }
  forwardVisible.value = false;
  MessagePlugin.success(targets.length > 1 ? `已转发给 ${targets.length} 个联系人` : "已转发");
}

let longPressTimer: ReturnType<typeof setTimeout> | null = null;
let longPressContact: { address: string; hidden: boolean } | null = null;
let touchStartY = 0;

function onTouchStart(e: TouchEvent, c: { address: string; hidden: boolean }) {
  longPressContact = c;
  touchStartY = e.touches[0].clientY;
  longPressTimer = setTimeout(() => {
    if (longPressContact) { selectContact(longPressContact.address); showSettings.value = true; }
    longPressTimer = null;
  }, 500);
}
function onTouchEnd() { if (longPressTimer) { clearTimeout(longPressTimer); longPressTimer = null; } longPressContact = null; }
function onTouchMove(e: TouchEvent) {
  if (longPressTimer && Math.abs(e.touches[0].clientY - touchStartY) > 10) { clearTimeout(longPressTimer); longPressTimer = null; longPressContact = null; }
}

let bubbleLongPressTimer: ReturnType<typeof setTimeout> | null = null;
let bubbleLongPressMsg: { text?: string } | null = null;
let bubbleTouchStartY = 0;

function onBubbleTouchStart(e: TouchEvent, msg: { text?: string }) {
  bubbleLongPressMsg = msg;
  bubbleTouchStartY = e.touches[0].clientY;
  bubbleLongPressTimer = setTimeout(() => {
    if (bubbleLongPressMsg) {
      const t = e.touches[0];
      ctxMenu.value = { visible: true, x: t.clientX, y: t.clientY, text: bubbleLongPressMsg.text ?? "" };
      ctxContact.value = null;
    }
    bubbleLongPressTimer = null;
  }, 500);
}
function onBubbleTouchEnd() { if (bubbleLongPressTimer) { clearTimeout(bubbleLongPressTimer); bubbleLongPressTimer = null; } bubbleLongPressMsg = null; }
function onBubbleTouchMove(e: TouchEvent) {
  if (bubbleLongPressTimer && Math.abs(e.touches[0].clientY - bubbleTouchStartY) > 10) {
    clearTimeout(bubbleLongPressTimer); bubbleLongPressTimer = null; bubbleLongPressMsg = null;
  }
}

function openRenameDialog(addr: string) {
  const mapping = (store.config?.plugin_mappings ?? []).find((m) => m.virtual_addr === addr);
  renameDialog.value = { visible: true, name: mapping?.display_name || parseAddress(addr).name };
}
async function confirmRename() {
  if (!selectedContact.value || !renameDialog.value.name.trim()) return;
  await store.renameContactDisplayName(selectedContact.value, renameDialog.value.name.trim());
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

document.addEventListener("click", () => { if (ctxMenu.value.visible) ctxMenu.value.visible = false; });

const ownAddress = computed(() => store.config?.own_address ?? "me");

interface Contact {
  address: string; lastText: string; lastTime: number; hostname: string; hidden: boolean;
  hasNew?: boolean; lastIsSelf?: boolean;
}

const contacts = computed<Contact[]>(() => {
  void store.readVersion;
  const ownName = ownAddress.value;
  const map = new Map<string, Contact>();
  const selfAddr = ownName;
  for (const m of store.sortedMessages) {
    const addr = nameFromAddr(m.from) === ownName ? m.to : m.from;
    // Skip bare addresses (no hash) — they don't represent valid contacts
    if (!addr.includes("#")) continue;
    if (nameFromAddr(addr) === ownName) {
      map.set(selfAddr, { address: selfAddr, lastText: m.text ?? "(文件)", lastTime: m.timestamp, hostname: "文件传输助手", hidden: store.hiddenAddresses.has(selfAddr), lastIsSelf: true });
      continue;
    }
    const isSelf = nameFromAddr(m.from) === ownName;
    if (!map.has(addr) || m.timestamp > map.get(addr)!.lastTime) {
      map.set(addr, { address: addr, lastText: m.text ?? "(文件)", lastTime: m.timestamp, hostname: hostnameFromAddr(addr), hidden: store.hiddenAddresses.has(addr), lastIsSelf: isSelf });
    }
  }
  if (!map.has(selfAddr)) {
    map.set(selfAddr, { address: selfAddr, lastText: "", lastTime: 0, hostname: "文件传输助手", hidden: store.hiddenAddresses.has(selfAddr) });
  }
  for (const m of store.config?.plugin_mappings ?? []) {
    const addr = m.virtual_addr;
    if (!map.has(addr)) { map.set(addr, { address: addr, lastText: "", lastTime: 0, hostname: hostnameFromAddr(addr), hidden: store.hiddenAddresses.has(addr) }); }
  }
  const result = Array.from(map.values()).map((c) => {
    const ts = store.readTimestamps[c.address] ?? 0;
    return { ...c, hasNew: selectedContact.value !== c.address && c.lastTime > ts && !c.lastIsSelf };
  });
  return result.sort((a, b) => b.lastTime - a.lastTime);
});

const hostnameOptions = computed(() => {
  const mappings = store.config?.plugin_mappings ?? [];
  const mapAddrs = new Set(mappings.map((m) => m.virtual_addr));
  const localCount = visibleContacts.value.filter((c) => mapAddrs.has(c.address)).length;
  const groups = new Map<string, number>();
  for (const c of visibleContacts.value) { const h = c.hostname || "未知"; groups.set(h, (groups.get(h) || 0) + 1); }
  const result: { label: string; value: string }[] = [
    { label: `本机添加 (${localCount})`, value: "local" },
    { label: `全部对话 (${visibleContacts.value.length})`, value: "all" },
  ];
  for (const [key, count] of groups) { result.push({ label: `${key} (${count})`, value: key }); }
  return result;
});

const visibleContacts = computed(() => contacts.value.filter((c) => !c.hidden));
const hasUnreadContact = computed(() => visibleContacts.value.some((c) => c.hasNew));
const hiddenContacts = computed(() => contacts.value.filter((c) => c.hidden));

const filteredContacts = computed(() => {
  let list: Contact[];
  if (selectedKey.value === "local") {
    const mapAddrs = new Set((store.config?.plugin_mappings ?? []).map((m) => m.virtual_addr));
    list = visibleContacts.value.filter((c) => mapAddrs.has(c.address));
  } else if (selectedKey.value === "all") {
    list = visibleContacts.value;
  } else {
    list = visibleContacts.value.filter((c) => c.hostname === selectedKey.value);
  }
  if (!searchText.value) return list;
  const q = searchText.value.toLowerCase();
  return list.filter(
    (c) =>
      resolveDisplayName(c.address).toLowerCase().includes(q) ||
      c.hostname.toLowerCase().includes(q),
  );
});

const conversation = computed(() => {
  const sel = selectedContact.value;
  if (!sel) return [];
  const ownName = ownAddress.value;
  const seen = new Set<string>();
  const result: any[] = [];
  const pending = store.pendingMessages.filter((p) => p.to === sel || p.from === sel);
  for (const p of pending) {
    if (p.status === "sent") continue;
    result.push({ ...p, text: p.text, timestamp: p.timestamp, from: p.from, to: p.to, __pending: true, __status: p.status });
  }
  for (const m of store.sortedMessages) {
    if (seen.has(m.id)) continue;
    const fromName = nameFromAddr(m.from);
    const toName = nameFromAddr(m.to);
    const selName = nameFromAddr(sel);
    if (fromName === ownName && toName === ownName && sel === ownName) { seen.add(m.id); result.push(m); }
    else if (nameFromAddr(m.from) === selName || nameFromAddr(m.to) === selName || m.from === sel || m.to === sel) { seen.add(m.id); result.push(m); }
  }
  return result.sort((a, b) => a.timestamp - b.timestamp);
});

function resolveDisplayName(addr: string) {
  const mapping = (store.config?.plugin_mappings ?? []).find((m) => m.virtual_addr === addr);
  if (mapping?.display_name) return mapping.display_name;
  const p = parseAddress(addr);
  return p.name || addr;
}

function selectContact(addr: string) { selectedContact.value = addr; store.markRead(addr); }

// Pull-to-refresh on contact list
function onPullStart(e: TouchEvent) {
  if (!contactListRef.value || contactListRef.value.scrollTop > 0) return;
  pullStartY = e.touches[0].clientY;
}
function onPullMove(e: TouchEvent) {
  if (!pullStartY) return;
  const delta = e.touches[0].clientY - pullStartY;
  if (delta <= 0) { pullOffset.value = 0; return; }
  pullOffset.value = Math.min(delta * 0.5, 100);
}
async function onPullEnd() {
  if (pullOffset.value > 60 && !pullRefreshing.value) {
    pullRefreshing.value = true;
    await store.loadMessages();
    pullRefreshing.value = false;
  }
  pullOffset.value = 0; pullStartY = 0;
}

// Swipe-back on message area
function onSwipeStart(e: TouchEvent) {
  msgTouchStartX = e.touches[0].clientX;
}
function onSwipeMove(e: TouchEvent) {
  const dx = e.touches[0].clientX - msgTouchStartX;
  const dy = e.touches[0].clientY - msgTouchStartY;
  if (Math.abs(dx) > 20 && Math.abs(dx) > Math.abs(dy)) {
    msgTouchIsSwipe = true;
  }
}
function onSwipeEnd(e: TouchEvent) {
  if (msgTouchIsSwipe && e.changedTouches[0].clientX - msgTouchStartX > 100 && isMobile.value && selectedContact.value) {
    selectedContact.value = "";
  }
  msgTouchStartX = 0; msgTouchStartY = 0; msgTouchIsSwipe = false;
}

async function handleSend() {
  if (!inputText.value.trim() || !selectedContact.value) return;
  try {
    await store.sendMessage(selectedContact.value, inputText.value.trim());
    inputText.value = "";
    await scrollToBottom();
  } catch (e) { MessagePlugin.error(`发送失败: ${e}`).catch(() => {}); }
}

async function handleComponentResponse(msg: { from: string; to: string }, value: string) {
  const contact = nameFromAddr(msg.from) === ownAddress.value ? msg.to : msg.from;
  await store.handlePluginResponse(contact, "", value);
  await scrollToBottom();
}

function isPending(msg: any): boolean { return !!(msg as any).__pending; }
function retryMessage(msg: any) { store.retryMessage(msg); }

async function scrollToBottom() {
  await nextTick();
  if (messagesContainer.value) { messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight; }
}
watch(selectedContact, scrollToBottom);

watch(selectedContact, (newVal, oldVal) => {
  if (!newVal && oldVal && isMobile.value) { store.markRead(oldVal); }
});

onMounted(async () => {
  await store.loadMessages();
  store.listenForMessages();
  store.listenForLogs();
});
</script>

<style scoped>
.chat-shell { display: flex; flex-direction: column; height: 100%; }
.chat-body { flex: 1; display: flex; overflow: hidden; }

.contact-panel {
  width: 280px; min-width: 280px; display: flex; flex-direction: column;
  border-right: 1px solid var(--td-component-stroke); background: var(--td-bg-color-container);
}
.panel-header { display: flex; align-items: center; padding: 12px 12px 4px; }
.panel-left { display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; }
.panel-icon { width: 28px; height: 28px; border-radius: 6px; background: rgba(0,0,0,0.6); padding: 4px; flex-shrink: 0; }
.panel-title { font-size: 16px; font-weight: 600; }
.panel-mark-read {
  display: inline-flex; align-items: center; justify-content: center;
  width: 30px; height: 30px; border: none; border-radius: 15px;
  background: transparent; color: var(--td-text-color-placeholder);
  cursor: pointer; transition: all 0.15s; font-family: inherit; flex-shrink: 0;
}
.panel-mark-read:hover { background: var(--td-bg-color-secondarycontainer); color: var(--td-brand-color); }
.panel-mark-read.has-unread { color: var(--td-success-color); background: var(--td-success-color-1); }

.search-input { margin: 4px 8px; width: calc(100% - 16px); box-sizing: border-box; }
.contact-list {
  flex: 1; overflow-y: auto; padding: 4px 0;
  scrollbar-width: thin; scrollbar-color: transparent transparent;
}
.contact-list:hover { scrollbar-color: var(--td-component-stroke) transparent; }
.contact-list::-webkit-scrollbar { width: 6px; }
.contact-list::-webkit-scrollbar-thumb { background: transparent; border-radius: 3px; }
.contact-list:hover::-webkit-scrollbar-thumb { background: var(--td-component-stroke); }

.hidden-section { border-top: 1px solid var(--td-component-stroke); margin-top: 4px; }
.hidden-toggle { display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; cursor: pointer; font-size: 13px; color: var(--td-text-color-placeholder); }
.toggle-arrow { display: flex; align-items: center; }
.hidden-item { opacity: 0.5; }

.chat-panel { flex: 1; display: flex; flex-direction: column; background: var(--td-bg-color-page); }
.chat-panel.chat-empty { align-items: center; justify-content: center; }
.chat-topbar { display: flex; align-items: center; padding: 10px 16px; border-bottom: 1px solid var(--td-component-stroke); background: var(--td-bg-color-container); flex-shrink: 0; }
.back-btn { font-size: 20px; cursor: pointer; line-height: 1; user-select: none; color: var(--td-brand-color); display: flex; align-items: center; padding: 0 8px 0 0; }
.topbar-name { font-size: 16px; font-weight: 600; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.topbar-more { cursor: pointer; padding: 0 4px; border-radius: 6px; font-size: 18px; color: var(--td-text-color-secondary); display: flex; align-items: center; margin-left: auto; transition: background 0.2s; }
.topbar-more:hover { background: var(--td-bg-color-secondarycontainer); }

.message-area { flex: 1; overflow-y: auto; padding: 4px 20px 16px; display: flex; flex-direction: column; gap: 8px; }
.pull-indicator { display: flex; align-items: center; justify-content: center; overflow: hidden; transition: height 0.15s ease; flex-shrink: 0; }
.pull-text { font-size: 12px; color: var(--td-text-color-placeholder); }
.pull-spinner { width: 18px; height: 18px; border: 2px solid var(--td-component-stroke); border-top-color: var(--td-brand-color); border-radius: 50%; animation: pullSpin 0.6s linear infinite; }
@keyframes pullSpin { to { transform: rotate(360deg); } }

.msg-row {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  min-width: 0;
}

/* 1. 限制外层最大宽度 */
.msg-row > * {
  min-width: 0;
  max-width: 75%;
}

/* 2. 使用 margin-left: auto 替代 justify-content，强制吸收左侧空间 */
.row-self > * {
  margin-left: auto;
}

/* 3. 核心修复：强制打断超长无空格字符，防止撑破 max-width */
.msg-row :deep(.msg-text) {
  overflow-wrap: anywhere;
  word-break: break-word;
}

/* 确保气泡本身也遵守宽度限制并 100% 填充外层包裹层 */
.msg-row :deep(.msg-bubble) {
  max-width: 100%;
  display: inline-block;
}

.row-other { justify-content: flex-start; }

@media (max-width: 767px) {
  .contact-panel { width: 100%; min-width: 0; }
  .contact-overlay { display: none; }
  .chat-panel { position: fixed; inset: 0; z-index: 10; padding-top: env(safe-area-inset-top, 0); background: var(--td-bg-color-page); }
  .chat-full { display: flex !important; }
  .message-area { padding-bottom: calc(16px + env(safe-area-inset-bottom, 0px)); }
}
@media (min-width: 768px) {
  .chat-shell .t-card { margin: 16px; }
}
</style>
