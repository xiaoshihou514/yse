<template>
  <div>
    <div
      :class="['msg-bubble', alignment === 'self' ? 'bubble-self' : 'bubble-other']"
      @contextmenu.prevent="$emit('bubbleContext', $event, message)"
      @touchstart.passive="$emit('bubbleTouchStart', $event, message)"
      @touchend="$emit('bubbleTouchEnd')"
      @touchmove="$emit('bubbleTouchMove', $event)"
    >
      <div
        class="msg-text"
        v-if="message.text"
        v-html="renderMarkdown(message.text)"
      ></div>
      <PluginComponent
        v-if="(message.meta as any)?.plugin?.component"
        :comp="(message.meta as any).plugin.component"
        @respond="$emit('respond', $event)"
      />
      <div class="msg-files" v-if="message.files?.length">
        <t-link
          v-for="f in message.files"
          :key="f.enc_name"
          theme="primary"
          size="small"
        >
          {{ f.name }} ({{ formatSize(f.size) }})
        </t-link>
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
  </div>
</template>

<script setup lang="ts">
import PluginComponent from "./PluginComponent.vue";
import { renderMarkdown } from "@/composables/useMarkdown";

interface Message {
  text?: string;
  timestamp: number;
  files?: { name: string; enc_name: string; size: number }[];
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
</script>

<style scoped>
.msg-bubble {
  max-width: 75%; padding: 8px 12px; border-radius: 12px;
  overflow-wrap: anywhere; word-break: break-word;
  position: relative;
}
.bubble-self {
  background: var(--td-brand-color); color: #fff; border-bottom-right-radius: 4px;
}
.bubble-other {
  background: var(--td-bg-color-secondarycontainer); border-bottom-left-radius: 4px;
}
.msg-text { font-size: 15px; line-height: 1.6; overflow-wrap: break-word; }
.msg-text :deep(pre) { margin: 6px 0; padding: 8px 10px; border-radius: 6px; overflow-x: auto; font-size: 13px; background: var(--td-bg-color-component); }
.msg-text :deep(code) { font-family: ui-monospace, monospace; font-size: 0.9em; }
.msg-text :deep(p) { margin: 4px 0; }
.msg-text :deep(ul), .msg-text :deep(ol) { padding-left: 20px; margin: 4px 0; }
.msg-text :deep(blockquote) { margin: 4px 0; padding-left: 10px; border-left: 3px solid var(--td-brand-color); color: var(--td-text-color-placeholder); }
.msg-text :deep(a) { color: var(--td-brand-color); }
.msg-text :deep(table) { border-collapse: collapse; margin: 6px 0; }
.msg-text :deep(th), .msg-text :deep(td) { border: 1px solid var(--td-component-stroke); padding: 4px 8px; }
.msg-files { margin-top: 4px; display: flex; flex-direction: column; gap: 2px; }
.msg-time { font-size: 11px; margin-top: 4px; opacity: 0.7; }
.msg-indicator { display: flex; align-items: center; }
.msg-spinner { width: 14px; height: 14px; border: 2px solid var(--td-component-stroke); border-top-color: var(--td-brand-color); border-radius: 50%; animation: spin 0.6s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.msg-retry { cursor: pointer; font-size: 14px; }
</style>
