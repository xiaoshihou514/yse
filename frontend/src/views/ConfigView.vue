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

  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from "vue";
import { MessagePlugin } from "tdesign-vue-next";
import { useYseStore } from "@/stores/yse";
import * as api from "@/api/commands";

const store = useYseStore();
const saving = ref(false);

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
});
</script>

<style scoped>
@media (max-width: 767px) {
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
}
</style>
