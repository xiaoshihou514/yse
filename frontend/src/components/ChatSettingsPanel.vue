<template>
  <div v-if="visible" class="settings-backdrop" @click.self="$emit('close')">
    <div :class="['settings-panel', { 'settings-mobile': isMobile }]">
      <div class="settings-header">
        <span class="settings-back" @click="$emit('close')">
          <ChevronLeftIcon v-if="isMobile" />
          <CloseIcon v-else />
        </span>
        <span class="settings-title">{{ displayName }}</span>
        <span class="settings-spacer"></span>
      </div>
      <div class="settings-body">
        <div class="settings-group">
          <div class="settings-group-label">信息</div>
          <div class="settings-item" @click="$emit('rename')">
            <span class="settings-item-label">显示名称</span>
            <span class="settings-item-value">{{ displayName }}</span>
            <span class="settings-item-arrow"><ChevronRightIcon /></span>
          </div>
          <div class="settings-item" @click="$emit('changeAvatar')">
            <span class="settings-item-label">修改头像</span>
            <div class="avatar-preview-sm">
              <img
                v-if="avatarUrl"
                :src="avatarUrl"
                class="avatar-preview-img"
              />
              <span v-else class="avatar-preview-txt">{{ initial }}</span>
            </div>
            <span class="settings-item-arrow"><ChevronRightIcon /></span>
          </div>
        </div>
        <div class="settings-group">
          <div class="settings-group-label">操作</div>
          <div class="settings-item" @click="$emit('toggleHide')">
            <span class="settings-item-label">{{
              isHidden ? "取消隐藏" : "隐藏对话"
            }}</span>
          </div>
          <div
            class="settings-item settings-item-danger"
            @click="$emit('delete')"
          >
            <span class="settings-item-label">删除对话</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  ChevronLeftIcon,
  CloseIcon,
  ChevronRightIcon,
} from "tdesign-icons-vue-next";
import { useIsMobile } from "@/composables/useIsMobile";
import { useYseStore } from "@/stores/yse";
import { parseAddress } from "@/utils/address";
import { loadAvatar } from "@/composables/useAvatar";

const props = defineProps<{
  visible: boolean;
  address: string;
  isHidden: boolean;
}>();

defineEmits<{
  close: [];
  rename: [];
  changeAvatar: [];
  toggleHide: [];
  delete: [];
}>();

const isMobile = useIsMobile();
const store = useYseStore();

const displayName = computed(() => {
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === props.address,
  );
  if (mapping?.display_name) return mapping.display_name;
  const p = parseAddress(props.address);
  return p.name || props.address;
});

const avatarUrl = computed(() => loadAvatar(props.address));

const initial = computed(() => {
  const p = parseAddress(props.address);
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === props.address,
  );
  const name = mapping?.display_name || p.name;
  return (name.charAt(0) || "?").toUpperCase();
});
</script>

<style scoped>
.settings-backdrop {
  position: fixed;
  inset: 0;
  z-index: 2000;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  justify-content: flex-end;
}
.settings-panel {
  width: 340px;
  max-width: 85vw;
  height: 100%;
  background: var(--td-bg-color-container);
  display: flex;
  flex-direction: column;
  animation: slideRight 0.25s ease;
  box-shadow: -2px 0 12px rgba(0, 0, 0, 0.1);
}
.settings-panel.settings-mobile {
  width: 100%;
  max-width: 100%;
  animation: slideUp 0.3s ease;
  padding-top: env(safe-area-inset-top, 0);
}
.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  border-bottom: 1px solid var(--td-component-stroke);
  flex-shrink: 0;
}
.settings-back {
  cursor: pointer;
  color: var(--td-brand-color);
  font-size: 16px;
  padding: 4px 0;
}
.settings-title {
  font-size: 17px;
  font-weight: 600;
  color: var(--td-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  text-align: center;
}
.settings-spacer {
  width: 60px;
}
.settings-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0 12px;
}
.settings-group {
  margin-bottom: 8px;
}
.settings-group-label {
  font-size: 12px;
  color: var(--td-text-color-placeholder);
  padding: 8px 16px 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.settings-item {
  display: flex;
  align-items: center;
  padding: 14px 16px;
  cursor: pointer;
  transition: background 0.15s;
  background: var(--td-bg-color-container);
}
.settings-item:active {
  background: var(--td-bg-color-secondarycontainer);
}
.settings-item-label {
  font-size: 16px;
  color: var(--td-text-color-primary);
  flex: 1;
}
.settings-item-value {
  font-size: 14px;
  color: var(--td-text-color-placeholder);
  margin-right: 8px;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.settings-item-arrow {
  font-size: 18px;
  color: var(--td-text-color-placeholder);
  display: flex;
  align-items: center;
}
.settings-item-danger .settings-item-label {
  color: var(--td-error-color);
}
.avatar-preview-sm {
  width: 28px;
  height: 28px;
  border-radius: 4px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--td-bg-color-secondarycontainer);
  margin-right: 4px;
}
.avatar-preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.avatar-preview-txt {
  font-size: 12px;
  font-weight: 600;
  color: var(--td-brand-color);
}
@keyframes slideRight {
  from {
    transform: translateX(100%);
  }
  to {
    transform: translateX(0);
  }
}
@keyframes slideUp {
  from {
    transform: translateY(100%);
  }
  to {
    transform: translateY(0);
  }
}
</style>
