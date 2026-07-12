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
    <button class="attach-btn" @click="$emit('attach')" title="附加文件">
      <svg
        viewBox="0 0 24 24"
        width="22"
        height="22"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <path
          d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48"
        />
      </svg>
    </button>
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
  attach: [];
}>();

const isMobile = useIsMobile();
const isKeyboardOpen = ref(false);
const inputRef = ref<HTMLTextAreaElement | null>(null);

function autoResize(el: HTMLTextAreaElement) {
  const prevHeight = el.offsetHeight;
  el.style.height = "";
  // scrollHeight 在单行时比 CSS 高度大 1-3px，减 3 吸收偏差
  const newH = Math.min(el.scrollHeight - 20, 120);
  if (Math.abs(newH - prevHeight) > 6) {
    el.style.height = newH + "px";
  } else {
    el.style.height = prevHeight + "px";
  }
}

function onInput(e: Event) {
  const el = e.target as HTMLTextAreaElement;
  emit("update:modelValue", el.value);
  autoResize(el);
}

watch(
  () => props.modelValue,
  (v) => {
    if (v) return; // onInput handles resize during typing
    nextTick(() => {
      const el = inputRef.value;
      if (el) el.style.height = "";
    });
  },
);

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
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 12px calc(10px + env(safe-area-inset-bottom, 0px));
  border-top: 1px solid var(--td-component-stroke);
  background: var(--td-bg-color-container);
  transition: padding-bottom 0.2s ease;
}
.input-area.keyboard-open {
  padding-bottom: 10px;
}
.chat-textarea {
  flex: 1;
  resize: none;
  outline: none;
  font-family: inherit;
  font-size: 16px;
  line-height: 1.5;
  padding: 10px 12px;
  color: var(--td-text-color-primary);
  background: var(--td-bg-color-secondarycontainer);
  border: none;
  border-radius: 8px;
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
.attach-btn {
  flex-shrink: 0;
  align-self: flex-start;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  border: 1px solid var(--td-component-stroke);
  background: transparent;
  color: var(--td-text-color-placeholder);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
}
.attach-btn:hover {
  background: var(--td-bg-color-secondarycontainer);
  color: var(--td-brand-color);
}
@media (max-width: 767px) {
  .input-area {
    padding: 10px 10px calc(10px + env(safe-area-inset-bottom, 0px));
    gap: 10px;
  }
  .chat-textarea {
    min-height: 24px;
    font-size: 16px;
  }
  .send-btn {
    height: 44px;
    min-height: 44px;
    font-size: 16px;
    padding: 0 16px;
  }
}
</style>
