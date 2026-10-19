<script setup lang="ts">
import { ref } from "vue";
import { nativeWindow } from "./window";
const controls = nativeWindow();
const error = ref("");
async function act(action: "minimize" | "maximize" | "close") {
  error.value = "";
  try {
    if (action === "minimize") await controls?.minimize();
    else if (action === "maximize") await controls?.maximize();
    else await controls?.close();
  } catch {
    error.value = "窗口操作未完成，请重试";
  }
}
</script>
<template>
  <div v-if="controls" class="window-controls" aria-label="窗口控制">
    <button type="button" aria-label="最小化窗口" @click="act('minimize')">
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M1 6h10" /></svg>
    </button>
    <button
      type="button"
      aria-label="最大化或还原窗口"
      @click="act('maximize')"
    >
      <svg viewBox="0 0 12 12" aria-hidden="true">
        <path d="M1.5 1.5h9v9h-9z" />
      </svg>
    </button>
    <button
      type="button"
      aria-label="关闭窗口"
      class="window-close"
      @click="act('close')"
    >
      <svg viewBox="0 0 12 12" aria-hidden="true">
        <path d="m1.5 1.5 9 9m0-9-9 9" />
      </svg>
    </button>
    <span v-if="error" role="alert" class="window-error">{{ error }}</span>
  </div>
</template>
<style scoped>
.window-controls {
  display: flex;
  flex: none;
  align-self: stretch;
  margin: -8px -8px -8px 0;
}
.window-controls button {
  display: grid;
  place-items: center;
  width: 44px;
  padding: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--rss-color-text);
}
.window-controls button:hover {
  background: var(--rss-color-neutral-bg);
}
.window-controls .window-close:hover {
  background: #c42b1c;
  color: #fff;
}
svg {
  width: 11px;
  height: 11px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1;
}
.window-error {
  position: absolute;
  right: 8px;
  top: 100%;
  padding: 8px 12px;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
  border: 1px solid var(--rss-color-border);
  border-radius: 6px;
  font-size: 12px;
}
@media (max-width: 600px) {
  .window-controls button {
    width: 32px;
  }
}
</style>
