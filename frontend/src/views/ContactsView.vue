<template>
  <div class="contacts-page">
    <t-card title="联系人管理" :bordered="false">
      <t-table
        :data="mappings"
        :columns="columns"
        row-key="virtual_addr"
        size="small"
      >
        <template #virtual_addr="{ row }">
          <span :data-label="'虚拟地址'">{{ row.virtual_addr }}</span>
        </template>
        <template #plugin_id="{ row }">
          <span :data-label="'绑定插件'">{{ pluginName(row.plugin_id) || row.plugin_id }}</span>
        </template>
        <template #operation="{ row }">
          <t-popconfirm content="确定删除此联系人？" @confirm="handleDelete(row)">
            <t-button theme="danger" variant="text">删除</t-button>
          </t-popconfirm>
        </template>
        <template #plugin_id="{ row }">
          {{ pluginName(row.plugin_id) || row.plugin_id }}
        </template>
      </t-table>

      <t-divider />

      <t-space>
        <t-input
          v-model="newAddr"
          placeholder="名称 (自动添加 @yse.org)"
          style="width: 240px"
          @keydown.enter="handleAdd"
        />
        <t-select
          v-model="newPlugin"
          placeholder="绑定插件 (可选)"
          style="width: 200px"
          :options="pluginOptions"
          clearable
        />
        <t-button @click="handleAdd">添加联系人</t-button>
      </t-space>
    </t-card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import * as api from "@/api/commands";

const store = useYseStore();
const newAddr = ref("");
const newPlugin = ref("");

const mappings = computed(() => store.config?.plugin_mappings ?? []);

const pluginOptions = computed(() =>
  store.plugins.map((p) => ({
    label: `${p.name || p.id} (${p.id})`,
    value: p.id,
  })),
);

const columns = [
  { colKey: "virtual_addr", title: "虚拟地址" },
  { colKey: "plugin_id", title: "绑定插件" },
  { colKey: "operation", title: "操作" },
];

function ensureSuffix(name: string): string {
  name = name.trim();
  if (!name) return "";
  if (name.includes("@")) return name;
  return name + "@yse.org";
}

function pluginName(id: string): string | undefined {
  return store.plugins.find((p) => p.id === id)?.name;
}

async function handleAdd() {
  const addr = ensureSuffix(newAddr.value);
  if (!addr) {
    await MessagePlugin.warning("请输入名称");
    return;
  }
  if (!store.config) return;
  const cfg = { ...store.config };
  // 检查是否已存在
  if (cfg.plugin_mappings.some((m) => m.virtual_addr === addr)) {
    await MessagePlugin.warning("该联系人已存在");
    return;
  }
  cfg.plugin_mappings.push({
    virtual_addr: addr,
    plugin_id: newPlugin.value || "",
  });
  try {
    await api.saveConfig(cfg);
    await store.loadConfig();
    newAddr.value = "";
    newPlugin.value = "";
    await MessagePlugin.success("联系人已添加");
  } catch (e) {
    await MessagePlugin.error(`添加失败: ${e}`);
  }
}

async function handleDelete(row: { virtual_addr: string }) {
  if (!store.config) return;
  const cfg = { ...store.config };
  cfg.plugin_mappings = cfg.plugin_mappings.filter(
    (m) => m.virtual_addr !== row.virtual_addr,
  );
  try {
    await api.saveConfig(cfg);
    await store.loadConfig();
    await MessagePlugin.success("联系人已删除");
  } catch (e) {
    await MessagePlugin.error(`删除失败: ${e}`);
  }
}

onMounted(async () => {
  await store.loadPlugins();
  await store.loadConfig();
});
</script>

<style scoped>
@media (max-width: 767px) {
  .contacts-page .t-card {
    margin: 8px;
  }
  .contacts-page :deep(.t-table) {
    font-size: 12px;
  }
  .contacts-page :deep(.t-table thead) {
    display: none;
  }
  .contacts-page :deep(.t-table tbody tr) {
    display: block;
    margin-bottom: 12px;
    border: 1px solid var(--td-component-stroke);
    border-radius: 8px;
    padding: 12px;
    background: var(--td-bg-color-container);
  }
  .contacts-page :deep(.t-table tbody td) {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 0 !important;
    border: none !important;
  }
  .contacts-page :deep(.t-table tbody td > span) {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
  }
  .contacts-page :deep(.t-table tbody td > span::before) {
    content: attr(data-label);
    font-weight: 600;
    color: var(--td-text-color-placeholder);
    font-size: 12px;
  }
  .contacts-page :deep(.t-table tbody td:last-child > span::before) {
    display: none;
  }
  .contacts-page :deep(.t-table tbody td:last-child) {
    justify-content: flex-end;
  }
  .contacts-page :deep(.t-table tbody td:last-child > span) {
    justify-content: flex-end;
  }
  .contacts-page :deep(.t-table__pagination) {
    padding-top: 8px;
  }
  .contacts-page .t-card .t-space,
  .contacts-page .t-card .t-space > .t-space-item {
    display: flex;
    flex-direction: column;
    width: 100%;
  }
  .contacts-page .t-card :deep(.t-space .t-input__wrap),
  .contacts-page .t-card :deep(.t-space .t-select__wrap),
  .contacts-page .t-card :deep(.t-space .t-button) {
    width: 100%;
  }
}
</style>
