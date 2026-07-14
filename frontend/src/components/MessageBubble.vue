<template>
  <div>
    <div
      :class="[
        'msg-bubble',
        alignment === 'self' ? 'bubble-self' : 'bubble-other',
      ]"
      @contextmenu.prevent="$emit('bubbleContext', $event, message)"
      @touchstart.passive="$emit('bubbleTouchStart', $event, message)"
      @touchend="$emit('bubbleTouchEnd')"
      @touchmove="$emit('bubbleTouchMove', $event)"
    >
      <div
        class="msg-text"
        v-if="message.text"
        v-html="renderMarkdown(message.text)"
        @click="handleLinkClick"
      ></div>
      <PluginComponent
        v-if="(message.meta as any)?.plugin?.component"
        :comp="(message.meta as any).plugin.component"
        @respond="$emit('respond', $event)"
      />
      <div class="msg-files" v-if="message.files?.length">
        <template v-for="f in message.files" :key="f.enc_name">
          <img
            v-if="isImage(f.mime) && thumbUrls[f.enc_name]"
            :src="thumbUrls[f.enc_name]"
            :alt="f.name"
            class="msg-img"
            @click="openViewer(f)"
            @error="onImgError(f.enc_name)"
          />
          <t-link
            v-else
            theme="primary"
            size="small"
            @click.prevent="downloadFile(message.id, f.enc_name, f.name)"
          >
            {{ f.name }} ({{ formatSize(f.size) }})
          </t-link>
        </template>
      </div>
      <div class="msg-time">{{ formatTime(message.timestamp) }}</div>
    </div>
    <div v-if="isPending" class="msg-indicator">
      <span v-if="pendingStatus === 'sending'" class="msg-spinner"></span>
      <span
        v-else-if="pendingStatus === 'failed'"
        class="msg-retry"
        @click.stop="$emit('retry', pendingMsg)"
        title="点击重试"
        >⚠</span
      >
    </div>

    <div v-if="viewerFile" class="viewer-overlay" @click.self="closeViewer">
      <button class="viewer-close" @click="closeViewer">✕</button>
      <img v-if="viewerSrc" :src="viewerSrc" class="viewer-img" @click.stop />
      <div v-else class="viewer-loading">加载中...</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from "vue";
import PluginComponent from "./PluginComponent.vue";
import { renderMarkdown, handleLinkClick } from "@/composables/useMarkdown";
import { readAttachment } from "@/api/commands";

interface FileItem {
  name: string;
  mime: string;
  size: number;
  enc_name: string;
}

interface Message {
  id: string;
  text?: string;
  timestamp: number;
  files?: FileItem[];
  meta?: unknown;
}

const props = defineProps<{
  message: Message;
  alignment?: "self" | "other";
  isPending: boolean;
  pendingStatus?: "sending" | "failed";
  pendingMsg?: unknown;
}>();

defineEmits<{
  bubbleContext: [e: MouseEvent, msg: Message];
  bubbleTouchStart: [e: TouchEvent, msg: Message];
  bubbleTouchEnd: [];
  bubbleTouchMove: [e: TouchEvent];
  respond: [value: string];
  retry: [msg: unknown];
}>();

const thumbUrls = reactive<Record<string, string>>({});
const viewerFile = ref<FileItem | null>(null);
const viewerSrc = ref("");

function isImage(mime: string): boolean {
  return mime.startsWith("image/");
}

async function loadThumb(messageId: string, encName: string, fileName: string) {
  if (thumbUrls[encName]) return;
  try {
    const data = await readAttachment(messageId, encName);
    const blob = new Blob([new Uint8Array(data)]);
    thumbUrls[encName] = URL.createObjectURL(blob);
  } catch {
    // fall through — thumb stays broken
  }
}

function onImgError(encName: string) {
  // Clear broken thumb URL so v-if hides it
  delete thumbUrls[encName];
}

