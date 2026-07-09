<template>
  <t-dialog
    :visible="visible"
    header="转发消息"
    width="400px"
    :close-on-overlay-click="true"
    :footer="false"
    @close="$emit('close')"
  >
    <t-input
      v-model="search"
      placeholder="搜索联系人..."
      size="small"
      clearable
      style="margin-bottom: 8px"
    />
    <div class="forward-list" :style="{ maxHeight: isMobile ? '40vh' : '300px' }">
      <div
        v-for="c in filteredContacts"
        :key="c.address"
        class="forward-contact"
        @click="toggle(c.address)"
      >
        <t-checkbox
          :checked="selected.has(c.address)"
          :style="{ pointerEvents: 'none' }"
        />
        <span class="forward-name">{{ displayName(c) }}</span>
        <span class="forward-hostname">{{
          c.hostname ? "@" + c.hostname : ""
        }}</span>
      </div>
      <t-empty v-if="filteredContacts.length === 0" description="暂无联系人" />
    </div>
    <div class="forward-actions">
      <t-button size="small" variant="text" @click="selectAll">全选</t-button>
      <t-button size="small" variant="text" @click="clear">清空</t-button>
    </div>
    <t-textarea
      v-model="extraText"
      placeholder="添加附加内容（可选）"
      :autosize="{ minRows: 2, maxRows: 4 }"
      style="margin-top: 8px"
    />
    <t-space style="margin-top: 12px; justify-content: flex-end; width: 100%">
      <t-button variant="outline" @click="$emit('close')">取消</t-button>
      <t-button
        theme="primary"
        :disabled="selected.size === 0"
        @click="$emit('confirm', [...selected], extraText)"
      >
        发送 ({{ selected.size }})
      </t-button>
    </t-space>
  </t-dialog>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { useYseStore } from "@/stores/yse";
import { useIsMobile } from "@/composables/useIsMobile";

interface Contact {
  address: string;
  hostname: string;
  hidden: boolean;
}

const props = defineProps<{
  visible: boolean;
  contacts: Contact[];
}>();

defineEmits<{
  close: [];
  confirm: [targets: string[], extraText: string];
}>();

const store = useYseStore();
const isMobile = useIsMobile();
const search = ref("");
const selected = ref(new Set<string>());
const extraText = ref("");

const filteredContacts = computed(() => {
  let list = props.contacts.filter(
    (c) => !c.hidden || store.hiddenAddresses.has(c.address),
  );
  if (search.value) {
    const q = search.value.toLowerCase();
    list = list.filter(
      (c) =>
        displayName(c).toLowerCase().includes(q) ||
        (c.hostname || "").toLowerCase().includes(q),
    );
  }
  return list;
});

function displayName(c: Contact) {
  const mapping = (store.config?.plugin_mappings ?? []).find(
    (m) => m.virtual_addr === c.address,
  );
  if (mapping?.display_name) return mapping.display_name;
  return c.address;
}

function toggle(addr: string) {
  const s = new Set(selected.value);
  if (s.has(addr)) s.delete(addr);
  else s.add(addr);
  selected.value = s;
}

function selectAll() {
  selected.value = new Set(filteredContacts.value.map((c) => c.address));
}

function clear() {
  selected.value = new Set();
  extraText.value = "";
  search.value = "";
}

defineExpose({ clear });
</script>

<style scoped>
.forward-list { overflow-y: auto; border: 1px solid var(--td-component-stroke); border-radius: 6px; }
.forward-contact {
  display: flex; align-items: center; gap: 8px; padding: 10px 12px; cursor: pointer;
  border-bottom: 1px solid var(--td-component-stroke); transition: background 0.1s;
}
.forward-contact:last-child { border-bottom: none; }
.forward-contact:active { background: var(--td-bg-color-secondarycontainer); }
.forward-name {
  font-size: 14px; color: var(--td-text-color-primary); flex: 1; min-width: 0;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.forward-hostname { font-size: 11px; color: var(--td-text-color-placeholder); flex-shrink: 0; }
.forward-actions { display: flex; gap: 8px; padding: 4px 0; }
</style>
