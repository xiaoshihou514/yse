<template>
  <div class="config-page">
    <t-card title="邮箱配置" :bordered="false" style="margin-bottom: 20px">
      <t-form :data="form" @submit="handleSave">
        <t-form-item label="IMAP 服务器" name="email_imap_server">
          <t-input v-model="form.email_imap_server" placeholder="imap.qq.com" />
        </t-form-item>
        <t-form-item label="IMAP 端口" name="email_imap_port">
          <t-input-number v-model="form.email_imap_port" :min="1" :max="65535" />
        </t-form-item>
        <t-form-item label="SMTP 服务器" name="email_smtp_server">
          <t-input v-model="form.email_smtp_server" placeholder="smtp.qq.com" />
        </t-form-item>
        <t-form-item label="SMTP 端口" name="email_smtp_port">
          <t-input-number v-model="form.email_smtp_port" :min="1" :max="65535" />
        </t-form-item>
        <t-form-item label="邮箱地址" name="email_username">
          <t-input v-model="form.email_username" placeholder="user@qq.com" />
        </t-form-item>
        <t-form-item label="IMAP授权码" name="email_password">
          <t-input v-model="form.email_password" type="password" />
        </t-form-item>
        <t-form-item label="我的显示名称">
          <t-input v-model="form.own_address" placeholder="你的名称 (用于发件地址)" />
          <template #help>本机主机名: {{ store.localHostname || '加载中...' }}</template>
        </t-form-item>
        <t-form-item label="加密密码">
          <t-input v-model="form.crypto_password" type="password" placeholder="用于消息加密，更改后保存即可生效" />
        </t-form-item>
        <t-form-item>
          <t-space>
            <t-button theme="primary" type="submit" :loading="saving">保存</t-button>
            <t-button theme="default" @click="handleTest">测试连接</t-button>
            <t-button theme="default" variant="outline" @click="showExportQr">导出二维码</t-button>
            <t-button theme="default" variant="outline" @click="handleImportQr">{{ isMobilePlatform ? '扫码导入' : '导入配置' }}</t-button>
          </t-space>
        </t-form-item>
      </t-form>
    </t-card>

    <!-- QR export dialog -->
    <t-dialog
      v-model:visible="qrExportVisible"
      header="导出配置"
      :footer="false"
      width="360px"
    >
      <div class="qr-center">
        <img v-if="qrDataUrl" :src="qrDataUrl" alt="配置二维码" class="qr-img" />
        <p v-else>生成中...</p>
        <p class="qr-hint">用另一台设备的盐水鹅扫描此二维码以导入配置</p>
      </div>
    </t-dialog>

    <!-- QR import dialog -->
    <!-- QR scanner overlay (windowed mode — webview transparent, camera shows through) -->
    <div v-if="qrImportVisible" class="qr-scanner-overlay">
      <div class="scanner-wrapper">
        <div id="qr-scanner-id" class="scanner-box">
          <div class="scanner-frame">
            <div class="frame-corner tl"></div>
            <div class="frame-corner tr"></div>
            <div class="frame-corner bl"></div>
            <div class="frame-corner br"></div>
          </div>
          <div class="scanner-line"></div>
        </div>
        <p class="qr-hint">将二维码置于框内</p>
      </div>
      <div class="qr-scanner-footer">
        <t-space>
          <t-button size="small" @click="stopScanner">取消</t-button>
          <t-button size="small" variant="outline" @click="uploadQrImage">上传图片</t-button>
        </t-space>
      </div>
    </div>

    <t-card title="界面" :bordered="false" style="margin-bottom: 20px">
      <t-form-item label="主题模式">
        <t-radio-group :value="themeMode" @change="setTheme">
          <t-radio-button value="light">浅色</t-radio-button>
          <t-radio-button value="auto">跟随系统</t-radio-button>
          <t-radio-button value="dark">深色</t-radio-button>
        </t-radio-group>
      </t-form-item>
    </t-card>

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
import { ref, reactive, computed, onMounted, nextTick, watch, onUnmounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import * as api from "@/api/commands";
import { platform } from "@tauri-apps/plugin-os";

const store = useYseStore();
const saving = ref(false);

function systemDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}
function storedTheme(): string {
  return localStorage.getItem("yse-theme") || "auto";
}
function applyTheme(theme: string) {
  if (theme === "dark") {
    document.documentElement.setAttribute("theme-mode", "dark");
  } else if (theme === "light") {
    document.documentElement.removeAttribute("theme-mode");
  } else {
    // auto
    if (systemDark()) {
      document.documentElement.setAttribute("theme-mode", "dark");
    } else {
      document.documentElement.removeAttribute("theme-mode");
    }
  }
}
const themeMode = ref(storedTheme());
applyTheme(themeMode.value);

