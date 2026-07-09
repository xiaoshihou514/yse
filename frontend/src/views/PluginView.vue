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
          <div class="row-actions">
            <t-button
              size="small"
              variant="text"
              @click="showPluginQr(row)"
              title="分享名片"
            >
              <template #icon><QrcodeIcon /></template>
            </t-button>
            <t-popconfirm
              content="确定删除此插件？"
              @confirm="handleDelete(row)"
            >
              <t-button theme="danger" variant="text" title="删除">
                <template #icon><DeleteIcon /></template>
              </t-button>
            </t-popconfirm>
          </div>
        </template>
      </t-table>
      <t-divider />
      <t-space>
        <t-input v-model="newName" placeholder="插件名称" />
        <t-input
          v-model="newExec"
          placeholder="可执行文件路径"
          style="width: 300px"
        />
        <t-button variant="outline" @click="pickFile">选择文件</t-button>
        <t-button @click="handleAdd">添加</t-button>
      </t-space>
    </t-card>

    <!-- Process manager (desktop only) -->
    <t-card
      v-if="!isMobile"
      title="进程管理"
      :bordered="false"
      style="margin-top: 0"
    >
      <t-table
        :data="store.processes"
        :columns="procColumns"
        row-key="id"
        size="small"
        :empty="'暂无运行中进程'"
      >
        <template #name="{ row }">
          <span>{{ row.name }}</span>
        </template>
        <template #state="{ row }">
          <t-tag :theme="procTag(row.state)" variant="light" size="small">{{
            row.state
          }}</t-tag>
        </template>
        <template #restart_count="{ row }">
          <span>{{ row.restart_count || 0 }}</span>
        </template>
        <template #operation="{ row }">
          <t-space size="small">
            <t-button
              size="small"
              variant="text"
              @click="handleStopProcess(row.id)"
              :disabled="row.state === 'Stopped'"
              title="结束进程"
            >
              <template #icon><StopCircleIcon /></template>
            </t-button>
            <t-button
              size="small"
              variant="text"
              @click="handleRestartProcess(row.id)"
              title="重启进程"
            >
              <template #icon><RefreshIcon /></template>
            </t-button>
            <t-button
              size="small"
              variant="text"
              @click="handleViewLogs(row.id)"
              title="查看日志"
            >
              <template #icon><ViewListIcon /></template>
            </t-button>
          </t-space>
        </template>
      </t-table>
    </t-card>

    <!-- Mobile: no plugin management (plugins run on desktop only) -->
    <template v-else>
      <div class="mobile-header">
        <h2 class="mobile-title">插件管理</h2>
      </div>
      <div class="mobile-hint-card">
        <p>插件运行在桌面端 YSE 服务中。</p>
        <p>在桌面端添加/管理插件后，移动端可以发消息控制它们。</p>
      </div>

      <!-- Add dialog -->
      <t-dialog
        v-model:visible="showAdd"
        header="添加插件"
        :footer="false"
        width="360px"
        :close-on-overlay-click="true"
      >
        <t-form>
          <t-form-item label="名称">
            <t-input v-model="newName" placeholder="如 echo-bot" />
          </t-form-item>
          <t-form-item label="执行路径">
            <t-input v-model="newExec" placeholder="/usr/local/bin/echo-bot" />
            <template #help>
              <t-button size="small" variant="outline" @click="pickFile"
                >选择文件</t-button
              >
            </template>
          </t-form-item>
          <t-form-item>
            <t-button block @click="handleAdd">添加</t-button>
          </t-form-item>
        </t-form>
      </t-dialog>
    </template>

    <!-- Per-plugin QR overlay (outside v-if/v-else so it works on desktop too) -->
    <div
      v-if="showPluginQrDialog"
      class="qr-overlay"
      @click="showPluginQrDialog = false"
    >
      <div class="qr-card" @click.stop>
        <div class="qr-card-header">{{ qrPlugin?.name ?? "" }}</div>
        <div class="qr-center">
          <img
            v-if="pluginQrUrl"
            :src="pluginQrUrl"
            alt="二维码"
            class="qr-img"
          />
          <p v-else>生成中...</p>
          <p class="qr-hint">让对方扫描即可添加联系人</p>
          <p v-if="qrPluginAddr" class="qr-addr">{{ qrPluginAddr }}</p>
        </div>
        <t-button
          size="small"
          style="margin-top: 12px"
          @click="showPluginQrDialog = false"
          >关闭</t-button
        >
      </div>
    </div>

    <!-- Process log dialog -->
    <t-dialog
      v-model:visible="logVisible"
      :header="`进程日志 — ${logProcessName}`"
      width="600px"
      :close-on-overlay-click="true"
      :footer="false"
    >
      <div class="log-viewer" v-if="logLines.length > 0">
        <pre class="log-content" v-for="(line, i) in logLines" :key="i">{{
          line
        }}</pre>
      </div>
      <t-empty v-else description="暂无日志" />
    </t-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import {
  QrcodeIcon,
  DeleteIcon,
  StopCircleIcon,
  RefreshIcon,
  ViewListIcon,
} from "tdesign-icons-vue-next";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { useQrCode } from "@/composables/useQrCode";
import * as api from "@/api/commands";
import type { PluginConfig } from "@/api/commands";
import { showError } from "@/utils/helpers";

