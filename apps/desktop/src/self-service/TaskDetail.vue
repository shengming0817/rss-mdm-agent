<script setup lang="ts">
import { reactive } from "vue";
import type { Answer, CatalogItem, FieldInput, RequestView } from "./types";
import ParameterForm from "./ParameterForm.vue";
import PlanSummary from "./PlanSummary.vue";
defineProps<{
  task: RequestView;
  item: CatalogItem | undefined;
  disabled: boolean;
}>();
const emit = defineEmits<{
  respond: [interactionId: string, answer: Answer];
}>();
const fields = reactive(new Map<string, FieldInput>());
function change(key: string, value: FieldInput | null) {
  if (value) fields.set(key, value);
  else fields.delete(key);
}
function parameters(id: string) {
  emit("respond", id, {
    kind: "parameters",
    fields: Object.fromEntries(fields),
  });
  fields.clear();
}
</script>
<template>
  <section class="task-detail" aria-label="任务详情">
    <h2>{{ task.plan.title }}</h2>
    <p class="notice" role="status">{{ task.message }}</p>
    <section
      v-for="interaction in task.interactions"
      :key="interaction.id"
      class="interaction-card"
    >
      <div class="section-heading">
        <h3>交互提示</h3>
        <span class="badge">{{
          interaction.status === "pending"
            ? "等待处理"
            : interaction.status === "answered"
              ? "已回答"
              : interaction.status === "expired"
                ? "已过期"
                : "已取消交互"
        }}</span>
      </div>
      <p>{{ interaction.message }}</p>
      <template v-if="interaction.status === 'pending'">
        <div
          v-if="interaction.kind.kind === 'userConfirmation'"
          class="actions"
        >
          <button
            :disabled="disabled"
            @click="
              emit('respond', interaction.id, {
                kind: 'confirmation',
                accepted: true,
              })
            "
          >
            确认计划
          </button>
          <button
            class="secondary"
            :disabled="disabled"
            @click="
              emit('respond', interaction.id, {
                kind: 'confirmation',
                accepted: false,
              })
            "
          >
            拒绝
          </button>
        </div>
        <div
          v-else-if="interaction.kind.kind === 'privacyConsent'"
          class="actions"
        >
          <button
            :disabled="disabled"
            @click="
              emit('respond', interaction.id, {
                kind: 'privacyConsent',
                accepted: true,
              })
            "
          >
            同意隐私范围
          </button>
          <button
            class="secondary"
            :disabled="disabled"
            @click="
              emit('respond', interaction.id, {
                kind: 'privacyConsent',
                accepted: false,
              })
            "
          >
            不同意
          </button>
        </div>
        <div
          v-else-if="
            interaction.kind.kind === 'restartPrompt' ||
            interaction.kind.kind === 'maintenanceWindow'
          "
          class="actions"
        >
          <button
            v-for="option in interaction.options"
            :key="option.id"
            :disabled="disabled"
            @click="
              emit('respond', interaction.id, {
                kind: 'choice',
                selection: option.id,
              })
            "
          >
            {{ option.label }}
          </button>
        </div>
        <form
          v-else-if="interaction.kind.kind === 'parameterInput' && item"
          novalidate
          @submit.prevent="parameters(interaction.id)"
        >
          <ParameterForm
            :fields="item.fields"
            :values="fields"
            :disabled="disabled"
            :prefix="interaction.id"
            @change="change"
          />
          <button :disabled="disabled">提交参数复核</button>
        </form>
        <p
          v-else-if="interaction.kind.kind === 'administratorAuthorization'"
          class="muted"
        >
          此处没有管理员批准入口。
        </p>
        <button
          class="text-button"
          :disabled="disabled"
          @click="emit('respond', interaction.id, { kind: 'cancel' })"
        >
          取消此交互（不取消任务）
        </button>
      </template>
    </section>
    <PlanSummary :plan="task.plan" />
  </section>
</template>