let systemThemeListener: (() => void) | null = null;

function setTheme(val: string) {
  themeMode.value = val;
  localStorage.setItem("yse-theme", val);
  applyTheme(val);
  // listen for system changes when in auto mode
  if (val === "auto") {
    listenSystemTheme();
  } else {
    unlistenSystemTheme();
  }
}

function listenSystemTheme() {
  unlistenSystemTheme();
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => applyTheme("auto");
  mq.addEventListener("change", handler);
  systemThemeListener = () => mq.removeEventListener("change", handler);
}

function unlistenSystemTheme() {
  systemThemeListener?.();
  systemThemeListener = null;
}
const levelFilter = ref<string>("info");
const logContainer = ref<HTMLElement | null>(null);

// QR export
const qrExportVisible = ref(false);
const qrDataUrl = ref("");

// QR import
const qrImportVisible = ref(false);
const scanning = ref(false);
const isMobilePlatform = platform() === "android";

async function showExportQr() {
  qrExportVisible.value = true;
  qrDataUrl.value = "";
  await nextTick();
  try {
    const QRCode = (await import("qrcode")).default;
    const data = JSON.stringify({ ...form, plugin_mappings: store.config?.plugin_mappings ?? [] });
    qrDataUrl.value = await QRCode.toDataURL(data, {
      width: 280,
      margin: 2,
      color: { dark: "#000000", light: "#ffffff" },
    });
  } catch (e) {
    await MessagePlugin.error(`生成二维码失败: ${e}`);
    qrExportVisible.value = false;
  }
}

async function showImportQr() {
  qrImportVisible.value = true;
  await nextTick();
  scanning.value = true;
  let mod: any = null;
  try {
    mod = await import("@tauri-apps/plugin-barcode-scanner");
    console.log("[QR] module loaded");
  } catch (e) {
    console.error("[QR] import failed:", e);
    await MessagePlugin.error(`扫码模块加载失败: ${e}`);
    await uploadQrImage();
    scanning.value = false;
    qrImportVisible.value = false;
    return;
  }
  try {
    const perm = await mod.checkPermissions();
    console.log("[QR] checkPermissions:", perm);
    if (perm !== "granted") {
      const result = await mod.requestPermissions();
      console.log("[QR] requestPermissions result:", result);
      if (result !== "granted") {
        await MessagePlugin.warning("摄像头权限被拒绝");
        qrImportVisible.value = false;
        scanning.value = false;
        return;
      }
    }
    console.log("[QR] starting scan...");
    const result = await mod.scan({ windowed: true, formats: [mod.Format.QRCode], cameraDirection: "back" });
    console.log("[QR] scan result:", result);
    applyQrConfig(result.content);
  } catch (e) {
    const msg = String(e);
    console.error("[QR] scan failed:", msg);
    await MessagePlugin.info(`扫码未成功: ${msg}，请选择二维码图片`);
    await uploadQrImage();
  } finally {
    scanning.value = false;
    qrImportVisible.value = false;
  }
}

