<template>
  <t-layout class="app-layout">
    <t-aside v-if="!isMobile" class="app-aside">
      <div class="aside-logo">
        <img src="/icon.png" alt="盐水鹅" class="logo-img" />
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
    </t-aside>
    <t-content class="main-content">
      <router-view />
    </t-content>
    <div v-if="isMobile" class="mobile-tab-bar">
      <div
        v-for="item in navItems"
        :key="item.path"
        :class="['tab-item', { active: currentRoute === item.path }]"
        @click="navigate(item.path)"
      >
        <component :is="item.icon" class="tab-icon" />
        <span class="tab-label">{{ item.label }}</span>
      </div>
    </div>
  </t-layout>
</template>

<script setup lang="ts">
import { computed, markRaw, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { trace, debug, info, warn, error } from "@tauri-apps/plugin-log";
import {
  ChatIcon, ExtensionIcon, SettingIcon, UserIcon,
} from "tdesign-icons-vue-next";

const router = useRouter();
const route = useRoute();
const store = useYseStore();

const currentRoute = computed(() => route.path);
const isMobile = useIsMobile();

const navItems = [
  { path: "/", label: "聊天", icon: markRaw(ChatIcon) },
  { path: "/plugins", label: "插件", icon: markRaw(ExtensionIcon) },
  { path: "/contacts", label: "联系人", icon: markRaw(UserIcon) },
  { path: "/config", label: "配置", icon: markRaw(SettingIcon) },
];

function navigate(val: string) {
  router.push(val);
}

function forwardConsole(fnName: "log" | "debug" | "info" | "warn" | "error", logger: (message: string) => Promise<void>) {
  const original = console[fnName];
  console[fnName] = (message: unknown) => {
    original(message);
    logger(String(message));
  };
}

onMounted(async () => {
  forwardConsole("log", trace);
  forwardConsole("debug", debug);
  forwardConsole("info", info);
  forwardConsole("warn", warn);
  forwardConsole("error", error);
  await store.loadConfig();
  store.listenForLogs();
  store.listenForMessages();
  await store.initializeApp();
  await store.loadProcesses();
  await store.loadSessions();
});
</script>

<style scoped>
.app-layout { height: 100vh; }
.app-aside {
  display: flex !important; flex-direction: column; align-items: center;
  width: 64px !important; min-width: 64px !important; padding: 8px 0;
  border-right: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
}
.aside-logo {
  width: 44px; height: 44px; margin-bottom: 24px; margin-top: 8px;
  display: flex; align-items: center; justify-content: center;
  background: rgba(0,0,0,0.6); border-radius: 10px;
}
.logo-img { width: 32px; height: 32px; }
.aside-nav { flex: 1; display: flex; flex-direction: column; gap: 4px; }
.nav-item {
  width: 44px; height: 44px; display: flex; align-items: center;
  justify-content: center; border-radius: 8px; cursor: pointer;
  color: var(--td-text-color-secondary); font-size: 22px;
  transition: background 0.15s, color 0.15s;
}
.nav-item:hover { background: var(--td-bg-color-secondarycontainer); color: var(--td-text-color-primary); }
.nav-item.active { background: var(--td-brand-color-light); color: var(--td-brand-color); }
.main-content { overflow-y: auto; background: var(--td-bg-color-page); }

.mobile-tab-bar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  display: flex;
  justify-content: space-around;
  align-items: center;
  height: 56px;
  padding-bottom: env(safe-area-inset-bottom, 0);
  background: var(--td-bg-color-container);
  border-top: 1px solid var(--td-component-stroke);
  z-index: 1000;
}
.tab-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  padding: 4px 8px;
  cursor: pointer;
  color: var(--td-text-color-secondary);
  transition: color 0.15s;
  user-select: none;
  -webkit-tap-highlight-color: transparent;
}
.tab-item.active {
  color: var(--td-brand-color);
}
.tab-icon {
  font-size: 22px;
}
.tab-label {
  font-size: 10px;
  line-height: 1;
}

@media (max-width: 767px) {
  .main-content {
    padding-top: env(safe-area-inset-top, 24px);
    padding-bottom: 56px;
  }
}
</style>
