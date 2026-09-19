<script setup lang="ts">
import { computed, reactive } from "vue";
import { questions } from "@rss-mdm-agent/ai-ui-bridge";
import type { InteractionView } from "@rss-mdm-agent/ai-client";
const props = defineProps<{ interaction: InteractionView; enabled: boolean }>();
const emit = defineEmits<{ answer: [answers: Record<string, string>] }>();
const rows = computed(() => questions(props.interaction.request));
const answers = reactive(new Map<string, string>());
const choices = reactive(new Map<string, string[]>());
function choose(question: string, value: string, multi: boolean) {
  const previous = choices.get(question) ?? [];
  const selected = multi
    ? previous.includes(value)
      ? previous.filter((v) => v !== value)
      : [...previous, value]
    : [value];
  choices.set(question, selected);
  answers.set(question, selected.join(", "));
}
function input(question: string, event: Event) {
  answers.set(question, (event.target as HTMLTextAreaElement).value);
}
const complete = computed(() =>
  rows.value?.every((row) => answers.get(row.question)?.trim()),
);
</script>
<template>
  <section class="question-card">
    <h3>AI 提问</h3>
    <p>回答仅用于继续对话；设备执行授权由执行服务独立判断。</p>
    <form
      v-if="rows"
      @submit.prevent="
        enabled && complete && emit('answer', Object.fromEntries(answers))
      "
    >
      <fieldset v-for="row in rows" :key="row.question" :disabled="!enabled">
        <legend>{{ row.header }} · {{ row.question }}</legend>
        <div class="question-options">
          <button
            v-for="option in row.options"
            :key="option.label"
            type="button"
            :aria-pressed="
              choices.get(row.question)?.includes(option.label) ?? false
            "
            @click="choose(row.question, option.label, row.multiSelect)"
          >
            {{ option.label }}<small>{{ option.description }}</small>
          </button>
        </div>
        <label
          >补充或自行回答<textarea
            :value="answers.get(row.question) ?? ''"
            rows="2"
            @input="input(row.question, $event)"
          />
        </label>
      </fieldset>
      <button type="submit" :disabled="!enabled || !complete">提交回答</button>
    </form>
    <p v-else>暂不支持此提问格式。请联系服务提供方。</p>
    <p v-if="!enabled" role="status">
      {{
        interaction.status === "answered"
          ? "已回答"
          : interaction.status === "expired"
            ? "已过期"
            : interaction.status === "unavailable"
              ? "此提问已失效"
              : "当前不可回答：请检查连接与有效期"
      }}
    </p>
  </section>
</template>
