<template>
  <div class="contact-avatar">
    <div class="avatar-box">
      <img v-if="avatarUrl" :src="avatarUrl" class="avatar-img" />
      <span v-else class="avatar-initial">{{ initial }}</span>
    </div>
    <slot name="badge" />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { parseAddress } from "@/utils/address";
import { loadAvatar } from "@/composables/useAvatar";
import { useYseStore } from "@/stores/yse";

const props = defineProps<{ address: string }>();
const store = useYseStore();

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
.contact-avatar {
  position: relative;
  flex-shrink: 0;
}
.avatar-box {
  width: 40px;
  height: 40px;
  border-radius: 4px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--td-bg-color-secondarycontainer);
}
.avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.avatar-initial {
  font-size: 16px;
  font-weight: 600;
  color: var(--td-brand-color);
  user-select: none;
}
</style>
