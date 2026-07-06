<template>
  <div class="list-select">
    <div class="list-title" v-if="component.title">{{ component.title }}</div>
    <div
      v-for="opt in component.options"
      :key="opt.value"
      class="list-option"
      @click="$emit('select', opt.value)"
    >
      <span class="opt-label">{{ opt.label }}</span>
      <span class="opt-desc" v-if="opt.description">{{ opt.description }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ListComponent } from "@/types/plugin";

defineProps<{ component: ListComponent }>();
defineEmits<{ select: [value: string] }>();
</script>

<style scoped>
.list-select {
  display: flex; flex-direction: column; gap: 4px;
}
.list-title {
  font-size: 13px; font-weight: 600;
  margin-bottom: 4px;
  color: var(--td-text-color-primary);
}
.list-option {
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  border: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-secondarycontainer);
  transition: all 0.15s;
  display: flex; flex-direction: column; gap: 2px;
}
.list-option:hover {
  border-color: var(--td-brand-color);
  background: var(--td-brand-color-light);
}
.opt-label { font-size: 14px; font-weight: 500; }
.opt-desc { font-size: 12px; color: var(--td-text-color-placeholder); }
</style>
