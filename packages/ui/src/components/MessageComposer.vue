<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ArrowUp, Square } from "@lucide/vue";
const composing = ref(false);
const editor = ref<HTMLTextAreaElement>();
let sizing: ResizeObserver | undefined;
let editorWidth = 0;
function fit() {
  const el = editor.value;
  if (!el || !el.getClientRects().length) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
}
onMounted(() => {
  fit();
  sizing = new ResizeObserver((entries) => {
    const width = entries[0]?.contentRect.width ?? 0;
    if (width !== editorWidth) {
      editorWidth = width;
      fit();
    }
  });
  if (editor.value) sizing.observe(editor.value);
});
onBeforeUnmount(() => sizing?.disconnect());
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
watch(
  () => props.modelValue,
  async () => {
    await nextTick();
    fit();
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
    <div class="composer-box">
      <slot name="context" />
      <label
        ><span class="sr-only">消息</span
        ><textarea
          ref="editor"
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
        <div class="composer-tools"><slot name="tools" /></div>
        <button
          v-if="canCancel"
          type="button"
          data-action="cancel"
          :disabled="disabled"
          title="停止回复"
          @click="!disabled && emit('cancel')"
        >
          <Square :size="12" aria-hidden="true" /><span>停止回复</span>
        </button>
        <button
          type="submit"
          :aria-label="submitLabel"
          :title="submitLabel"
          :disabled="disabled || busy || !canSubmit || !modelValue.trim()"
        >
          <ArrowUp :size="18" aria-hidden="true" /><span class="sr-only">{{
            submitLabel
          }}</span>
        </button>
      </div>
    </div>
    <p class="composer-hint">Enter 发送<span> · </span>Shift+Enter 换行</p>
  </form>
</template>
<style scoped>
/* ref: vscode 1.105.0 chat/browser/media/chat.css — shared input surface with a compact toolbar. */
.composer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.composer-box {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  padding: 14px;
  border: 1px solid var(--rss-color-border-strong);
  border-radius: 16px;
  background: var(--rss-color-surface);
  box-shadow: 0 2px 8px #18223006;
  transition:
    border-color 140ms ease,
    box-shadow 140ms ease;
}
.composer-box:focus-within {
  border-color: color-mix(
    in srgb,
    var(--rss-color-accent) 45%,
    var(--rss-color-border)
  );
  box-shadow: 0 0 0 3px
    color-mix(in srgb, var(--rss-color-accent) 6%, transparent);
}
.toolbar {
  display: flex;
  gap: 8px;
  align-items: center;
  min-width: 0;
}
.composer-tools {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}
.composer .toolbar button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex: none;
  min-height: 30px;
  height: 30px;
}
.composer .toolbar button[data-action="cancel"] {
  padding: 0 8px;
  border-color: var(--rss-color-border);
  font-size: 11px;
  background: transparent;
}
.composer .toolbar button[type="submit"] {
  width: 30px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: var(--rss-color-text);
  color: var(--rss-color-surface);
}
.composer .toolbar button[type="submit"]:disabled {
  opacity: 1;
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text-muted);
}
.composer .toolbar button[type="submit"]:hover:not(:disabled) {
  background: var(--rss-color-accent);
  color: var(--rss-color-on-accent);
}
label {
  display: block;
  min-width: 0;
}
.composer textarea {
  display: block;
  width: 100%;
  resize: none;
  min-height: var(--rss-composer-min-height, 64px);
  max-height: 160px;
  border: 0;
  background: transparent;
  padding: 2px;
  border-radius: 0;
  font-size: 14px;
  line-height: 1.7;
  outline: none;
  box-shadow: none;
  scrollbar-width: thin;
}
textarea::placeholder {
  color: var(--rss-color-text-muted);
}
.composer-hint {
  margin: 0;
  text-align: center;
  font-size: 10px;
  line-height: 1.5;
  color: var(--rss-color-text-muted);
}
@media (max-height: 520px) {
  .composer-box {
    padding: 10px;
    gap: 6px;
  }
  .composer textarea {
    min-height: 40px;
    max-height: 72px;
  }
  .composer-hint {
    display: none;
  }
}
</style>
