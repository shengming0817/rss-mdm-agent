<script setup lang="ts">
import { ref } from "vue";
import { ListboxRoot, ListboxContent, ListboxItem } from "reka-ui";
import type { UiItem } from "../context";
const props = defineProps<{
  items: readonly UiItem[];
  modelValue: string[];
  multiple?: boolean;
  disabled?: boolean;
  label: string;
}>();
const emit = defineEmits<{
  "update:modelValue": [value: string[]];
  activate: [id: string];
}>();
const highlighted = ref<string>();
function activate(id?: string) {
  if (id && !props.disabled && !props.items.find((i) => i.id === id)?.disabled)
    emit("activate", id);
}
</script>
<template>
  <ListboxRoot
    :model-value="multiple ? modelValue : (modelValue[0] ?? null)"
    :multiple="multiple"
    :disabled="disabled"
    class="ui-list"
    @update:model-value="
      emit(
        'update:modelValue',
        Array.isArray($event) ? $event.map(String) : [String($event)],
      )
    "
    @highlight="highlighted = $event?.value as string | undefined"
    @keydown.enter="!$event.isComposing && activate(highlighted)"
  >
    <ListboxContent :aria-label="label"
      ><ListboxItem
        v-for="item in items"
        :key="item.id"
        :value="item.id"
        :disabled="item.disabled"
        class="ui-option"
        @dblclick="activate(item.id)"
        ><slot name="item" :item="item">{{ item.label }}</slot></ListboxItem
      ></ListboxContent
    >
  </ListboxRoot>
  <p v-if="!items.length" role="status"><slot name="empty">暂无项目</slot></p>
</template>
