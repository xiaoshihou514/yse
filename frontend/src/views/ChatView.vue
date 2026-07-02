<template>
  <div class="chat-layout">
    <!-- Header -->
    <div class="chat-header">
      <t-space>
        <t-select
          v-model="selectedContact"
          placeholder="选择联系人"
          style="width: 200px"
          :options="contacts"
          clearable
        />
        <t-button
          :icon="polling ? 'poweroff' : 'play-circle'"
          :theme="polling ? 'danger' : 'primary'"
          @click="togglePolling"
        >
          {{ polling ? '停止' : '开始轮询' }}
        </t-button>
        <t-tag v-if="connected" theme="success">已连接</t-tag>
        <t-tag v-else theme="default">未连接</t-tag>
      </t-space>
    </div>

    <!-- Messages -->
    <div class="chat-messages" ref="messagesContainer">
      <t-empty v-if="displayMessages.length === 0" description="暂无消息" />
      <div
        v-for="msg in displayMessages"
        :key="msg.id"
        :class="['msg-bubble', msg.from === ownAddress ? 'msg-self' : 'msg-other']"
      >
        <div class="msg-meta">
          <span class="msg-from">{{ msg.from }}</span>
          <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
        </div>
        <div class="msg-text" v-if="msg.text">{{ msg.text }}</div>
        <div class="msg-files" v-if="msg.files?.length">
          <t-link v-for="f in msg.files" :key="f.enc_name" theme="primary">
            {{ f.name }} ({{ formatSize(f.size) }})
          </t-link>
        </div>
      </div>
    </div>

    <!-- Input -->
    <div class="chat-input">
      <t-textarea
        v-model="inputText"
        placeholder="输入消息..."
        :autosize="{ minRows: 2, maxRows: 6 }"
        @keydown.ctrl.enter="handleSend"
      />
      <t-button
        style="margin-top: 8px"
        :disabled="!inputText.trim() || !selectedContact"
        @click="handleSend"
      >
        发送 (Ctrl+Enter)
      </t-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from "vue";
import { useYseStore } from "@/stores/yse";

const store = useYseStore();
const inputText = ref("");
const selectedContact = ref("");
const messagesContainer = ref<HTMLElement | null>(null);

const contacts = computed(() => {
  const set = new Set<string>();
  store.messages.forEach((m) => {
    set.add(m.from);
    set.add(m.to);
  });
  return Array.from(set)
    .filter((a) => a !== store.config?.own_address)
    .map((a) => ({ label: a, value: a }));
});

const ownAddress = computed(() => store.config?.own_address ?? "me@yse.org");
const polling = computed(() => store.polling);
const connected = computed(() => store.connected);

const displayMessages = computed(() => {
  const addr = selectedContact.value;
  if (!addr) return store.sortedMessages;
  return store.sortedMessages.filter(
    (m) => m.from === addr || m.to === addr,
  );
});

function formatTime(ts: number) {
  return new Date(ts).toLocaleString("zh-CN");
}

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

async function handleSend() {
  if (!inputText.value.trim() || !selectedContact.value) return;
  await store.sendMessage(selectedContact.value, inputText.value.trim());
  inputText.value = "";
  await scrollToBottom();
}

async function togglePolling() {
  if (polling.value) {
    await store.stopPolling();
  } else {
    await store.startPolling();
  }
}

async function scrollToBottom() {
  await nextTick();
  if (messagesContainer.value) {
    messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
  }
}

onMounted(async () => {
  await store.loadMessages();
  await scrollToBottom();
});
</script>

<style scoped>
.chat-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.chat-header {
  padding-bottom: 16px;
  border-bottom: 1px solid var(--td-component-stroke);
  margin-bottom: 16px;
}
.chat-messages {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 8px 0;
}
.msg-bubble {
  max-width: 70%;
  padding: 10px 14px;
  border-radius: 8px;
  word-break: break-word;
}
.msg-self {
  align-self: flex-end;
  background: var(--td-brand-color-light);
}
.msg-other {
  align-self: flex-start;
  background: var(--td-bg-color-secondarycontainer);
}
.msg-meta {
  font-size: 12px;
  margin-bottom: 4px;
  display: flex;
  justify-content: space-between;
  gap: 12px;
}
.msg-from {
  font-weight: 600;
  color: var(--td-text-color-primary);
}
.msg-time {
  color: var(--td-text-color-placeholder);
}
.msg-text {
  font-size: 14px;
}
.msg-files {
  margin-top: 6px;
}
.chat-input {
  border-top: 1px solid var(--td-component-stroke);
  padding-top: 12px;
}
</style>
