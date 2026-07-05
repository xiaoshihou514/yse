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
        <t-form-item label="邮箱密码/授权码" name="email_password">
          <t-input v-model="form.email_password" type="password" />
        </t-form-item>
        <t-form-item label="我的虚拟地址">
          <t-input v-model="form.own_address" placeholder="me@yse.org" />
        </t-form-item>
        <t-form-item label="加密密码">
          <t-input v-model="form.crypto_password" type="password" placeholder="用于消息加密，更改后保存即可生效" />
        </t-form-item>
        <t-form-item>
          <t-space>
            <t-button theme="primary" type="submit" :loading="saving">保存</t-button>
            <t-button theme="default" @click="handleTest">测试连接</t-button>
          </t-space>
        </t-form-item>
      </t-form>
    </t-card>

    <t-card title="界面" :bordered="false" style="margin-bottom: 20px">
      <t-space align="center">
        <span>深色模式</span>
        <t-switch :value="isDark" @change="toggleDark" />
      </t-space>
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
import { ref, reactive, computed, onMounted, nextTick, watch } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import * as api from "@/api/commands";

const store = useYseStore();
const saving = ref(false);
const isDark = ref(document.documentElement.getAttribute("theme-mode") === "dark");
const levelFilter = ref<string>("info");
const logContainer = ref<HTMLElement | null>(null);

const form = reactive({
  email_imap_server: "",
  email_imap_port: 993,
  email_smtp_server: "",
  email_smtp_port: 465,
  email_username: "",
  email_password: "",
  own_address: "me@yse.org",
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

function toggleDark(v: boolean) {
  isDark.value = v;
  document.documentElement.setAttribute("theme-mode", v ? "dark" : "light");
  localStorage.setItem("yse-dark", String(v));
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
});
</script>

<style scoped>
.config-page {
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
@media (max-width: 767px) {
  .config-page {
    max-width: none;
  }
  .config-page .t-card {
    margin: 8px;
  }
  .config-page :deep(.t-form-item) {
    flex-direction: column;
    align-items: stretch;
  }
  .config-page :deep(.t-form__label) {
    width: auto !important;
    padding-bottom: 4px;
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
