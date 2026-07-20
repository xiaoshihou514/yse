<template>
  <div class="picker-dialog">
    <!-- Header -->
    <div class="picker-header">
      <span class="picker-title">{{ title }}</span>
      <span class="picker-close" @click="$emit('cancel')">
        <t-icon name="close" />
      </span>
    </div>

    <!-- Body: left-right split -->
    <div class="picker-body">
      <!-- Left: contact list -->
      <div class="picker-list">
        <div class="picker-select-all">
          <label>
            <t-checkbox
              :checked="isAllSelected"
              :indeterminate="isIndeterminate"
              @change="toggleAll"
            />
            全选
          </label>
        </div>
        <div class="picker-items">
          <div
            v-for="contact in contacts"
            :key="contact"
            :class="['picker-item', { selected: isSelected(contact) }]"
            @click="toggleContact(contact)"
          >
            <t-checkbox :checked="isSelected(contact)" @click.stop />
            <ContactAvatar :address="contact" />
            <span class="picker-contact-name">{{ nameFromAddr(contact) }}</span>
          </div>
        </div>
      </div>

      <!-- Right: selected summary -->
      <div class="picker-selected">
        <div class="picker-count">
          已选择（{{ selectedList.length }}/{{ max }}）
        </div>
        <div class="picker-tags">
          <t-tag
            v-for="addr in selectedList"
            :key="addr"
            closable
            size="small"
            @close="removeSelected(addr)"
          >
            {{ nameFromAddr(addr) }}
          </t-tag>
          <span v-if="selectedList.length === 0" class="picker-empty-hint">
            暂无选择
          </span>
        </div>
      </div>
    </div>

    <!-- Footer -->
    <div class="picker-footer">
      <t-button variant="outline" @click="$emit('cancel')">取消</t-button>
      <t-button @click="confirm">确定</t-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { useYseStore } from "@/stores/yse";
import { nameFromAddr } from "@/utils/address";
import ContactAvatar from "@/components/ContactAvatar.vue";
import type { Message } from "@/api/commands";

const props = withDefaults(
  defineProps<{
    title?: string;
    multiple?: boolean;
    max?: number;
    filterAddr?: string;
  }>(),
  {
    title: "选择联系人",
    multiple: true,
    max: 10,
    filterAddr: "",
  },
);

const emit = defineEmits<{
  confirm: [selected: string[]];
  cancel: [];
}>();

const store = useYseStore();

// Derive unique contacts from messages
const contacts = computed(() => {
  const addrs = new Set<string>();
  for (const msg of store.messages as Message[]) {
    if (!msg.from) continue;
    // If filterAddr is set, only show contacts from that conversation
    if (props.filterAddr) {
<<<<<<< Updated upstream
      if (msg.from !== props.filterAddr && msg.to !== props.filterAddr)
        continue;
=======
      if (msg.from !== props.filterAddr && msg.to !== props.filterAddr) continue;
>>>>>>> Stashed changes
      // Include both participants of the conversation
      if (msg.from !== msg.to) {
        addrs.add(msg.from);
        addrs.add(msg.to);
      } else {
        addrs.add(msg.from);
      }
    } else {
      addrs.add(msg.from);
    }
  }
  return Array.from(addrs).sort((a, b) =>
    nameFromAddr(a).localeCompare(nameFromAddr(b)),
  );
});

// Selection state
const selectedList = ref<string[]>([]);

const selectedSet = computed(() => new Set(selectedList.value));
<<<<<<< Updated upstream
function isSelected(addr: string) {
  return selectedSet.value.has(addr);
}
const isAllSelected = computed(
  () =>
    contacts.value.length > 0 &&
    contacts.value.every((c) => selectedSet.value.has(c)),
);
const isIndeterminate = computed(
  () =>
    !isAllSelected.value &&
    contacts.value.some((c) => selectedSet.value.has(c)),
=======
function isSelected(addr: string) { return selectedSet.value.has(addr); }
const isAllSelected = computed(
  () => contacts.value.length > 0 && contacts.value.every((c) => selectedSet.value.has(c)),
);
const isIndeterminate = computed(
  () => !isAllSelected.value && contacts.value.some((c) => selectedSet.value.has(c)),
>>>>>>> Stashed changes
);

function toggleAll(checked: boolean) {
  if (checked) {
    selectedList.value = contacts.value.slice(0, props.max);
  } else {
    selectedList.value = [];
  }
}

function removeSelected(addr: string) {
  selectedList.value = selectedList.value.filter((a) => a !== addr);
}

function toggleContact(addr: string) {
  const s = new Set(selectedList.value);
  if (s.has(addr)) {
    s.delete(addr);
  } else if (!props.multiple) {
    s.clear();
    s.add(addr);
  } else if (s.size < props.max) {
    s.add(addr);
  }
  selectedList.value = Array.from(s);
}

function confirm() {
  emit("confirm", selectedList.value);
}
</script>

<style scoped>
.picker-dialog {
  width: 560px;
  max-width: 90vw;
  max-height: 80vh;
  background: var(--td-bg-color-container);
  border-radius: 12px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.2);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Header */
.picker-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--td-component-stroke);
}
.picker-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--td-text-color-primary);
}
.picker-close {
  cursor: pointer;
  font-size: 18px;
  color: var(--td-text-color-secondary);
  display: flex;
  align-items: center;
  padding: 4px;
  border-radius: 4px;
}
.picker-close:hover {
  background: var(--td-bg-color-secondarycontainer);
}

/* Body */
.picker-body {
  display: flex;
  flex: 1;
  min-height: 300px;
  overflow: hidden;
}

/* Left: list */
.picker-list {
  flex: 1;
  border-right: 1px solid var(--td-component-stroke);
  display: flex;
  flex-direction: column;
}
.picker-select-all {
  padding: 12px 16px;
  border-bottom: 1px solid var(--td-component-stroke);
  font-size: 14px;
  color: var(--td-text-color-primary);
}
.picker-select-all label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}
.picker-items {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}
.picker-item {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  gap: 10px;
  cursor: pointer;
  transition: background 0.15s;
}
.picker-item:hover {
  background: var(--td-bg-color-secondarycontainer);
}
.picker-item.selected {
  background: var(--td-brand-color-light);
}
.picker-contact-name {
  font-size: 14px;
  color: var(--td-text-color-primary);
}

/* Right: selected */
.picker-selected {
  width: 200px;
  display: flex;
  flex-direction: column;
  padding: 16px;
}
.picker-count {
  font-size: 14px;
  font-weight: 500;
  color: var(--td-text-color-primary);
  margin-bottom: 12px;
}
.picker-tags {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.t-tag {
  --td-tag-default-border-color: transparent;
}
.picker-empty-hint {
  font-size: 13px;
  color: var(--td-text-color-placeholder);
}

/* Footer */
.picker-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid var(--td-component-stroke);
}
</style>
