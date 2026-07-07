<template>
  <div class="progress-wrap">
    <div class="progress-bar">
      <div class="progress-fill" :style="{ width: pct + '%' }"></div>
    </div>
    <div class="progress-meta">
      <span class="progress-status" v-if="component.status">{{
        component.status
      }}</span>
      <span class="progress-pct">{{ pct }}%</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { ProgressComponent } from "@/types/plugin";

const props = defineProps<{ component: ProgressComponent }>();

const pct = computed(() => {
  const max = props.component.max ?? 100;
  if (max <= 0) return 0;
  return Math.round((props.component.value / max) * 100);
});
</script>

<style scoped>
.progress-wrap {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.progress-bar {
  height: 8px;
  border-radius: 4px;
  background: var(--td-bg-color-component);
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  border-radius: 4px;
  background: var(--td-brand-color);
  transition: width 0.4s ease;
}
.progress-meta {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}
.progress-status {
  color: var(--td-text-color-placeholder);
}
.progress-pct {
  color: var(--td-text-color-primary);
  font-weight: 500;
}
</style>
