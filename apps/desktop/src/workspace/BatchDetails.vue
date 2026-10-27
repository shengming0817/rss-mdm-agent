<script setup lang="ts">
import { onUnmounted, ref } from "vue";
import CopyEntry from "./CopyEntry.vue";
import type { ManagementActivityPage } from "../self-service/types";
import type { NavigationPort } from "./entry";
const props = defineProps<{
  batch: string;
  initial: ManagementActivityPage;
  port: NavigationPort;
}>();
const emit = defineEmits<{ close: []; open: [id: string] }>();
const page = ref(props.initial),
  busy = ref(false),
  error = ref("");
let alive = true;
onUnmounted(() => {
  alive = false;
});
async function more() {
  if (!page.value.nextCursor || busy.value) return;
  busy.value = true;
  try {
    const result = await props.port({
      kind: "activities",
      batch: props.batch,
      related: null,
      after: page.value.nextCursor,
      limit: 64,
    });
    if (!alive) return;
    if (result.kind !== "activities") throw Error("unavailable");
    page.value = result.value;
  } catch {
    if (alive) error.value = "批次页未确认，请重试当前页";
  } finally {
    if (alive) busy.value = false;
  }
}
</script>
<template>
  <section aria-label="批次详情">
    <header>
      <h2 tabindex="-1">批次 {{ batch }}</h2>
      <button @click="emit('close')">返回任务中心</button>
    </header>
    <CopyEntry :selector="{ kind: 'batch', id: batch }" />
    <p>
      截至
      {{
        new Date(page.asOf * 1000).toLocaleString()
      }}，以下为当前授权范围内的原始活动事实。
    </p>
    <p v-for="source in page.coverage" :key="source.source">
      {{ source.source }}：{{ source.complete ? "来源完整" : "部分覆盖" }}
      {{ source.reasons.join(" · ") }}
    </p>
    <p v-if="error" role="alert">{{ error }}</p>
    <button
      v-for="item in page.items"
      :key="item.id"
      @click="emit('open', item.id)"
    >
      {{ item.phase }} · {{ item.decision ?? "尚无判定" }} ·
      {{ item.links.member ?? item.id }}
    </button>
    <button v-if="page.nextCursor" :disabled="busy" @click="more">
      读取下一页
    </button>
  </section>
</template>
