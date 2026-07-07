<template>
  <div class="contacts-page">
    <!-- Desktop: table -->
    <t-card v-if="!isMobile" title="联系人管理" :bordered="false">
      <t-table
        :data="displayMappings"
        :columns="columns"
        row-key="virtual_addr"
        size="small"
      >
        <template #virtual_addr="{ row }">
          <span>{{ displayAddress(row) }}</span>
        </template>
        <template #plugin_id="{ row }">
          <span>{{ pluginName(row.plugin_id) || '—' }}</span>
        </template>
        <template #operation="{ row }">
          <t-popconfirm content="确定删除此联系人？" @confirm="handleDelete(row)">
            <t-button theme="danger" variant="text">删除</t-button>
          </t-popconfirm>
        </template>
      </t-table>
      <t-divider />
      <t-space>
        <t-input v-model="newName" placeholder="名称" style="width: 200px" @keydown.enter="handleAdd" />
        <t-input v-model="newHostname" placeholder="目标主机" style="width: 160px" @keydown.enter="handleAdd" />
        <t-input v-model="newPlugin" placeholder="绑定插件 (可选)" style="width: 200px" @keydown.enter="handleAdd" />
        <t-button @click="handleAdd">添加联系人</t-button>
      </t-space>
      <div class="form-hint">地址格式：<code>{{ newName || '名称' }}#8位随机码@{{ newHostname || '主机名' }}</code></div>
    </t-card>

    <!-- Mobile: card list -->
    <template v-else>
      <div class="mobile-header">
        <h2 class="mobile-title">联系人管理</h2>
      </div>
      <div class="contact-cards">
        <div v-for="m in displayMappings" :key="m.virtual_addr" class="contact-card">
          <div class="card-top">
            <div class="card-info">
              <span class="card-addr">{{ displayAddress(m) }}</span>
              <span v-if="m.plugin_id" class="card-plugin">绑定: {{ pluginName(m.plugin_id) }}</span>
            </div>
          </div>
          <div class="card-actions">
            <t-popconfirm content="确定删除此联系人？" @confirm="handleDelete(m)">
              <t-button theme="danger" variant="text" size="small">删除</t-button>
            </t-popconfirm>
          </div>
        </div>
        <t-empty v-if="!displayMappings.length" description="暂无联系人" />
      </div>

      <t-button class="fab" shape="circle" size="large" @click="showAdd = true">
        <template #icon><span class="fab-icon">+</span></template>
      </t-button>

      <t-dialog v-model:visible="showAdd" header="添加联系人" :footer="false" width="360px" :close-on-overlay-click="true">
        <t-form>
          <t-form-item label="名称">
            <t-input v-model="newName" placeholder="如 echo-bot" />
          </t-form-item>
          <t-form-item label="主机名">
            <t-input v-model="newHostname" placeholder="目标主机" />
            <template #help>已知主机: {{ store.hostnames.join(', ') || '无' }}</template>
          </t-form-item>
          <t-form-item label="绑定插件">
            <t-input v-model="newPlugin" placeholder="可选" />
            <template #help>已知插件: {{ store.plugins.map(p => p.name).join(', ') || '无' }}</template>
          </t-form-item>
          <t-form-item>
              <div class="addr-preview">地址: <code>{{ newName || '名称' }}#8位随机码@{{ newHostname || '主机名' }}</code></div>
          </t-form-item>
          <t-form-item>
            <t-button block @click="handleAdd">添加</t-button>
          </t-form-item>
          <t-form-item v-if="isMobilePlatform">
            <t-button variant="outline" block @click="startContactScan">扫一扫添加</t-button>
          </t-form-item>
        </t-form>
      </t-dialog>

    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, watch } from "vue";
import { useRouter, useRoute } from "vue-router";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { platform } from "@tauri-apps/plugin-os";
import * as api from "@/api/commands";

const router = useRouter();
const route = useRoute();
const isMobile = useIsMobile();
const isMobilePlatform = platform() === "android";
const store = useYseStore();
const newName = ref("");
const newHostname = ref("");
const newPlugin = ref("");
const showAdd = ref(false);

async function startContactScan() {
  router.push("/scan?type=contact");
}

watch([() => route.query.scanName, () => route.query.scanHostname], ([name, hostname]) => {
  if (name) {
    newPlugin.value = name as string;
    showAdd.value = true;
    router.replace({ query: {} });
  }
});

