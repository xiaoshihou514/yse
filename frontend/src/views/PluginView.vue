<template>
  <div class="plugin-page">
    <t-card title="插件管理" :bordered="false">
      <t-table
        :data="store.plugins"
        :columns="pluginColumns"
        row-key="id"
        :loading="loading"
      >
        <template #enabled="{ row }">
          <t-switch
            :value="row.enabled"
            @change="(v: boolean) => handleToggle(row, v)"
          />
        </template>
        <template #operation="{ row }">
          <t-popconfirm content="确定删除此插件？" @confirm="handleDelete(row)">
            <t-button theme="danger" variant="text">删除</t-button>
          </t-popconfirm>
        </template>
      </t-table>

      <t-divider />

      <t-space>
        <t-input v-model="newId" placeholder="插件 ID" />
        <t-input v-model="newName" placeholder="显示名称" />
        <t-input v-model="newExec" placeholder="可执行文件路径" style="width: 300px" />
        <t-button variant="outline" @click="pickFile">选择文件</t-button>
        <t-button @click="handleAdd">添加插件</t-button>
      </t-space>
    </t-card>

    <t-card title="消息映射" :bordered="false" style="margin-top: 20px">
      <p class="card-desc">配置虚拟地址 → 插件的分发规则</p>
      <t-table :data="mappings" :columns="mappingColumns" row-key="virtual_addr" size="small">
        <template #operation="{ row }">
          <t-button theme="danger" variant="text" @click="removeMapping(row)">删除</t-button>
        </template>
      </t-table>
      <t-space style="margin-top: 12px">
        <t-input v-model="newMappingAddr" placeholder="虚拟地址 (如 echo@yse.org)" style="width: 240px" />
        <t-select v-model="newMappingPlugin" placeholder="选择插件" style="width: 200px" :options="pluginOptions" />
        <t-button @click="addMapping">添加映射</t-button>
      </t-space>
    </t-card>

    <t-card title="插件运行状态" :bordered="false" style="margin-top: 20px">
      <t-space v-if="runningPlugins.length">
        <t-tag v-for="id in runningPlugins" :key="id" theme="success">{{ id }}</t-tag>
      </t-space>
      <t-empty v-else description="没有运行中的插件" />
    </t-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import type { PluginConfig } from "@/api/commands";
import * as api from "@/api/commands";

const store = useYseStore();
const loading = ref(false);
const newId = ref("");
const newName = ref("");
const newExec = ref("");

const runningPlugins = ref<string[]>([]);

// Mapping state
const newMappingAddr = ref("");
const newMappingPlugin = ref("");

const mappings = computed(() => store.config?.plugin_mappings ?? []);

const pluginOptions = computed(() =>
  store.plugins.map((p) => ({
    label: `${p.name || p.id} (${p.id})`,
    value: p.id,
  })),
);

const pluginColumns = [
  { colKey: "id", title: "ID" },
  { colKey: "name", title: "名称" },
  { colKey: "exec_path", title: "路径" },
  { colKey: "enabled", title: "启用" },
  { colKey: "operation", title: "操作" },
];

const mappingColumns = [
  { colKey: "virtual_addr", title: "虚拟地址" },
  { colKey: "plugin_id", title: "插件 ID" },
  { colKey: "operation", title: "操作" },
];

async function addMapping() {
  if (!newMappingAddr.value || !newMappingPlugin.value) return;
  if (!store.config) return;
  const cfg = { ...store.config };
  cfg.plugin_mappings.push({
    virtual_addr: newMappingAddr.value,
    plugin_id: newMappingPlugin.value,
  });
  await api.saveConfig(cfg);
  await store.loadConfig();
  newMappingAddr.value = "";
  newMappingPlugin.value = "";
  await MessagePlugin.success("映射已添加");
}

async function removeMapping(row: { virtual_addr: string }) {
  if (!store.config) return;
  const cfg = { ...store.config };
  cfg.plugin_mappings = cfg.plugin_mappings.filter(
    (m) => m.virtual_addr !== row.virtual_addr,
  );
  await api.saveConfig(cfg);
  await store.loadConfig();
  await MessagePlugin.success("映射已删除");
}

async function handleToggle(row: PluginConfig, enabled: boolean) {
  try {
    await store.togglePlugin(row.id, enabled);
    await MessagePlugin.success(`${enabled ? "启动" : "停止"} ${row.name}`);
  } catch (e) {
    await MessagePlugin.error(`操作失败: ${e}`);
  }
}

async function handleDelete(row: PluginConfig) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await store.togglePlugin(row.id, false);
    await invoke("remove_plugin", { id: row.id });
    await MessagePlugin.success(`已删除 ${row.name}`);
    await store.loadPlugins();
  } catch (e) {
    await MessagePlugin.error(`删除失败: ${e}`);
  }
}

async function handleAdd() {
  if (!newId.value || !newExec.value) {
    await MessagePlugin.warning("请填写 ID 和可执行文件路径");
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("add_plugin", {
      id: newId.value,
      name: newName.value || newId.value,
      execPath: newExec.value,
    });
    newId.value = "";
    newName.value = "";
    newExec.value = "";
    await store.loadPlugins();
    await MessagePlugin.success("插件已添加");
  } catch (e) {
    await MessagePlugin.error(`添加失败: ${e}`);
  }
}

async function pickFile() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: false,
      title: "选择可执行文件",
    });
    if (selected) {
      newExec.value = selected;
    }
  } catch {
    // Not in Tauri runtime, skip
  }
}

async function fetchRunning() {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    runningPlugins.value = await invoke("list_running_plugins");
  } catch {
    runningPlugins.value = [];
  }
}

onMounted(async () => {
  loading.value = true;
  await store.loadPlugins();
  await store.loadConfig();
  await fetchRunning();
  loading.value = false;
});
</script>

<style scoped>
.card-desc {
  color: var(--td-text-color-placeholder);
  font-size: 13px;
  margin: 0 0 12px;
}
</style>
