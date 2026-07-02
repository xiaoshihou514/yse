<template>
  <t-layout class="app-layout">
    <t-aside>
      <div class="logo">YSE</div>
      <t-menu :value="currentRoute" @change="navigate">
        <t-menu-item value="/">
          <template #icon><chat-icon /></template>
          聊天
        </t-menu-item>
        <t-menu-item value="/plugins">
          <template #icon><extension-icon /></template>
          插件
        </t-menu-item>
        <t-menu-item value="/config">
          <template #icon><setting-icon /></template>
          配置
        </t-menu-item>
        <t-menu-item value="/logs">
          <template #icon><file-icon /></template>
          日志
        </t-menu-item>
      </t-menu>
    </t-aside>
    <t-content class="main-content">
      <router-view />
    </t-content>
  </t-layout>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useRouter, useRoute } from "vue-router";
import { ChatIcon, ExtensionIcon, SettingIcon, FileIcon } from "tdesign-icons-vue-next";

const router = useRouter();
const route = useRoute();

const currentRoute = computed(() => route.path);

function navigate(val: string) {
  router.push(val);
}
</script>

<style scoped>
.app-layout {
  height: 100vh;
}
.logo {
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: bold;
  color: var(--td-brand-color);
}
.main-content {
  padding: 24px;
  overflow-y: auto;
}
</style>
