<script setup lang="ts">
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "reka-ui";
import type { UiItem } from "../context";
defineProps<{
  modelValue: string;
  items: readonly UiItem[];
  label: string;
  segmented?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
</script>
<template>
  <TabsRoot
    :model-value="modelValue"
    class="ui-tabs"
    :class="{ 'ui-segmented': segmented }"
    @update:model-value="emit('update:modelValue', String($event))"
    ><TabsList :aria-label="label" class="ui-tablist"
      ><TabsTrigger
        v-for="item in items"
        :key="item.id"
        :value="item.id"
        :disabled="item.disabled"
        >{{ item.label }}</TabsTrigger
      ></TabsList
    >
    <div v-for="item in items" :key="item.id" :hidden="modelValue !== item.id">
      <TabsContent :value="item.id" :force-mount="true" class="ui-tabpanel"
        ><slot :item="item"
      /></TabsContent></div
  ></TabsRoot>
</template>
