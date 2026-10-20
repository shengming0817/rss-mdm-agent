<script setup lang="ts">
import { Sparkles, X } from "@rss-mdm-agent/ui";
import SettingsGlyph from "../system-settings/SettingsGlyph.vue";
import ConversationShortcut from "./ConversationShortcut.vue";
import type { ResourceContext } from "../assistant/resource-context";
import type { AssistantController } from "../assistant/controller";
defineProps<{
  context: ResourceContext;
  controller: AssistantController;
  historyOpen: boolean;
}>();
const emit = defineEmits<{ close: []; main: []; history: []; attach: [] }>();
</script>
<template>
  <section class="page-assistant" aria-label="页面电脑助手">
    <header class="page-assistant-heading">
      <h2><Sparkles :size="16" aria-hidden="true" />电脑助手</h2>
      <div class="page-assistant-actions">
        <ConversationShortcut
          :controller="controller"
          :history-open="historyOpen"
          @history="emit('history')"
          @select="emit('attach')"
        />
        <button
          type="button"
          aria-label="在 AI 页面继续"
          title="在 AI 页面继续"
          @click="emit('main')"
        >
          <SettingsGlyph kind="display" :size="15" />
        </button>
        <button
          type="button"
          aria-label="关闭电脑助手侧栏"
          title="关闭电脑助手侧栏"
          @click="emit('close')"
        >
          <X :size="16" aria-hidden="true" />
        </button>
      </div>
    </header>
    <p class="page-assistant-location" :title="context.path.join(' › ')">
      <span>当前页面</span>{{ context.path.join(" › ") }}
    </p>
    <div class="page-assistant-host"><slot /></div>
  </section>
</template>
<style scoped>
.page-assistant {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  padding: 0 16px 12px;
  background: var(--rss-color-bg);
}
.page-assistant-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: none;
  gap: 8px;
  min-height: 52px;
}
.page-assistant-heading h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  margin: 0;
  white-space: nowrap;
}
.page-assistant-heading h2 > svg {
  color: var(--rss-color-text-muted);
}
.page-assistant-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}
.page-assistant-heading .page-assistant-actions button,
.page-assistant-actions :deep(.conversation-shortcuts button) {
  display: grid;
  place-items: center;
  flex: none;
  width: 28px;
  min-height: 28px;
  height: 28px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--rss-color-text-muted);
}
.page-assistant-heading button:hover,
.page-assistant-actions :deep(.conversation-shortcuts button:hover) {
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text);
}
.page-assistant-actions :deep(.conversation-shortcuts) {
  display: flex;
  align-items: center;
  flex: none;
  flex-direction: row;
  gap: 2px;
  padding: 0;
  margin: 0;
  border: 0;
}
.page-assistant-actions :deep(.conversation-shortcuts button > span) {
  display: none;
}
.page-assistant-location {
  flex: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin: 0 0 8px;
  padding: 0 0 12px;
  border-bottom: 1px solid var(--rss-color-border);
  color: var(--rss-color-text-muted);
  font-size: 11px;
  line-height: 1.5;
}
.page-assistant-location > span {
  margin-right: 8px;
  opacity: 0.7;
}
.page-assistant-host {
  flex: 1;
  min-height: 0;
}
.page-assistant-host :deep(.assistant-start) {
  overflow: hidden;
}
.page-assistant-host :deep(.assistant-welcome) {
  flex: 1;
  min-height: 0;
  overflow: auto;
  margin: 0;
  padding: 28px 8px 16px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}
.page-assistant-host :deep(.welcome-mark) {
  display: grid;
  margin: 0 auto 16px;
  width: 36px;
  height: 36px;
}
.page-assistant-host :deep(.assistant-welcome h2) {
  font-size: 20px;
  margin-bottom: 10px;
  letter-spacing: -0.4px;
}
.page-assistant-host :deep(.assistant-welcome p) {
  font-size: 12px;
  max-width: 240px;
  margin-inline: auto;
}
.page-assistant-host :deep(.assistant-timeline) {
  padding: 12px 0;
}
.page-assistant-host :deep(.assistant-timeline > article) {
  margin: 20px 0;
}
.page-assistant-host :deep(.conversation-input) {
  padding: 12px 0 0;
}
.page-assistant-host :deep(.composer) {
  --rss-composer-min-height: 52px;
}
.page-assistant-host :deep(.composer-hint) {
  font-size: 9px;
}
.page-assistant-host :deep(.assistant-start-followup) {
  padding: 8px 0 12px;
}
.page-assistant-host :deep(.start-suggestions) {
  flex-direction: row;
  flex-wrap: wrap;
  gap: 6px;
}
.page-assistant-host :deep(.start-suggestions button) {
  min-height: 30px;
  font-size: 11px;
  padding: 5px 10px;
}
@media (max-height: 620px) {
  .page-assistant-host :deep(.assistant-welcome) {
    padding-top: 8px;
  }
  .page-assistant-host :deep(.assistant-welcome .welcome-mark) {
    display: none;
  }
  .page-assistant-host :deep(.assistant-start-followup) {
    display: none;
  }
}
@media (max-height: 520px) {
  .page-assistant {
    padding: 0 12px 8px;
  }
  .page-assistant-heading {
    min-height: 42px;
  }
  .page-assistant-location {
    display: none;
  }
  .page-assistant-host :deep(.assistant-welcome) {
    display: none;
  }
  .page-assistant-host :deep(.conversation-input) {
    padding-top: 4px;
  }
  .page-assistant-host :deep(.resource-context) {
    max-height: 64px;
  }
}
</style>