async function stopScanner() {
  try {
    const { cancel } = await import("@tauri-apps/plugin-barcode-scanner");
    await cancel();
  } catch { /* ignore */ }
  qrImportVisible.value = false;
}

async function handleImportQr() {
  if (isMobilePlatform) {
    await showImportQr();
  } else {
    await uploadQrImage();
  }
}

async function uploadQrImage() {
  try {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const { Html5Qrcode } = await import("html5-qrcode");
      const tempCode = new Html5Qrcode("qr-scanner-id");
      try {
        const result = await tempCode.scanFile(file, false);
        applyQrConfig(result);
      } finally {
        try { await tempCode.clear(); } catch {}
      }
    };
    input.click();
  } catch (e) {
    await MessagePlugin.error(`解析图片失败: ${e}`);
  }
}

function applyQrConfig(json: string) {
  try {
    const cfg = JSON.parse(json);
    if (cfg.email_imap_server != null) form.email_imap_server = cfg.email_imap_server;
    if (cfg.email_imap_port != null) form.email_imap_port = cfg.email_imap_port;
    if (cfg.email_smtp_server != null) form.email_smtp_server = cfg.email_smtp_server;
    if (cfg.email_smtp_port != null) form.email_smtp_port = cfg.email_smtp_port;
    if (cfg.email_username) form.email_username = cfg.email_username;
    if (cfg.email_password) form.email_password = cfg.email_password;
    if (cfg.own_address) form.own_address = cfg.own_address;
    if (cfg.crypto_password) form.crypto_password = cfg.crypto_password;
    stopScanner();
    MessagePlugin.success("配置已从二维码导入，点击保存以生效");
  } catch (e) {
    MessagePlugin.error(`解析配置失败: ${e}`);
  }
}

onUnmounted(() => {
  stopScanner();
  unlistenSystemTheme();
});

const form = reactive({
  email_imap_server: "",
  email_imap_port: 993,
  email_smtp_server: "",
  email_smtp_port: 465,
  email_username: "",
  email_password: "",
  own_address: "me",
  crypto_password: "",
});

const levelOptions = [
  { label: "INFO", value: "info" },
  { label: "DEBUG", value: "debug" },
  { label: "WARN", value: "warn" },
  { label: "ERROR", value: "error" },
];

const levelPriority: Record<string, number> = {
  debug: 0, info: 1, warn: 2, error: 3,
};

