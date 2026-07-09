<template>
  <div
    v-if="visible"
    class="context-menu"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <div class="ctx-item" @click="$emit('copy')">
      {{ isContact ? "复制地址" : "复制" }}
    </div>
    <div v-if="isContact" class="ctx-sep"></div>
    <div v-if="isContact" class="ctx-item" @click="$emit('toggleHide')">
      {{ isHidden ? "取消隐藏" : "隐藏对话" }}
    </div>
    <div v-if="!isContact && messageText" class="ctx-sep"></div>
    <div
      v-if="!isContact && messageText"
      class="ctx-item"
      @click="$emit('forward')"
    >
      转发
    </div>
    <div v-if="isContact" class="ctx-sep"></div>
    <div v-if="isContact" class="ctx-item ctx-danger" @click="$emit('delete')">
      删除对话
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  visible: boolean;
  x: number;
  y: number;
  isContact: boolean;
  isHidden?: boolean;
  messageText?: string;
}>();

defineEmits<{
  copy: [];
  toggleHide: [];
  forward: [];
  delete: [];
}>();
</script>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 2000;
  background: var(--td-bg-color-container);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
  padding: 4px 0;
  min-width: 140px;
}
.ctx-item {
  padding: 8px 16px;
  font-size: 14px;
  cursor: pointer;
  color: var(--td-text-color-primary);
  transition: background 0.1s;
}
.ctx-item:hover {
  background: var(--td-bg-color-secondarycontainer);
}
.ctx-item.ctx-danger {
  color: var(--td-error-color);
}
.ctx-sep {
  height: 1px;
  margin: 4px 12px;
  background: var(--td-component-stroke);
}
</style>
