<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";
import { nativeNavigation } from "./entry";
import type { ManagementActivity } from "../self-service/types";
const props = defineProps<{ activity: string; after: string }>();
const emit = defineEmits<{ open: [id: string] }>();
const port = nativeNavigation();
const next = ref(props.after),
  items = ref<ManagementActivity[]>([]),
  busy = ref(false),
  error = ref("");
let epoch = 0;
onUnmounted(() => {
  epoch++;
});
watch(
  () => [props.activity, props.after],
  () => {
    epoch++;
    next.value = props.after;
    items.value = [];
    busy.value = false;
    error.value = "";
  },
);
async function load() {
  if (!port || busy.value || !next.value) return;
  const call = epoch;
  busy.value = true;
  try {
    const result = await port({
      kind: "activities",
      batch: null,
      related: props.activity,
      after: next.value,
      limit: 50,
    });
    if (epoch !== call) return;
    if (result.kind !== "activities") throw Error("unavailable");
    items.value = result.value.items;
    next.value = result.value.nextCursor ?? "";
  } catch {
    if (epoch === call) error.value = "关联页未确认，请重试当前页";
  } finally {
    if (epoch === call) busy.value = false;
  }
}
</script>
<template>
  <section aria-label="更多关联活动">
    <p v-if="error" role="alert">{{ error }}</p>
    <button v-for="item in items" :key="item.id" @click="emit('open', item.id)">
      {{ item.kind }} · {{ item.phase }} · {{ item.id }}
    </button>
    <button v-if="next" :disabled="busy || !port" @click="load">
      读取下一页关联
    </button>
  </section>
</template>
