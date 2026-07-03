<template>
  <div class="contacts-page">
    <t-card title="联系人管理" :bordered="false">
      <t-table
        :data="mappings"
        :columns="columns"
        row-key="virtual_addr"
        size="small"
      >
        <template #operation="{ row }">
          <t-popconfirm content="确定删除此联系人？" @confirm="handleDelete(row)">
            <t-button theme="danger" variant="text">删除</t-button>
          </t-popconfirm>
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
