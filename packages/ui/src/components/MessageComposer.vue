<script setup lang="ts">
import { ref } from "vue";
const composing = ref(false);
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
  if (
    composing.value ||
    !text ||
    props.disabled ||
    props.busy ||
    !props.canSubmit
  )
    return;
  emit("submit", text);
}
function keydown(event: KeyboardEvent) {
  if (
    event.key === "Enter" &&
    !event.shiftKey &&
    !composing.value &&
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
    <label
      >消息<textarea
        :value="modelValue"
        :disabled="disabled"
        :placeholder="placeholder"
        rows="2"
        @compositionstart="composing = true"
        @compositionend="composing = false"
        @input="input"
        @keydown="keydown"
      />
    </label>
    <div class="toolbar">
      <button
        v-if="canCancel"
        type="button"
        data-action="cancel"
        :disabled="disabled"
        @click="!disabled && emit('cancel')"
      >
        停止回复
      </button>
      <button
        type="submit"
        :disabled="disabled || busy || !canSubmit || !modelValue.trim()"
      >
        {{ submitLabel }}
      </button>
    </div>
    <p>Enter 发送 · Shift+Enter 换行</p>
  </form>
</template>
<style scoped>
.composer {
  border: 1px solid var(--rss-color-border-strong);
  border-radius: var(--rss-radius-lg);
  padding: 12px;
  background: var(--rss-color-surface);
  display: flex;
  flex-direction: column;
}
.toolbar {
  display: flex;
  gap: 8px;
  order: 2;
  align-items: center;
}
.toolbar button:first-child {
  margin-left: auto;
}
.toolbar button[data-action="cancel"] {
  margin-left: 0;
  margin-right: auto;
}
button[type="submit"] {
  background: var(--rss-color-accent);
  color: var(--rss-color-on-accent);
}
label {
  display: block;
  font-size: 0;
}
textarea {
  display: block;
  width: 100%;
  resize: vertical;
  min-height: 64px;
  max-height: 160px;
  border: 0;
  background: transparent;
  padding: 4px;
  border-radius: var(--rss-radius-sm);
  font-size: var(--rss-font-size-lg);
}
p {
  margin: 8px 0 0;
  font-size: var(--rss-font-size-xs);
  color: var(--rss-color-text-muted);
  order: 3;
}
@media (max-height: 520px) {
  .composer {
    padding: 8px;
  }
  textarea {
    min-height: 44px;
    max-height: 72px;
  }
  p {
    display: none;
  }
}
</style>
