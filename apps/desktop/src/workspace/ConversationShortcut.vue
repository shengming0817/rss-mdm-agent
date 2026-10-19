<script setup lang="ts">
import { Plus } from "@rss-mdm-agent/ui";
import SettingsGlyph from "../system-settings/SettingsGlyph.vue";
import type { AssistantController } from "../assistant/controller";
const props = defineProps<{
  controller: AssistantController;
  historyOpen: boolean;
}>();
const emit = defineEmits<{ select: []; history: [] }>();
function create() {
  props.controller.create();
  emit("select");
}
</script>
<template>
  <nav
    class="conversation-shortcuts module-navigation"
    aria-label="AI 助手操作"
  >
    <button
      type="button"
      data-action="new-conversation"
      title="新会话"
      aria-label="新会话"
      @click="create"
    >
      <Plus :size="16" aria-hidden="true" /><span>新会话</span>
    </button>
    <button
      type="button"
      data-action="history-conversations"
      title="历史会话"
      aria-label="历史会话"
      :aria-expanded="historyOpen"
      :aria-current="historyOpen ? 'page' : undefined"
      @click="emit('history')"
    >
      <SettingsGlyph kind="history" :size="16" /><span>历史会话</span>
    </button>
  </nav>
</template>
