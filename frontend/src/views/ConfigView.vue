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
        <t-form-item>
          <t-space>
            <t-button theme="primary" type="submit" :loading="saving">保存</t-button>
            <t-button theme="default" @click="handleTest">测试连接</t-button>
          </t-space>
        </t-form-item>
      </t-form>
    </t-card>

    <t-card title="加密设置" :bordered="false" style="margin-bottom: 20px">
      <t-form>
        <t-form-item label="加密切换密码">
          <t-input v-model="cryptoPassword" type="password" placeholder="输入密码（不会保存到本地配置）" />
        </t-form-item>
        <t-form-item label="我的虚拟地址">
          <t-input v-model="form.own_address" placeholder="me@yse.org" />
        </t-form-item>
      </t-form>
    </t-card>

    <t-card title="消息分发映射" :bordered="false">
      <t-table :data="form.plugin_mappings" :columns="mappingColumns" row-key="virtual_addr">
        <template #operation="{ row }">
          <t-button theme="danger" variant="text" @click="removeMapping(row)">删除</t-button>
        </template>
      </t-table>
      <t-space style="margin-top: 12px">
        <t-input v-model="newMappingAddr" placeholder="虚拟地址" style="width: 200px" />
        <t-select v-model="newMappingPlugin" placeholder="选择插件" style="width: 200px" />
        <t-button @click="addMapping">添加</t-button>
      </t-space>
    </t-card>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import * as api from "@/api/commands";

const store = useYseStore();
const saving = ref(false);
const cryptoPassword = ref("");
const newMappingAddr = ref("");
const newMappingPlugin = ref("");

const form = reactive({
  email_imap_server: "",
  email_imap_port: 993,
  email_smtp_server: "",
  email_smtp_port: 465,
  email_username: "",
  email_password: "",
  own_address: "me@yse.org",
  plugin_mappings: [] as { virtual_addr: string; plugin_id: string }[],
});

const mappingColumns = [
  { colKey: "virtual_addr", title: "虚拟地址" },
  { colKey: "plugin_id", title: "插件 ID" },
  { colKey: "operation", title: "操作" },
];

function addMapping() {
  if (!newMappingAddr.value || !newMappingPlugin.value) return;
  form.plugin_mappings.push({
    virtual_addr: newMappingAddr.value,
    plugin_id: newMappingPlugin.value,
  });
  newMappingAddr.value = "";
  newMappingPlugin.value = "";
}

function removeMapping(row: { virtual_addr: string }) {
  form.plugin_mappings = form.plugin_mappings.filter(
    (m) => m.virtual_addr !== row.virtual_addr,
  );
}

async function handleSave() {
  saving.value = true;
  try {
    await store.saveConfigAndApply({ ...form });
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
  if (store.config) {
    form.email_imap_server = store.config.email_imap_server;
    form.email_imap_port = store.config.email_imap_port;
    form.email_smtp_server = store.config.email_smtp_server;
    form.email_smtp_port = store.config.email_smtp_port;
    form.email_username = store.config.email_username;
    form.email_password = store.config.email_password;
    form.own_address = store.config.own_address;
    form.plugin_mappings = store.config.plugin_mappings;
  }
});
</script>

<style scoped>
.config-page {
  max-width: 800px;
}
</style>
