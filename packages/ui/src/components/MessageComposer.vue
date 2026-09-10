<script setup lang="ts">
import { ref } from "vue";
const props = withDefaults(
  defineProps<{
    modelValue: string;
    disabled?: boolean;
    busy?: boolean;
    canCancel?: boolean;
    placeholder?: string;
  }>(),
  { placeholder: "输入消息…", disabled: false, busy: false, canCancel: false },
);
const emit = defineEmits<{
  "update:modelValue": [text: string];
  submit: [text: string];
  cancel: [];
}>();
const collapsed = ref(false);
function submit() {
  const text = props.modelValue.trim();
  if (!text || props.disabled || props.busy || collapsed.value) return;
  emit("submit", text);
}
function keydown(event: KeyboardEvent) {
  if (
    event.key === "Enter" &&
    !event.shiftKey &&
    !event.isComposing &&
    event.keyCode !== 229
  ) {
    event.preventDefault();
    submit();
  }
}
function input(event: Event) {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}
</script>
<template>
  <form class="rss-ui composer" @submit.prevent="submit">
    <div class="toolbar">
      <button
        type="button"
        :aria-expanded="!collapsed"
        @click="collapsed = !collapsed"
      >
        {{ collapsed ? "展开输入" : "收起输入" }}
      </button>
      <button
        v-if="canCancel"
        type="button"
        data-action="cancel"
        :disabled="disabled"
        @click="!disabled && emit('cancel')"
      >
        取消
      </button>
      <button
        type="submit"
        :disabled="disabled || busy || collapsed || !modelValue.trim()"
      >
        发送
      </button>
    </div>
    <template v-if="!collapsed">
      <label
        >消息<textarea
          :value="modelValue"
          :disabled="disabled || busy"
          :placeholder="placeholder"
          rows="3"
          @input="input"
          @keydown="keydown"
        />
      </label>
      <p>Enter 发送 · Shift+Enter 换行</p>
    </template>
  </form>
</template>
<style scoped>
.composer {
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-md);
  padding: 12px;
  background: var(--color-surface);
}
.toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.toolbar button:first-child {
  margin-right: auto;
}
button {
  padding: 6px 12px;
  background: var(--color-bg);
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-sm);
}
button[type="submit"] {
  background: var(--color-accent);
  color: white;
}
label {
  display: block;
}
textarea {
  display: block;
  width: 100%;
  resize: vertical;
  min-height: 80px;
  max-height: 250px;
  margin-top: 6px;
  border: 1px solid var(--color-border-strong);
  background: var(--color-bg);
  padding: 10px;
  border-radius: var(--radius-sm);
}
p {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--color-text-muted);
}
</style>
