<script setup lang="ts">
import { computed } from "vue";
import type { FieldInput, Parameter } from "./types";
const props = defineProps<{
  fields: Record<string, Parameter>;
  values: Map<string, FieldInput>;
  disabled: boolean;
  prefix: string;
}>();
const emit = defineEmits<{ change: [key: string, value: FieldInput | null] }>();
const entries = computed(() => Object.entries(props.fields));
function value(key: string): string {
  const input = props.values.get(key);
  return input && "value" in input ? String(input.value) : "";
}
function text(key: string, type: "text" | "integer", event: Event) {
  const input = (event.target as HTMLInputElement).value;
  emit("change", key, input === "" ? null : { kind: type, value: input });
}
function boolean(key: string, event: Event) {
  const input = (event.target as HTMLSelectElement).value;
  emit(
    "change",
    key,
    input === "" ? null : { kind: "boolean", value: input === "true" },
  );
}
function secret(key: string, coordinate: "id" | "revision", event: Event) {
  const previous = props.values.get(key);
  const input =
    previous?.kind === "secretReference"
      ? { ...previous }
      : { kind: "secretReference" as const, id: "", revision: "" };
  if (coordinate === "id") input.id = (event.target as HTMLInputElement).value;
  else input.revision = (event.target as HTMLInputElement).value;
  emit("change", key, input.id || input.revision ? input : null);
}
function secretValue(key: string, coordinate: "id" | "revision") {
  const input = props.values.get(key);
  return input?.kind === "secretReference"
    ? coordinate === "id"
      ? input.id
      : input.revision
    : "";
}
function defaultLabel(field: Parameter): string {
  return "default" in field.rule &&
    field.rule.default !== null &&
    field.rule.default !== undefined
    ? `目录默认值：${String(field.rule.default)}`
    : "未填写";
}
</script>
<template>
  <div class="parameter-fields">
    <p v-if="entries.length === 0" class="muted">此操作无需参数。</p>
    <div v-for="[key, field] in entries" :key="key" class="field">
      <label :for="`${prefix}-${key}`"
        >{{ field.title }}
        <span v-if="field.required" class="required">必填</span
        ><span v-else class="muted">可选</span></label
      >
      <p class="field-help">{{ field.description }}</p>
      <template v-if="field.rule.type === 'secretReference'">
        <input
          :id="`${prefix}-${key}`"
          :name="key"
          type="password"
          autocomplete="off"
          placeholder="引用 ID"
          :disabled="disabled"
          :value="secretValue(key, 'id')"
          @input="secret(key, 'id', $event)"
        />
        <label :for="`${prefix}-${key}-revision`" class="sub-label"
          >引用版本</label
        >
        <input
          :id="`${prefix}-${key}-revision`"
          type="password"
          autocomplete="off"
          placeholder="精确版本"
          :disabled="disabled"
          :value="secretValue(key, 'revision')"
          @input="secret(key, 'revision', $event)"
        />
      </template>
      <select
        v-else-if="field.rule.type === 'boolean'"
        :id="`${prefix}-${key}`"
        :disabled="disabled"
        :value="value(key)"
        @change="boolean(key, $event)"
      >
        <option value="">{{ defaultLabel(field) }}</option>
        <option value="true">是</option>
        <option value="false">否</option>
      </select>
      <select
        v-else-if="field.rule.choices"
        :id="`${prefix}-${key}`"
        :disabled="disabled"
        :value="value(key)"
        @change="
          text(key, field.rule.type === 'integer' ? 'integer' : 'text', $event)
        "
      >
        <option value="">{{ defaultLabel(field) }}</option>
        <option
          v-for="choice in field.rule.choices"
          :key="choice"
          :value="String(choice)"
        >
          {{ choice }}
        </option>
      </select>
      <input
        v-else
        :id="`${prefix}-${key}`"
        :name="key"
        type="text"
        :inputmode="field.rule.type === 'integer' ? 'decimal' : 'text'"
        :placeholder="defaultLabel(field)"
        :disabled="disabled"
        :value="value(key)"
        @input="
          text(key, field.rule.type === 'integer' ? 'integer' : 'text', $event)
        "
      />
    </div>
  </div>
</template>
