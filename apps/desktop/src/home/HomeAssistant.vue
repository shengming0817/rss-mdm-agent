<script setup lang="ts">
import { useId } from "vue";
import { Sparkles, Monitor, ChevronRight } from "@rss-mdm-agent/ui";
withDefaults(
  defineProps<{ draft: string; connected: boolean; context?: string }>(),
  { context: "我的电脑" },
);
const emit = defineEmits<{
  "update:draft": [text: string];
  navigate: [page: string];
}>();
const questionId = useId();
</script>
<template>
  <div class="home-assistant" role="complementary" aria-label="电脑助手入口">
    <div class="assistant-title">
      <span><Sparkles :size="21" aria-hidden="true" /></span
      ><strong>电脑助手</strong>
    </div>
    <p class="assistant-subtitle">一起处理工作中的小问题</p>
    <div class="assistant-context">
      <Monitor :size="14" aria-hidden="true" /><span>{{ context }}</span
      ><small>{{ connected ? "AI 已连接" : "AI 未连接" }}</small>
    </div>
    <div class="home-welcome">
      <h3>有什么需要帮忙的？</h3>
      <p>描述你遇到的问题，一起找到下一步。</p>
    </div>
    <div class="home-prompts">
      <button
        v-for="prompt in [
          '应用连不上，应该怎么排查？',
          '怎样查看任务的处理进度？',
          '帮我了解可用的软件和工具',
        ]"
        :key="prompt"
        :disabled="!!draft"
        type="button"
        @click="emit('update:draft', prompt)"
      >
        {{ prompt }}<ChevronRight :size="13" aria-hidden="true" />
      </button>
    </div>
    <form class="home-composer" @submit.prevent="emit('navigate', 'assistant')">
      <label class="sr-only" :for="questionId">向电脑助手提问</label
      ><textarea
        :id="questionId"
        :value="draft"
        rows="3"
        placeholder="描述你想做的事情…"
        @input="
          emit('update:draft', ($event.target as HTMLTextAreaElement).value)
        "
      />
      <div>
        <span>在 AI 助手中继续</span
        ><button type="submit" aria-label="前往 AI 助手">↗</button>
      </div>
    </form>
    <p class="assistant-footnote">保留当前草稿，发送前可再次编辑</p>
  </div>
</template>
<style scoped>
.home-assistant {
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--rss-color-border);
  padding: 24px 18px 16px;
  min-width: 0;
}
.assistant-title {
  justify-content: flex-start;
  font-size: 13px;
}
.assistant-title > span {
  display: flex;
  padding: 7px;
  border-radius: 9px;
  background: #a58bd218;
  color: #9a88ba;
}
.assistant-subtitle {
  font-size: 10px;
  color: var(--rss-color-text-muted);
  margin: 10px 0 24px;
}
.assistant-context {
  font-size: 10px;
  padding: 12px 10px;
  background: color-mix(in srgb, var(--rss-color-bg) 94%, #b7b9d9);
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  color: var(--rss-color-text-muted);
  gap: 6px;
}
.assistant-context small {
  margin-left: auto;
  font-size: 9px;
}
.home-welcome {
  margin: 22px 0 18px;
}
.home-welcome h3 {
  font-size: 12px;
  margin: 0 0 7px;
}
.home-welcome p {
  font-size: 12px;
  line-height: 1.9;
  color: var(--rss-color-text-muted);
  margin: 0;
}
.home-prompts {
  display: grid;
  gap: 9px;
}
.home-prompts button {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  padding: 12px 10px;
  color: var(--rss-color-text-muted);
  background: var(--rss-color-bg);
  font-size: 10px;
  text-align: left;
}
.home-prompts svg {
  flex: none;
}
.home-prompts button:disabled {
  opacity: 0.5;
  cursor: default;
}
.home-composer {
  margin-top: auto;
  padding: 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--rss-color-bg) 97%, #8bb2db);
}
.home-prompts {
  margin-bottom: 30px;
}
.home-composer textarea {
  resize: vertical;
  min-height: 55px;
  max-height: 160px;
  width: 100%;
  border: 0;
  outline: 0;
  box-sizing: border-box;
  background: transparent;
  color: var(--rss-color-text);
  font: inherit;
  font-size: 11px;
  line-height: 1.7;
}
.home-composer > div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--rss-color-text-muted);
  font-size: 9px;
}
.home-composer button {
  border: 0;
  border-radius: 5px;
  color: #627fa1;
  background: #8baedb18;
  font-size: 17px;
}
.assistant-footnote {
  font-size: 9px;
  text-align: center;
  color: var(--rss-color-text-muted);
  margin: 10px 0 0;
}

.assistant-title,
.assistant-context {
  display: flex;
  align-items: center;
  gap: 9px;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}
button {
  cursor: pointer;
  font-family: inherit;
}
button:focus-visible,
textarea:focus-visible {
  outline: 2px solid #6d95c8;
  outline-offset: 3px;
}
@media (max-width: 1250px) {
  .home-assistant {
    padding: 22px 14px 16px;
  }
}
@media (max-width: 1050px) {
  .home-assistant {
    border-left: 0;
    border-top: 1px solid var(--rss-color-border);
  }
  .home-prompts {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .home-composer {
    margin-top: 20px;
  }
}
@media (max-width: 500px) {
  .home-prompts {
    grid-template-columns: 1fr;
  }
}
</style>
