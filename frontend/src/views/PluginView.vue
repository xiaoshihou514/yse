<template>
  <div class="plugin-page">
    <!-- Desktop: table -->
    <t-card v-if="!isMobile" title="插件管理" :bordered="false">
      <t-table
        :data="store.plugins"
        :columns="columns"
        row-key="id"
        :loading="loading"
      >
        <template #name="{ row }">
          <span>{{ row.name }}</span>
        </template>
        <template #exec_path="{ row }">
          <span class="path-cell">{{ row.exec_path }}</span>
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

    <!-- Mobile: card list -->
    <template v-else>
      <div class="mobile-header">
        <h2 class="mobile-title">插件管理</h2>
      </div>
      <div class="plugin-cards">
        <div v-for="plugin in store.plugins" :key="plugin.id" class="plugin-card">
          <div class="card-top">
            <div class="card-info">
              <span class="card-name">{{ plugin.name }}</span>
              <span class="card-path">{{ plugin.exec_path }}</span>
            </div>
            <t-tag
              :theme="procTag(procState(plugin.id))"
              size="small"
              variant="light"
            >{{ procState(plugin.id) || '未启动' }}</t-tag>
          </div>
          <div class="card-actions">
            <t-popconfirm content="确定删除此插件？" @confirm="handleDelete(plugin)">
              <t-button theme="danger" variant="text" size="small">删除</t-button>
            </t-popconfirm>
          </div>
        </div>
        <t-empty v-if="!store.plugins.length" description="暂无插件" />
      </div>

      <!-- FAB -->
      <t-button class="fab" shape="circle" size="large" @click="showAdd = true">
        <template #icon><span class="fab-icon">+</span></template>
      </t-button>

      <!-- Add dialog -->
      <t-dialog v-model:visible="showAdd" header="添加插件" :footer="false" width="360px" :close-on-overlay-click="true">
        <t-form>
          <t-form-item label="名称">
            <t-input v-model="newName" placeholder="如 echo-bot" />
          </t-form-item>
          <t-form-item label="执行路径">
            <t-input v-model="newExec" placeholder="/usr/local/bin/echo-bot" />
            <template #help>
              <t-button size="small" variant="outline" @click="pickFile">选择文件</t-button>
            </template>
          </t-form-item>
          <t-form-item>
            <t-button block @click="handleAdd">添加</t-button>
          </t-form-item>
        </t-form>
      </t-dialog>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import * as api from "@/api/commands";
import type { PluginConfig } from "@/api/commands";

const isMobile = useIsMobile();
const store = useYseStore();
const loading = ref(false);
const newName = ref("");
const newExec = ref("");
const showAdd = ref(false);

const columns = [
  { colKey: "name", title: "名称" },
  { colKey: "exec_path", title: "路径" },
  { colKey: "operation", title: "操作" },
];

function procState(pluginId: string): string | undefined {
  return store.processes.find((p) => p.id === pluginId)?.state;
}

function procTag(state: string | undefined): "success" | "warning" | "danger" | "default" {
  if (state === "Running") return "success";
  if (state === "Starting" || state === "Stopping") return "warning";
  if (state?.startsWith("Crashed")) return "danger";
  return "default";
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleString("zh-CN");
}

async function handleDelete(row: PluginConfig) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await api.stopPlugin(row.id).catch(() => {});
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
    showAdd.value = false;
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
.path-cell {
  max-width: 240px;
  display: inline-block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: middle;
}

.mobile-header {
  padding: 16px 16px 0;
}
.mobile-title {
  font-size: 20px;
  font-weight: 600;
  margin: 0;
}

.plugin-cards {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 16px 80px;
}
.plugin-card {
  background: var(--td-bg-color-container);
  border-radius: 10px;
  padding: 14px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.08);
}
.card-top {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 8px;
}
.card-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.card-name {
  font-size: 15px;
  font-weight: 500;
  color: var(--td-text-color-primary);
}
.card-path {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-actions {
  margin-top: 8px;
  display: flex;
  justify-content: flex-end;
}

.fab {
  position: fixed;
  bottom: 64px;
  right: 16px;
  z-index: 900;
  width: 52px;
  height: 52px;
  border-radius: 50%;
  box-shadow: 0 4px 12px rgba(0,0,0,0.25);
}
.fab-icon {
  font-size: 24px;
  line-height: 1;
}

@media (min-width: 768px) {
  .plugin-page .t-card { margin: 16px; }
}
</style>
