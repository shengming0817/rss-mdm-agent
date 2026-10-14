<script setup lang="ts">
import {
  computed,
  markRaw,
  onBeforeUnmount,
  ref,
  shallowRef,
  watch,
  type Component,
} from "vue";
import ExecutionDetails from "../ExecutionDetails.vue";
import type { AssistantController } from "../controller";
import { connectOperation } from "./host";
import type { BackendTaskView } from "@rss-mdm-agent/execution-bindings/task-details";
const props = defineProps<{
  controller: AssistantController;
  operationId: string;
  recorded: boolean;
  visible: boolean;
  now: number;
  detailsOpen: boolean;
  detailsHost?: HTMLElement;
}>();
const emit = defineEmits<{ tasks: []; open: [id: string]; close: [] }>();
const assembly = shallowRef<Awaited<ReturnType<typeof connectOperation>>>();
const renderer = shallowRef<Component>();
const details = shallowRef<BackendTaskView>();
const snapshot =
  shallowRef<Awaited<ReturnType<typeof connectOperation>>["snapshot"]>();
const error = ref(""),
  loading = ref(false);
let owner: AbortController | undefined;
let stop = () => {};
async function loadRenderer() {
  try {
    const module = await import("./OperationCard.vue");
    renderer.value = markRaw(module.default);
  } catch {
    error.value = "操作界面加载失败；仍可查询原任务或进入任务列表。";
  }
}
async function refresh() {
  if (!props.visible || loading.value) return;
  const active = (owner ??= new AbortController());
  loading.value = true;
  try {
    if (!assembly.value) {
      const next = await connectOperation(
        props.controller,
        props.operationId,
        active.signal,
      );
      if (owner !== active || active.signal.aborted) return;
      assembly.value = next;
      snapshot.value = next.snapshot;
      stop = next.host.observe(next.scope, ({ snapshot: value }) => {
        if (!active.signal.aborted) {
          snapshot.value = value;
          details.value = next.current();
        }
      });
    } else snapshot.value = await assembly.value.read();
    details.value = assembly.value.current();
    if (!active.signal.aborted) error.value = "";
    if (!renderer.value && !active.signal.aborted) await loadRenderer();
  } catch {
    if (!active.signal.aborted)
      error.value =
        "无法核实原请求状态，请查询原任务；当前内容不能作为执行依据。";
  } finally {
    if (owner === active) loading.value = false;
  }
}
watch(
  [
    () => props.operationId,
    () => props.controller.state.selected,
    () => props.controller.view.value?.namespace.principalId,
    () => props.controller.view.value?.namespace.authorityId,
    () => props.controller.view.value?.namespace.tenantId,
    () => props.controller.view.value?.namespace.sessionId,
  ],
  () => {
    owner?.abort();
    stop();
    assembly.value = undefined;
    snapshot.value = undefined;
    details.value = undefined;
    loading.value = false;
    owner = undefined;
    error.value = "";
    void refresh();
  },
  { immediate: true },
);
watch(
  () => [props.visible, props.recorded, Math.floor(props.now / 2000)],
  () => {
    void refresh();
  },
);
watch(
  () => props.visible,
  (visible) => {
    if (!visible) {
      owner?.abort();
      stop();
      owner = undefined;
      assembly.value = undefined;
      snapshot.value = undefined;
      details.value = undefined;
      loading.value = false;
    }
  },
);
onBeforeUnmount(() => {
  owner?.abort();
  stop();
});
const key = computed(() =>
  snapshot.value ? JSON.stringify(snapshot.value.scope) : props.operationId,
);
</script>
<template>
  <section v-if="visible" class="execution-activity" aria-label="设备操作">
    <component
      v-if="renderer && assembly && snapshot"
      :is="renderer"
      :key="key"
      :presentation="assembly.presentation"
      :snapshot="snapshot"
      :host="assembly.host"
      :drafts="controller.operationDrafts"
    />
    <p v-else role="status">{{ error || "正在查询可信操作…" }}</p>
    <p v-if="error && renderer" role="alert">{{ error }}</p>
    <div class="assistant-actions">
      <button :disabled="loading" @click="refresh">查询原任务</button>
      <button v-if="!renderer" @click="loadRenderer">恢复操作界面</button>
      <button
        type="button"
        data-action="execution-details"
        :data-operation-id="operationId"
        @click="emit('open', operationId)"
      >
        查看设备操作
      </button>
      <button @click="emit('tasks')">前往任务处理</button>
    </div>
    <Teleport :to="detailsHost ?? 'body'" :disabled="!detailsHost">
      <section
        v-if="visible && detailsOpen"
        class="execution-inspector-content"
      >
        <ExecutionDetails
          v-if="details?.kind === 'execution'"
          :details="details.value"
          :now="now"
        />
        <p v-else>原请求仍在准备阶段，目标和最终参数见操作卡片。</p>
      </section>
    </Teleport>
  </section>
</template>
