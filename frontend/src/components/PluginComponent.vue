<template>
  <div class="plugin-component">
    <PluginListSelect
      v-if="comp?.type === 'list'"
      :component="comp"
      @select="$emit('respond', $event)"
    />
    <PluginProgressBar
      v-else-if="comp?.type === 'progress'"
      :component="comp"
    />
  </div>
</template>

<script setup lang="ts">
import type { PluginComponent as PC } from "@/types/plugin";
import PluginListSelect from "./PluginListSelect.vue";
import PluginProgressBar from "./PluginProgressBar.vue";

defineProps<{ comp: PC }>();
defineEmits<{ respond: [value: string] }>();
</script>

<style scoped>
.plugin-component { margin-top: 8px; }
</style>
