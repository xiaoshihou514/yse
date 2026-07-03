<template>
  <div class="log-page">
    <t-card title="运行日志" :bordered="false">
      <template #actions>
        <t-space>
          <t-button size="small" @click="refresh">刷新</t-button>
          <t-button size="small" theme="danger" @click="clear">清空</t-button>
          <t-select
            v-model="levelFilter"
            style="width: 120px"
            :options="levelOptions"
            clearable
          />
        </t-space>
      </template>

      <div class="log-container" ref="logContainer">
        <div v-if="filteredLogs.length === 0" style="text-align: center; color: var(--td-text-color-placeholder); padding: 40px">
          暂无日志
        </div>
        <div
          v-for="(entry, i) in filteredLogs"
          :key="i"
          :class="['log-entry', `log-${entry.level}`]"
        >
          <span class="log-time">{{ formatTime(entry.timestamp) }}</span>
          <t-tag :theme="tagTheme(entry.level)" size="small">{{ entry.level.toUpperCase() }}</t-tag>
          <span class="log-msg">{{ entry.message }}</span>
        </div>
      </div>
    </t-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from "vue";
import type { LogEntry } from "@/api/commands";
import { useYseStore } from "@/stores/yse";

const store = useYseStore();
const levelFilter = ref<string | undefined>(undefined);
const logContainer = ref<HTMLElement | null>(null);

const levelOptions = [
  { label: "DEBUG", value: "debug" },
  { label: "INFO", value: "info" },
  { label: "WARN", value: "warn" },
  { label: "ERROR", value: "error" },
];

const filteredLogs = computed(() => {
  if (!levelFilter.value) return store.logs;
  return store.logs.filter((l) => l.level === levelFilter.value);
});

function formatTime(ts: number) {
  return new Date(ts).toLocaleString("zh-CN");
}

function tagTheme(level: string) {
  switch (level) {
    case "error": return "danger";
    case "warn": return "warning";
    case "info": return "primary";
    default: return "default";
  }
}

async function scrollToBottom() {
  await nextTick();
  if (logContainer.value) {
    logContainer.value.scrollTop = logContainer.value.scrollHeight;
  }
}

async function refresh() {
  await store.loadLogs();
  await scrollToBottom();
}

function clear() {
  store.logs.splice(0);
}

// Auto-scroll when new log entries arrive
watch(() => store.logs.length, scrollToBottom);

onMounted(async () => {
  await refresh();
  store.listenForLogs();
});
</script>

<style scoped>
.log-page {
  max-width: 1000px;
}
.log-container {
  max-height: 600px;
  overflow-y: auto;
  font-family: "Cascadia Code", "JetBrains Mono", "Fira Code", monospace;
  font-size: 13px;
  line-height: 1.6;
}
.log-entry {
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 2px 0;
  border-bottom: 1px solid var(--td-component-stroke);
}
.log-time {
  color: var(--td-text-color-placeholder);
  min-width: 160px;
  flex-shrink: 0;
}
.log-msg {
  flex: 1;
  word-break: break-all;
}
</style>
