<script setup lang="ts">
import { shallowRef } from "vue";
import type { OperationDrafts } from "../../packages/ai-ui-bridge/dist/index.js";
import OperationCard from "../../apps/desktop/src/assistant/operation-ui/OperationCard.vue";
import { operationFixture, operationTemplates } from "./operation-fixtures";
import type { OperationPresentation } from "../../packages/ai-contract/dist/operations.js";
defineProps<{ drafts: OperationDrafts }>();
const selected = shallowRef<OperationPresentation["template"]>("SettingsForm");
const fixture = shallowRef(operationFixture());
function select(event: Event) {
  selected.value = (event.target as HTMLSelectElement)
    .value as OperationPresentation["template"];
  fixture.value = operationFixture(selected.value);
}
</script>
<template>
  <section aria-label="操作模板 fixture">
    <label
      >操作模板
      <select :value="selected" aria-label="操作模板" @change="select">
        <option v-for="template in operationTemplates" :key="template">
          {{ template }}
        </option>
      </select></label
    >
    <OperationCard
      :key="selected"
      :presentation="fixture.presentation"
      :snapshot="fixture.snapshot"
      :host="fixture.host"
      :drafts="drafts"
    />
  </section>
</template>
