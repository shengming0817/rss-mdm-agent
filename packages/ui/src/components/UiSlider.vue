<script setup lang="ts">
import { ref } from "vue";
import { SliderRoot, SliderTrack, SliderRange, SliderThumb } from "reka-ui";
import { useFieldControl } from "../field";
defineOptions({ inheritAttrs: false });
const props = withDefaults(defineProps<{ modelValue: number; min?: number; max?: number; step?: number; disabled?: boolean; readonly?: boolean }>(), { min: 0, max: 100, step: 1 });
const emit = defineEmits<{ "update:modelValue": [value: number] }>();
const thumb = ref<InstanceType<typeof SliderThumb>>();
const focus = () => (thumb.value?.$el as HTMLElement)?.focus();
const field = useFieldControl(focus);
defineExpose({ focus });
</script>
<template>
  <SliderRoot class="ui-slider" :model-value="[modelValue]" :min="min" :max="max" :step="step" :disabled="disabled || readonly" @update:model-value="!props.readonly && emit('update:modelValue', $event![0])">
    <SliderTrack class="ui-slider-track"><SliderRange class="ui-slider-range" /></SliderTrack><SliderThumb ref="thumb" class="ui-slider-thumb" v-bind="{ ...field, ...$attrs }" />
  </SliderRoot>
</template>