const isMobile = useIsMobile();
const store = useYseStore();
const loading = ref(false);
const newName = ref("");
const newExec = ref("");
const showAdd = ref(false);
const showPluginQrDialog = ref(false);
const { qrDataUrl: pluginQrUrl, generate: generateQr } = useQrCode();
const qrPlugin = ref<PluginConfig | null>(null);
const qrPluginAddr = ref("");

const columns = [
  { colKey: "name", title: "名称" },
  { colKey: "exec_path", title: "路径" },
  { colKey: "operation", title: "操作" },
];

const procColumns = [
  { colKey: "name", title: "插件" },
  { colKey: "state", title: "状态" },
  { colKey: "restart_count", title: "重启次数", width: 100 },
  { colKey: "operation", title: "操作", width: 140 },
];

function procState(pluginId: string): string | undefined {
  return store.processes.find((p) => p.id === pluginId)?.state;
}

function procTag(
  state: string | undefined,
): "success" | "warning" | "danger" | "default" {
  if (state === "Running") return "success";
  if (state === "Starting" || state === "Stopping") return "warning";
  if (state === "Crashed") return "danger";
  return "default";
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
    showError("删除", e);
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
    showError("添加", e);
  }
}

async function pickFile() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ multiple: false, title: "选择可执行文件" });
    if (selected) newExec.value = selected;
  } catch {
    /* not in tauri */
  }
}

async function showPluginQr(plugin: PluginConfig) {
  qrPlugin.value = plugin;
  qrPluginAddr.value = "";
  showPluginQrDialog.value = true;
  const hostname = store.localHostname || "localhost";
  const addr = `${plugin.name}#00000000@${hostname}`;
  qrPluginAddr.value = addr;
  const data = JSON.stringify({ name: plugin.name, hostname });
  await generateQr(data);
}

async function handleStopProcess(processId: string) {
  try {
    await api.stopPlugin(processId);
    await store.loadProcesses();
    await MessagePlugin.success("进程已结束");
  } catch (e) {
    showError("结束进程", e);
  }
}

async function handleRestartProcess(processId: string) {
  try {
    await api.stopPlugin(processId);
    await api.startPlugin(processId);
    await store.loadProcesses();
    await MessagePlugin.success("进程已重启");
  } catch (e) {
    showError("重启进程", e);
  }
}

let procRefreshTimer: ReturnType<typeof setInterval> | null = null;

const logVisible = ref(false);
const logProcessName = ref("");
const logLines = ref<string[]>([]);

async function handleViewLogs(processId: string) {
  try {
    const p = store.processes.find((x) => x.id === processId);
    logProcessName.value = p?.name ?? processId;
    logLines.value = await api.getProcessLogs(processId);
    logVisible.value = true;
  } catch (e) {
    showError("获取日志", e);
  }
}

onMounted(async () => {
  loading.value = true;
  await store.loadPlugins();
  await store.loadProcesses();
  loading.value = false;
  procRefreshTimer = setInterval(() => store.loadProcesses(), 5000);
});

onUnmounted(() => {
  if (procRefreshTimer) clearInterval(procRefreshTimer);
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
.mobile-hint-card {
  margin: 16px;
  padding: 20px;
  border-radius: 10px;
  background: var(--td-bg-color-secondarycontainer);
  line-height: 1.8;
  font-size: 14px;
  color: var(--td-text-color-secondary);
}
.mobile-hint-card p {
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
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
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
.card-func {
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
.row-actions {
  display: flex;
  align-items: center;
  gap: 4px;
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
  gap: 6px;
  justify-content: flex-end;
}
.qr-center {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}
.qr-img {
  width: 240px;
  height: 240px;
  border: 1px solid var(--td-component-stroke);
  border-radius: 8px;
  padding: 8px;
}
.qr-hint {
  font-size: 13px;
  color: var(--td-text-color-placeholder);
  text-align: center;
}
.qr-addr {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  word-break: break-all;
  text-align: center;
}
.qr-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
}
.qr-card {
  background: var(--td-bg-color-container);
  border-radius: 12px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  max-width: 320px;
  width: 100%;
}
.qr-card-header {
  font-size: 16px;
  font-weight: 600;
  color: var(--td-text-color-primary);
}

.fab {
  position: fixed;
  bottom: 64px;
  right: 16px;
  z-index: 900;
  width: 52px;
  height: 52px;
  border-radius: 50%;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.25);
}
.fab-icon {
  font-size: 24px;
  line-height: 1;
}

@media (max-width: 767px) {
  .fab {
    bottom: calc(72px + env(safe-area-inset-bottom, 0px));
  }
}
@media (min-width: 768px) {
  .plugin-page .t-card {
    margin: 16px;
  }
}

.log-viewer {
  max-height: 400px;
  overflow-y: auto;
  background: var(--td-bg-color-page);
  border-radius: 6px;
  padding: 8px;
}
.log-content {
  margin: 0;
  padding: 2px 0;
  font-size: 12px;
  font-family: ui-monospace, monospace;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--td-text-color-primary);
  border-bottom: 1px solid var(--td-component-stroke);
}
.log-content:last-child {
  border-bottom: none;
}
</style>
