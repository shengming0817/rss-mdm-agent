<script setup lang="ts">
import { ref } from "vue";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
withDefaults(defineProps<{ modelValue?: string; multiline?: boolean; type?: "text" | "password"; disabled?: boolean; readonly?: boolean }>(), { modelValue: "", type: "text" });
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const input = ref<HTMLInputElement | HTMLTextAreaElement>();
const focus = () => input.value?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <textarea v-if="multiline" ref="input" v-bind="{ ...field, ...$attrs }" class="ui-input" :value="modelValue" :disabled="disabled" :readonly="readonly" @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)" />
  <input v-else ref="input" v-bind="{ ...field, ...$attrs }" class="ui-input" :type="type" :value="modelValue" :disabled="disabled" :readonly="readonly" @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)" />
</template>
