<template>
  <t-layout class="app-layout">
    <t-aside class="app-aside">
      <div class="aside-logo">
        <img src="/icon.png" alt="YSE" class="logo-img" />
      </div>
      <div class="aside-nav">
        <div
          v-for="item in navItems"
          :key="item.path"
          :class="['nav-item', { active: currentRoute === item.path }]"
          @click="navigate(item.path)"
          :title="item.label"
        >
          <component :is="item.icon" />
        </div>
      </div>
      <div class="aside-footer">
        <div
          class="nav-item"
          :title="isDark ? '切换亮色' : '切换暗色'"
          @click="toggleDark(!isDark)"
        >
          <template v-if="isDark">
            <mode-light-icon />
          </template>
          <template v-else>
            <mode-dark-icon />
          </template>
        </div>
      </div>
    </t-aside>
    <t-content class="main-content">
      <router-view />
    </t-content>
  </t-layout>
</template>

<script setup lang="ts">
import { ref, computed, markRaw } from "vue";
import { useRouter, useRoute } from "vue-router";
import {
  ChatIcon, ExtensionIcon, SettingIcon, FileIcon,
  ModeLightIcon, ModeDarkIcon,
} from "tdesign-icons-vue-next";

const router = useRouter();
const route = useRoute();

const currentRoute = computed(() => route.path);
const isDark = ref(document.documentElement.getAttribute("theme-mode") === "dark");

const navItems = [
  { path: "/", label: "聊天", icon: markRaw(ChatIcon) },
  { path: "/plugins", label: "插件", icon: markRaw(ExtensionIcon) },
  { path: "/config", label: "配置", icon: markRaw(SettingIcon) },
  { path: "/logs", label: "日志", icon: markRaw(FileIcon) },
];

function navigate(val: string) {
  router.push(val);
}

function toggleDark(v: boolean) {
  isDark.value = v;
  document.documentElement.setAttribute("theme-mode", v ? "dark" : "light");
  localStorage.setItem("yse-dark", String(v));
}
</script>

<style scoped>
.app-layout {
  height: 100vh;
}
.app-aside {
  display: flex !important;
  flex-direction: column;
  align-items: center;
  width: 64px !important;
  padding: 8px 0;
  border-right: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.aside-logo {
  width: 44px;
  height: 44px;
  margin-bottom: 24px;
  margin-top: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.logo-img {
  width: 32px;
  height: 32px;
  filter: drop-shadow(0 0 3px rgba(0,0,0,0.45));
}
.aside-nav {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.aside-footer {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.nav-item {
  width: 44px;
  height: 44px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  cursor: pointer;
  color: var(--td-text-color-secondary);
  font-size: 22px;
  transition: background 0.15s, color 0.15s;
}
.nav-item:hover {
  background: var(--td-bg-color-secondarycontainer);
  color: var(--td-text-color-primary);
}
.nav-item.active {
  background: var(--td-brand-color-light);
  color: var(--td-brand-color);
}
.main-content {
  overflow-y: auto;
  background: var(--td-bg-color-page);
}
</style>
