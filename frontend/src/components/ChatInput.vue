<template>
  <div :class="['input-area', { 'keyboard-open': isKeyboardOpen && isMobile }]">
    <textarea
      ref="inputRef"
      :value="modelValue"
      placeholder="输入消息..."
      rows="1"
      class="chat-textarea"
      @input="onInput"
      @keydown="onKeydown"
      @focus="isKeyboardOpen = true"
      @blur="onBlur"
    ></textarea>
    <t-button
      class="send-btn"
      :disabled="!modelValue.trim()"
      size="small"
      @click="$emit('send')"
      >发送</t-button
    >
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from "vue";
import { useIsMobile } from "@/composables/useIsMobile";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{
  "update:modelValue": [value: string];
  send: [];
}>();

const isMobile = useIsMobile();
const isKeyboardOpen = ref(false);
const inputRef = ref<HTMLTextAreaElement | null>(null);

watch(
  () => props.modelValue,
  () => {
    nextTick(() => {
      const el = inputRef.value;
      if (el) {
        el.style.height = "";
        el.style.height = Math.min(el.scrollHeight, 120) + "px";
      }
    });
  },
);

function onInput(e: Event) {
  const el = e.target as HTMLTextAreaElement;
  emit("update:modelValue", el.value);
  autoResize(el);
}

function autoResize(el: HTMLTextAreaElement) {
  const prevHeight = el.offsetHeight;
  el.style.height = "";
  const newH = Math.min(el.scrollHeight, 120);
  if (Math.abs(newH - prevHeight) > 6) {
    el.style.height = newH + "px";
  } else {
    el.style.height = prevHeight + "px";
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    emit("send");
  }
}

function onBlur() {
  setTimeout(() => {
    isKeyboardOpen.value = false;
  }, 150);
}

defineExpose({ inputRef });
</script>

<style scoped>
.input-area {
  display: flex; align-items: flex-start; gap: 12px;
  padding: 10px 12px calc(10px + env(safe-area-inset-bottom, 0px));
  border-top: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  transition: padding-bottom 0.2s ease;
}
.input-area.keyboard-open { padding-bottom: 10px; }
.chat-textarea {
  flex: 1; resize: none; outline: none; font-family: inherit;
  font-size: 16px; line-height: 1.5; padding: 10px 12px;
  color: var(--td-text-color-primary);
  background: var(--td-bg-color-secondarycontainer);
  border: none; border-radius: 8px;
}
.send-btn {
  flex-shrink: 0;
  align-self: flex-start;
  height: 44px;
  display: flex;
  align-items: center;
  font-size: 18px;
  font-weight: 500;
  padding: 0 15px;
  border-radius: 8px;
}
@media (max-width: 767px) {
  .input-area { padding: 10px 10px calc(10px + env(safe-area-inset-bottom, 0px)); gap: 10px; }
  .chat-textarea { min-height: 44px; font-size: 16px; }
  .send-btn { height: 44px; min-height: 44px; font-size: 16px; padding: 0 16px; }
}
</style>
