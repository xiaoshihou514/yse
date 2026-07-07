<template>
  <router-view v-if="route.meta?.fullscreen" />
  <div v-else class="app-layout">
    <div v-if="!isMobile" class="titlebar" data-tauri-drag-region>
      <div class="titlebar-controls">
        <button class="titlebar-btn" id="titlebar-minimize" title="最小化">
          <svg viewBox="0 0 24 24" width="16" height="16">
            <path fill="currentColor" d="M19 13H5v-2h14z" />
          </svg>
        </button>
        <button class="titlebar-btn" id="titlebar-maximize" title="最大化">
          <svg viewBox="0 0 24 24" width="16" height="16">
            <path fill="currentColor" d="M4 4h16v16H4zm2 4v10h12V8z" />
          </svg>
        </button>
        <button
          class="titlebar-btn titlebar-btn-close"
          id="titlebar-close"
          title="关闭"
        >
          <svg viewBox="0 0 24 24" width="16" height="16">
            <path
              fill="currentColor"
              d="M13.46 12L19 17.54V19h-1.46L12 13.46L6.46 19H5v-1.46L10.54 12L5 6.46V5h1.46L12 10.54L17.54 5H19v1.46z"
            />
          </svg>
        </button>
      </div>
    </div>
    <t-layout class="app-body">
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
    </t-layout>
    <div v-if="isMobile && !mobileChatOpen" class="mobile-tab-bar">
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
  </div>
</template>

<script setup lang="ts">
import { computed, markRaw, onMounted, nextTick } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";
import { mobileChatOpen } from "@/composables/useChatOpen";
import { trace, debug, info, warn, error } from "@tauri-apps/plugin-log";
import { getCurrentWindow } from "@tauri-apps/api/window";
import appRouter, { setConfigState } from "@/router";
import {
  ChatIcon,
  ExtensionIcon,
  SettingIcon,
  UserIcon,
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

function forwardConsole(
  fnName: "log" | "debug" | "info" | "warn" | "error",
  logger: (message: string) => Promise<void>,
) {
  const original = console[fnName];
  console[fnName] = (message: unknown) => {
    original(message);
    logger(String(message));
  };
}

function setupTitlebar() {
  const win = getCurrentWindow();
  const $ = (id: string) => document.getElementById(id);
  $("titlebar-minimize")?.addEventListener("click", () => win.minimize());
  $("titlebar-maximize")?.addEventListener("click", () => win.toggleMaximize());
  $("titlebar-close")?.addEventListener("click", () => win.close());
  // Double-click titlebar to maximize
  document.querySelector(".titlebar")?.addEventListener("dblclick", (e) => {
    if ((e.target as HTMLElement).closest(".titlebar-controls")) return;
    win.toggleMaximize();
  });
}

onMounted(async () => {
  forwardConsole("log", trace);
  forwardConsole("debug", debug);
  forwardConsole("info", info);
  forwardConsole("warn", warn);
  forwardConsole("error", error);
  await nextTick();
  setupTitlebar();
  await store.loadConfig();
  const hasCfg = !!store.config?.email_username;
  setConfigState(hasCfg);
  if (!hasCfg && appRouter.currentRoute.value.name !== "welcome") {
    appRouter.replace("/welcome");
  }
  store.listenForLogs();
  store.listenForMessages();
  await store.initializeApp();
  await store.loadProcesses();
  await store.loadSessions();
});
</script>

<style scoped>
.app-layout {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
.app-layout > .t-layout {
  flex: 1;
  min-height: 0;
  display: flex;
}
.app-aside {
  display: flex !important;
  flex-direction: column;
  align-items: center;
  width: 64px !important;
  min-width: 64px !important;
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
  background: rgba(0, 0, 0, 0.6);
  border-radius: 10px;
}

/* ── Custom titlebar ── */
.titlebar {
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  background: var(--td-bg-color-container);
  border-bottom: 1px solid var(--td-component-stroke);
  user-select: none;
  flex-shrink: 0;
}
.titlebar-controls {
  display: flex;
  height: 100%;
}
.titlebar-btn {
  appearance: none;
  padding: 0;
  margin: 0;
  border: none;
  display: inline-flex;
  justify-content: center;
  align-items: center;
  width: 40px;
  background: transparent;
  color: var(--td-text-color-secondary);
  cursor: pointer;
  transition:
    background 0.12s,
    color 0.12s;
}
.titlebar-btn:hover {
  background: var(--td-bg-color-secondarycontainer);
  color: var(--td-text-color-primary);
}
.titlebar-btn-close:hover {
  background: var(--td-error-color);
  color: #fff;
}
.logo-img {
  width: 32px;
  height: 32px;
}
.aside-nav {
  flex: 1;
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
  transition:
    background 0.15s,
    color 0.15s;
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
