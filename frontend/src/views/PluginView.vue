<template>
  <div class="plugin-page">
    <t-card title="插件管理" :bordered="false">
      <t-table
        :data="store.plugins"
        :columns="columns"
        row-key="id"
        :loading="loading"
      >
        <template #name="{ row }">
          <span :data-label="'名称'">{{ row.name }}</span>
        </template>
        <template #id="{ row }">
          <span :data-label="'ID'">{{ row.id }}</span>
        </template>
        <template #exec_path="{ row }">
          <span :data-label="'路径'">{{ row.exec_path }}</span>
        </template>
        <template #status="{ row }">
          <span :data-label="'状态'">
            {{ statusFor(row.id) }}
          </span>
        </template>
        <template #enabled="{ row }">
          <span :data-label="'启用'">
            <t-switch
              :value="row.enabled"
              @change="(v: boolean) => handleToggle(row, v)"
            />
          </span>
        </template>
        <template #operation="{ row }">
          <t-popconfirm content="确定删除此插件？" @confirm="handleDelete(row)">
            <t-button theme="danger" variant="text">删除</t-button>
          </t-popconfirm>
        </template>
      </t-table>

      <t-divider />

      <t-space>
        <t-input v-model="newName" placeholder="插件名称" />
        <t-input v-model="newExec" placeholder="可执行文件路径" style="width: 300px" />
        <t-button variant="outline" @click="pickFile">选择文件</t-button>
        <t-button @click="handleAdd">添加</t-button>
      </t-space>
    </t-card>

    <t-card title="运行状态" :bordered="false" style="margin-top: 20px">
      <div v-if="store.processes.length" class="process-list">
        <div v-for="p in store.processes" :key="p.id" class="process-item">
          <div class="proc-header">
            <span class="proc-name">{{ p.name || p.id }}</span>
            <t-tag
              :theme="tagTheme(p.state)"
              size="small"
              variant="light"
            >{{ p.state }}</t-tag>
          </div>
          <div class="proc-meta">
            <span v-if="p.start_time">启动于 {{ formatTime(p.start_time) }}</span>
            <span v-if="p.restart_count > 0">重启 {{ p.restart_count }} 次</span>
            <span v-if="p.last_exit">最后退出: {{ p.last_exit }}</span>
          </div>
        </div>
      </div>
      <t-empty v-else description="暂无运行中的插件" />
    </t-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import type { PluginConfig } from "@/api/commands";

const store = useYseStore();
const loading = ref(false);
const newName = ref("");
const newExec = ref("");

const columns = [
  { colKey: "name", title: "名称" },
  { colKey: "id", title: "ID" },
  { colKey: "exec_path", title: "路径" },
  { colKey: "status", title: "状态" },
  { colKey: "enabled", title: "启用" },
  { colKey: "operation", title: "操作" },
];

function statusFor(id: string): string {
  const p = store.processes.find((pr) => pr.id === id);
  if (!p) return "Stopped";
  return p.state;
}

function tagTheme(state: string): "success" | "warning" | "danger" | "default" {
  if (state === "Running") return "success";
  if (state === "Starting" || state === "Stopping") return "warning";
  if (state?.startsWith("Crashed")) return "danger";
  return "default";
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleString("zh-CN");
}

async function handleToggle(row: PluginConfig, enabled: boolean) {
  try {
    await store.togglePlugin(row.id, enabled);
    await store.loadProcesses();
    await MessagePlugin.success(`${enabled ? "已启动" : "已停止"} ${row.name}`);
  } catch (e) {
    await MessagePlugin.error(`操作失败: ${e}`);
  }
}

async function handleDelete(row: PluginConfig) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await store.togglePlugin(row.id, false);
    await invoke("remove_plugin", { id: row.id });
    await store.loadPlugins();
    await store.loadProcesses();
    await MessagePlugin.success(`已删除 ${row.name}`);
  } catch (e) {
    await MessagePlugin.error(`删除失败: ${e}`);
  }
}

async function handleAdd() {
  if (!newName.value || !newExec.value) {
    await MessagePlugin.warning("请填写插件名称和执行路径");
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("add_plugin", {
      name: newName.value,
      execPath: newExec.value,
    });
    newName.value = "";
    newExec.value = "";
    await store.loadPlugins();
    await store.loadConfig();
    await store.loadProcesses();
    await MessagePlugin.success("插件已添加");
  } catch (e) {
    await MessagePlugin.error(`添加失败: ${e}`);
  }
}

async function pickFile() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ multiple: false, title: "选择可执行文件" });
    if (selected) newExec.value = selected;
  } catch { /* not in tauri */ }
}

onMounted(async () => {
  loading.value = true;
  await store.loadPlugins();
  await store.loadProcesses();
  loading.value = false;
});
</script>

<style scoped>
.process-list { display: flex; flex-direction: column; gap: 8px; }
.process-item {
  padding: 10px 12px;
  border: 1px solid var(--td-component-stroke);
  border-radius: 8px;
  background: var(--td-bg-color-container);
}
.proc-header {
  display: flex; align-items: center; gap: 8px;
}
.proc-name { font-size: 14px; font-weight: 500; }
.proc-meta {
  font-size: 12px; color: var(--td-text-color-placeholder);
  margin-top: 4px; display: flex; gap: 12px; flex-wrap: wrap;
}

@media (max-width: 767px) {
  .plugin-page .t-card { margin: 8px; }
  .plugin-page :deep(.t-table) { font-size: 12px; }
  .plugin-page :deep(.t-table thead) { display: none; }
  .plugin-page :deep(.t-table tbody tr) {
    display: block; margin-bottom: 12px;
    border: 1px solid var(--td-component-stroke);
    border-radius: 8px; padding: 12px;
    background: var(--td-bg-color-container);
  }
  .plugin-page :deep(.t-table tbody td) {
    display: flex; justify-content: space-between; align-items: center;
    padding: 6px 0 !important; border: none !important;
  }
  .plugin-page :deep(.t-table tbody td > span) {
    display: flex; justify-content: space-between; align-items: center; width: 100%;
  }
  .plugin-page :deep(.t-table tbody td > span::before) {
    content: attr(data-label); font-weight: 600;
    color: var(--td-text-color-placeholder); font-size: 12px;
  }
  .plugin-page :deep(.t-table tbody td:last-child > span::before) { display: none; }
  .plugin-page :deep(.t-table tbody td:last-child) { justify-content: flex-end; }
  .plugin-page :deep(.t-table tbody td:last-child > span) { justify-content: flex-end; }
  .plugin-page :deep(.t-table__pagination) { padding-top: 8px; }
  .plugin-page .t-card .t-space,
  .plugin-page .t-card .t-space > .t-space-item {
    display: flex; flex-direction: column; width: 100%;
  }
  .plugin-page .t-card :deep(.t-space .t-input__wrap),
  .plugin-page .t-card :deep(.t-space .t-button) { width: 100%; }
}
</style>
