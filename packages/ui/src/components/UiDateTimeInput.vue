<script setup lang="ts">
import { ref, watch } from "vue";
import UiDateInput from "./UiDateInput.vue";
import UiTimeInput from "./UiTimeInput.vue";
import UiField from "./UiField.vue";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue?: string;
  disabled?: boolean;
  readonly?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const date = ref(""),
  time = ref("");
watch(
  () => props.modelValue,
  (value) => {
    const parts = (value ?? "").split("T");
    date.value = parts.at(0) ?? "";
    time.value = parts.at(1) ?? "";
  },
  { immediate: true },
);
const dateControl = ref<InstanceType<typeof UiDateInput>>();
const focus = () => dateControl.value?.focus();
const field = useFieldControl(focus);
function update() {
  emit(
    "update:modelValue",
    date.value && time.value ? `${date.value}T${time.value}` : "",
  );
}
defineExpose({ focus });
</script>
<template>
  <div
    class="ui-date-time"
    role="group"
    :aria-labelledby="field.id ? `${field.id}-label` : undefined"
    v-bind="{ ...field, ...$attrs }"
  >
    <UiField label="日期"
      ><UiDateInput
        ref="dateControl"
        v-model="date"
        :disabled="disabled"
        :readonly="readonly"
        @update:model-value="update" /></UiField
    ><UiField label="时间"
      ><UiTimeInput
        v-model="time"
        :disabled="disabled"
        :readonly="readonly"
        @update:model-value="update"
    /></UiField>
  </div>
</template>