const filteredLogs = computed(() => {
  const minPriority = levelPriority[levelFilter.value] ?? 1;
  return store.logs.filter((l) => (levelPriority[l.level] ?? 3) >= minPriority);
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

async function refresh() {
  await store.loadLogs();
  await nextTick();
  if (logContainer.value) {
    logContainer.value.scrollTop = logContainer.value.scrollHeight;
  }
}

function clear() {
  store.logs.splice(0);
}

async function handleSave() {
  saving.value = true;
  try {
    const mappings = store.config?.plugin_mappings ?? [];
    await store.saveConfigAndApply({ ...form, plugin_mappings: mappings });
    await MessagePlugin.success("配置已保存");
  } catch (e) {
    await MessagePlugin.error(`保存失败: ${e}`);
  } finally {
    saving.value = false;
  }
}

async function handleTest() {
  try {
    const result = await api.testEmail(
      form.email_imap_server,
      form.email_imap_port,
      form.email_username,
      form.email_password,
    );
    await MessagePlugin.success(result);
  } catch (e) {
    await MessagePlugin.error(`连接失败: ${e}`);
  }
}

onMounted(async () => {
  await store.loadConfig();
  await store.loadPlugins();
  if (store.config) {
    form.email_imap_server = store.config.email_imap_server;
    form.email_imap_port = store.config.email_imap_port;
    form.email_smtp_server = store.config.email_smtp_server;
    form.email_smtp_port = store.config.email_smtp_port;
    form.email_username = store.config.email_username;
    form.email_password = store.config.email_password;
    form.own_address = store.config.own_address;
    form.crypto_password = store.config.crypto_password;
  }
  await refresh();
  store.listenForLogs();
  if (themeMode.value === "auto") listenSystemTheme();
});
</script>

<style scoped>
.config-page {
  max-width: 1000px;
}
.config-page :deep(.t-form__label) {
  text-align: left;
  padding-right: 16px;
}
.config-page :deep(.t-form-item__help) {
  text-align: left;
}
.qr-center {
  display: flex; flex-direction: column; align-items: center; gap: 12px;
}
.qr-img {
  width: 240px; height: 240px;
  border: 1px solid var(--td-component-stroke);
  border-radius: 8px; padding: 8px;
}
.qr-hint {
  font-size: 13px; color: var(--td-text-color-placeholder); text-align: center;
}
.qr-scanner-overlay {
  position: fixed; inset: 0; z-index: 9999;
  display: flex; flex-direction: column; align-items: center;
  background: rgba(0,0,0,0.5);
}
.qr-scanner-footer {
  position: fixed; bottom: 40px; left: 0; right: 0;
  display: flex; justify-content: center;
  z-index: 10000;
}
.scanner-wrapper {
  width: 280px; height: 280px; border-radius: 8px;
  overflow: hidden; position: relative;
  margin-top: 25vh;
  background: transparent;
}
.scanner-box {
  width: 100%; height: 100%; position: relative;
}
.scanner-frame {
  position: absolute; inset: 0; z-index: 2;
  pointer-events: none;
}
.frame-corner {
  position: absolute; width: 28px; height: 28px;
  border-color: var(--td-brand-color);
  border-style: solid;
}
.frame-corner.tl { top: 12px; left: 12px; border-width: 3px 0 0 3px; border-radius: 4px 0 0 0; }
.frame-corner.tr { top: 12px; right: 12px; border-width: 3px 3px 0 0; border-radius: 0 4px 0 0; }
.frame-corner.bl { bottom: 12px; left: 12px; border-width: 0 0 3px 3px; border-radius: 0 0 0 4px; }
.frame-corner.br { bottom: 12px; right: 12px; border-width: 0 3px 3px 0; border-radius: 0 0 4px 0; }
.scanner-line {
  position: absolute; left: 12px; right: 12px; height: 2px;
  background: var(--td-brand-color); z-index: 3;
  animation: scanLine 2s ease-in-out infinite;
  border-radius: 1px; opacity: 0.7;
}
@keyframes scanLine {
  0% { top: 16px; }
  50% { top: calc(100% - 18px); }
  100% { top: 16px; }
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
@media (max-width: 767px) {
  .config-page {
    max-width: none;
    padding-bottom: 64px;
  }
  .config-page .t-card {
    margin: 8px;
  }
  .config-page :deep(.t-form__item) {
    flex-direction: column;
    align-items: stretch;
  }
  .config-page :deep(.t-form__label) {
    width: auto !important;
    padding-bottom: 4px;
  }
  .config-page :deep(.t-form__item:last-child) {
    overflow: hidden;
    max-width: 100%;
  }
  .config-page :deep(.t-form__item:last-child .t-space) {
    display: flex !important;
    flex-wrap: wrap;
    gap: 6px;
    width: 100%;
  }
  .config-page :deep(.t-form__item:last-child .t-space .t-space-item) {
    flex: 1 1 auto;
    min-width: 0;
  }
  .config-page :deep(.t-form__item:last-child .t-space .t-button) {
    flex: 1 1 auto;
    min-width: 0;
  }
  .scanner-wrapper {
    width: calc(100vw - 64px); height: calc(100vw - 64px);
    max-width: 300px; max-height: 300px;
  }
  .log-container {
    max-height: none;
    font-size: 12px;
  }
  .log-entry {
    flex-wrap: wrap;
    gap: 4px;
  }
  .log-time {
    min-width: auto;
    font-size: 11px;
  }
}
</style>
