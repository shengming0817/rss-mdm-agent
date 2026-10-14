<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onErrorCaptured,
  provide,
  shallowRef,
  watch,
  type PropType,
} from "vue";
import { effect, getValue, type ComponentNode } from "@a2ui/web_core/v0_9";
import {
  createOperationController,
  createOperationSurface,
  OperationDrafts,
  type OperationController,
} from "@rss-mdm-agent/ai-ui-bridge";
import type {
  OperationHostPort,
  OperationSnapshot,
} from "@rss-mdm-agent/ai-contract/operations";
import { NodeView, operationContext } from "./NodeView";
const props = defineProps({
  presentation: { type: Object, required: true },
  snapshot: { type: Object as PropType<OperationSnapshot>, required: true },
  host: { type: Object as PropType<OperationHostPort>, required: true },
  drafts: { type: Object as PropType<OperationDrafts>, required: true },
});
const failed = shallowRef(false),
  root = shallowRef<ComponentNode>();
let controller: OperationController | undefined;
let current: ReturnType<typeof createOperationSurface> | undefined;
let stop = () => {};
function mountSurface() {
  stop();
  current?.dispose();
  root.value = undefined;
  if (!controller) {
    failed.value = true;
    return;
  }
  try {
    failed.value = false;
    current = createOperationSurface(
      controller.presentation,
      !!controller.state.fallbackReason,
    );
    current.update(controller.state.draft.values);
    stop = effect(() => {
      root.value = getValue(current!.root);
    });
  } catch {
    if (!controller.state.fallbackReason) {
      controller.fallback();
      mountSurface();
    } else failed.value = true;
  }
}
try {
  controller = createOperationController(
    props.presentation,
    props.snapshot,
    props.host,
    props.drafts,
  );
  provide(operationContext, controller);
  mountSurface();
} catch {
  failed.value = true;
}
watch(
  () => controller?.state.draft.values,
  (draft) => {
    if (draft) current?.update(draft);
  },
  { deep: true },
);
watch(
  () => props.snapshot,
  (snapshot) => {
    try {
      if (controller?.update(snapshot) && failed.value) mountSurface();
    } catch {
      failed.value = true;
      controller?.invalidate();
      stop();
      current?.dispose();
      current = undefined;
      root.value = undefined;
    }
  },
);
onErrorCaptured(() => {
  if (!controller || controller.state.fallbackReason) failed.value = true;
  else {
    controller.fallback();
    mountSurface();
  }
  return false;
});
onBeforeUnmount(() => {
  controller?.dispose();
  stop();
  current?.dispose();
});
async function refresh() {
  await controller?.act("refresh");
  if (controller && !controller.state.unknown) mountSurface();
}
const title = computed(
  () => controller?.state.snapshot.title ?? "操作界面不可用",
);
</script>
<template>
  <article class="operation-card" :aria-label="title">
    <h2>{{ title }}</h2>
    <template v-if="failed">
      <p role="alert">操作界面不可用，请恢复界面或查询原任务。</p>
      <button
        v-if="controller"
        type="button"
        :disabled="!controller.can('refresh')"
        @click="refresh"
      >
        查询原任务
      </button>
    </template>
    <NodeView v-else-if="root" :node="root" />
  </article>
</template>
