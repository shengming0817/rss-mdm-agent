<script setup lang="ts">
import { computed, provide, useId } from "vue";
import { fieldContext } from "../field";
import type { UiFocusable } from "../context";
const props = defineProps<{
  label: string;
  description?: string;
  error?: string;
  required?: boolean;
  id?: string;
}>();
const id = props.id ?? `ui-field-${useId()}`;
let control: UiFocusable | undefined;
provide(fieldContext, {
  id,
  descriptionId: computed(() =>
    props.description || props.error ? `${id}-description` : undefined,
  ),
  invalid: computed(() => !!props.error),
  register(value) {
    control = value;
    return () => {
      if (control === value) control = undefined;
    };
  },
});
defineExpose({ focus: () => control?.focus() });
</script>
<template>
  <div class="ui-field">
    <label :for="id" :id="`${id}-label`"
      >{{ label }}<span v-if="required" aria-hidden="true"> *</span></label
    >
    <slot :id="id" :labelled-by="`${id}-label`" />
    <p
      v-if="description || error"
      :id="`${id}-description`"
      :role="error ? 'alert' : undefined"
      :class="{ 'ui-tone-danger': error }"
    >
      {{ error || description }}
    </p>
  </div>
</template>