async function openViewer(f: FileItem) {
  viewerFile.value = f;
  viewerSrc.value = "";
  try {
    const data = await readAttachment(props.message.id, f.enc_name);
    const blob = new Blob([new Uint8Array(data)], { type: f.mime });
    viewerSrc.value = URL.createObjectURL(blob);
  } catch {
    viewerSrc.value = "";
  }
}

function closeViewer() {
  if (viewerSrc.value) URL.revokeObjectURL(viewerSrc.value);
  viewerFile.value = null;
  viewerSrc.value = "";
}

// Load thumbs for image files on mount
for (const f of props.message.files ?? []) {
  if (isImage(f.mime)) {
    loadThumb(props.message.id, f.enc_name, f.name);
  }
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

async function downloadFile(
  messageId: string,
  encName: string,
  fileName: string,
) {
  try {
    const data = await readAttachment(messageId, encName);
    const blob = new Blob([new Uint8Array(data)]);
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = fileName;
    a.click();
    URL.revokeObjectURL(url);
  } catch {
    const { MessagePlugin } = await import("tdesign-vue-next");
    MessagePlugin.error("附件下载失败：该文件可能已被删除或尚未下载完成").catch(
      () => {},
    );
  }
}
</script>

<style scoped>
.msg-bubble {
  max-width: 70%;
  padding: 8px 12px;
  border-radius: 12px;
  overflow-wrap: anywhere;
  position: relative;
}
.bubble-self {
  background: var(--td-brand-color);
  color: #fff;
  border-bottom-right-radius: 4px;
}
.bubble-other {
  background: var(--td-bg-color-secondarycontainer);
  border-bottom-left-radius: 4px;
}
.msg-text {
  font-size: 15px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.msg-text :deep(pre) {
  margin: 6px 0;
  padding: 8px 10px;
  border-radius: 6px;
  overflow-x: auto;
  font-size: 13px;
  background: var(--td-bg-color-component);
  white-space: pre-wrap;
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
  padding-left: 20px;
  margin: 4px 0;
}
.msg-text :deep(blockquote) {
  margin: 4px 0;
  padding-left: 10px;
  border-left: 3px solid var(--td-brand-color);
  color: var(--td-text-color-placeholder);
}
.msg-text :deep(a) {
  color: var(--td-brand-color);
}
.msg-text :deep(.details-block) {
  margin: 8px 0;
  border: 1px solid var(--td-component-stroke);
  border-radius: 8px;
  overflow: hidden;
}
.msg-text :deep(.details-summary) {
  padding: 6px 10px;
  cursor: pointer;
  font-size: 13px;
  opacity: 0.7;
  user-select: none;
  background: var(--td-bg-color-component);
}
.msg-text :deep(.details-content) {
  padding: 8px 10px;
  border-top: 1px solid var(--td-component-stroke);
}
.msg-text :deep(.details-content pre) {
  margin: 4px 0;
}
.msg-text :deep(table) {
  border-collapse: collapse;
  margin: 6px 0;
}
.msg-text :deep(th),
.msg-text :deep(td) {
  border: 1px solid var(--td-component-stroke);
  padding: 4px 8px;
}
.msg-files {
  margin-top: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.msg-img {
  max-width: 200px;
  max-height: 200px;
  border-radius: 6px;
  cursor: pointer;
  object-fit: cover;
  margin-top: 4px;
}
.msg-time {
  font-size: 11px;
  margin-top: 4px;
  opacity: 0.7;
}
.msg-indicator {
  display: flex;
  align-items: center;
}
.msg-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--td-component-stroke);
  border-top-color: var(--td-brand-color);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
.msg-retry {
  cursor: pointer;
  font-size: 14px;
}

.viewer-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.9);
}
.viewer-close {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 10000;
  background: rgba(255, 255, 255, 0.2);
  border: none;
  color: #fff;
  font-size: 24px;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}
.viewer-img {
  max-width: 95vw;
  max-height: 95vh;
  object-fit: contain;
}
.viewer-loading {
  color: #fff;
  font-size: 16px;
}
</style>