function parseAddress(addr: string) {
  const at = addr.lastIndexOf("@");
  if (at < 0) return { name: addr, hash: "", hostname: "" };
  const hostname = addr.slice(at + 1);
  const local = addr.slice(0, at);
  const hashIdx = local.indexOf("#");
  if (hashIdx < 0) return { name: local, hash: "", hostname };
  return {
    name: local.slice(0, hashIdx),
    hash: local.slice(hashIdx + 1),
    hostname,
  };
}

const mappings = computed(() => store.config?.plugin_mappings ?? []);

const displayMappings = computed(() =>
  mappings.value.map((m) => ({
    ...m,
    _parsed: parseAddress(m.virtual_addr),
  })),
);

const hostnameOptions = computed(() =>
  store.hostnames.map((h) => ({ label: h, value: h })),
);

const pluginOptions = computed(() =>
  store.plugins.map((p) => ({
    label: p.name || p.id,
    value: p.id,
  })),
);

const columns = [
  { colKey: "virtual_addr", title: "地址" },
  { colKey: "plugin_id", title: "绑定插件" },
  { colKey: "operation", title: "操作" },
];

async function scanContactQr() {
  try {
    const { scan, Format, requestPermissions, checkPermissions } = await import("@tauri-apps/plugin-barcode-scanner");
    const perm = await checkPermissions();
    if (perm !== "granted") {
      const result = await requestPermissions();
      if (result !== "granted") {
        await MessagePlugin.warning("摄像头权限被拒绝");
        return;
      }
    }
    const result = await scan({ windowed: true, formats: [Format.QRCode], cameraDirection: "back" });
    const data = JSON.parse(result.content);
    // QR encodes { name: "插件名", hostname: "设备名" }
    if (data.name) newPlugin.value = data.name;
    if (data.hostname) newHostname.value = data.hostname;
    await MessagePlugin.success("已从二维码读取联系人信息，请输入联系人名称");
  } catch {
    await MessagePlugin.info("扫码取消或失败");
  }
}

function displayAddress(row: { virtual_addr: string; _parsed: ReturnType<typeof parseAddress> }) {
  const p = row._parsed;
  if (p.hostname) return `${p.name}@${p.hostname}`;
  return p.name;
}

function pluginName(id: string): string | undefined {
  return store.plugins.find((p) => p.id === id)?.name;
}

function generateHash(): string {
  return Math.random().toString(16).slice(2, 10);
}

async function handleAdd() {
  const name = newName.value.trim();
  if (!name) {
    await MessagePlugin.warning("请输入名称");
    return;
  }
  let hostname = newHostname.value.trim();
  if (!hostname) {
    hostname = store.localHostname || (await api.getHostname());
  }
  const hash = generateHash();
  const vaddr = `${name}#${hash}@${hostname}`;

  if (!store.config) return;
  const cfg = { ...store.config };
  if (cfg.plugin_mappings.some((m) => m.virtual_addr === vaddr)) {
    await MessagePlugin.warning("该联系人已存在");
    return;
  }
  cfg.plugin_mappings.push({
    virtual_addr: vaddr,
    plugin_id: newPlugin.value || "",
  });
  try {
    await api.saveConfig(cfg);
    await store.loadConfig();
    newName.value = "";
    newHostname.value = "";
    newPlugin.value = "";
    showAdd.value = false;
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
  await store.loadLocalHostname();
  if (!newHostname.value && store.localHostname) {
    newHostname.value = store.localHostname;
  }
});
</script>

<style scoped>
.form-hint {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  margin-top: 8px;
}
.form-hint code,
.addr-preview code {
  background: var(--td-bg-color-secondarycontainer);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
}
.addr-preview {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  line-height: 1.6;
}

.mobile-header {
  padding: 16px 16px 0;
}
.mobile-title {
  font-size: 20px;
  font-weight: 600;
  margin: 0;
}

.contact-cards {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 16px 80px;
}
.contact-card {
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
.card-addr {
  font-size: 15px;
  font-weight: 500;
  color: var(--td-text-color-primary);
}
.card-plugin {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
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

.scanner-box-wrap {
  position: fixed; left: 50%; top: 25vh;
  width: 280px; height: 280px;
  transform: translateX(-50%);
  border-radius: 12px;
  background: transparent !important;
  box-shadow: 0 0 0 9999px rgba(0,0,0,0.6);
  pointer-events: auto;
}

@media (max-width: 767px) {
  .fab {
    bottom: calc(72px + env(safe-area-inset-bottom, 0px));
  }
  .scanner-box-wrap {
    width: calc(100vw - 64px); height: calc(100vw - 64px);
    max-width: 300px; max-height: 300px;
  }
}
@media (min-width: 768px) {
  .contacts-page .t-card { margin: 16px; }
}
</style>
