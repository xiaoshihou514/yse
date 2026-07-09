<template>
  <div
    :class="['contact-item', { active, hidden: contact.hidden }]"
    @click="$emit('select', contact.address)"
    @contextmenu.prevent="$emit('context', $event, contact)"
    @touchstart.passive="emit('touchStart', $event, contact)"
    @touchend="emit('touchEnd', $event)"
    @touchmove="emit('touchMove', $event)"
  >
    <ContactAvatar :address="contact.address">
      <template #badge>
        <span v-if="contact.hasNew" class="new-dot"></span>
      </template>
    </ContactAvatar>
    <div class="contact-info">
      <div class="contact-row1">
        <span class="contact-name">{{ displayName }}</span>
        <span class="contact-hostname">{{ hostnameLabel }}</span>
      </div>
      <div class="contact-row2">
        <span class="contact-text">{{ contact.lastText || "" }}</span>
        <span class="contact-time">{{
          contact.lastTime ? formatTime(contact.lastTime) : ""
        }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import ContactAvatar from "./ContactAvatar.vue";
import { computed } from "vue";
import { useYseStore } from "@/stores/yse";
import { parseAddress } from "@/utils/address";

interface Contact {
  address: string;
  lastText: string;
  lastTime: number;
  hostname: string;
  hidden: boolean;
  hasNew?: boolean;
  lastIsSelf?: boolean;
}

const props = defineProps<{ contact: Contact; active: boolean }>();
const emit = defineEmits<{
  select: [addr: string];
  context: [e: MouseEvent, c: Contact];
  touchStart: [e: TouchEvent, c: Contact];
  touchEnd: [e: TouchEvent];
  touchMove: [e: TouchEvent];
}>();

const store = useYseStore();

const displayName = computed(() => {
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === props.contact.address,
  );
  if (mapping?.display_name) return mapping.display_name;
  const p = parseAddress(props.contact.address);
  return p.name || props.contact.address;
});

const hostnameLabel = props.contact.hostname
  ? `@${props.contact.hostname}`
  : "";

function formatTime(ts: number) {
  const d = new Date(ts);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay)
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  return d.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
}
</script>

<style scoped>
.contact-item {
  display: flex; align-items: center; gap: 10px;
  padding: 10px 12px; cursor: pointer; transition: background 0.1s;
}
.contact-item:hover { background: var(--td-bg-color-secondarycontainer); }
.contact-item.active { background: var(--td-brand-color-light); }
.contact-item.hidden { opacity: 0.5; }
.contact-info { flex: 1; min-width: 0; }
.contact-row1 { display: flex; justify-content: space-between; align-items: center; }
.contact-row2 {
  display: flex; justify-content: space-between; align-items: center;
  margin-top: 3px;
}
.contact-name {
  font-size: 14px; font-weight: 500; white-space: nowrap; overflow: hidden;
  text-overflow: ellipsis; flex: 1; min-width: 0;
}
.contact-hostname {
  font-size: 10px; color: var(--td-text-color-placeholder); white-space: nowrap;
  max-width: 80px; overflow: hidden; text-overflow: ellipsis; flex-shrink: 0;
  margin-left: 6px;
}
.contact-text {
  font-size: 12px; color: var(--td-text-color-secondary); white-space: nowrap;
  overflow: hidden; text-overflow: ellipsis; flex: 1; min-width: 0;
}
.contact-time {
  font-size: 11px; color: var(--td-text-color-placeholder); white-space: nowrap;
  flex-shrink: 0; margin-left: 6px;
}
</style>
