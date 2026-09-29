<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue: string;
    disabled?: boolean;
    busy?: boolean;
    canCancel?: boolean;
    canSubmit?: boolean;
    placeholder?: string;
    submitLabel?: string;
  }>(),
  {
    placeholder: "输入消息…",
    submitLabel: "发送",
    disabled: false,
    busy: false,
    canCancel: false,
    canSubmit: true,
  },
);
const emit = defineEmits<{
  "update:modelValue": [text: string];
  submit: [text: string];
  cancel: [];
}>();
function submit() {
  const text = props.modelValue.trim();
  if (!text || props.disabled || props.busy || !props.canSubmit) return;
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
        v-if="canCancel"
        type="button"
        data-action="cancel"
        :disabled="disabled"
        @click="!disabled && emit('cancel')"
      >
        停止本轮
      </button>
      <button
        type="submit"
        :disabled="disabled || busy || !canSubmit || !modelValue.trim()"
      >
        {{ submitLabel }}
      </button>
    </div>
    <label
      >消息<textarea
        :value="modelValue"
        :disabled="disabled"
        :placeholder="placeholder"
        rows="3"
        @input="input"
        @keydown="keydown"
      />
    </label>
    <p>Enter 发送 · Shift+Enter 换行</p>
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
  box-sizing: border-box;
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
