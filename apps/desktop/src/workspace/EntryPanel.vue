<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import CopyEntry from "./CopyEntry.vue";
import type {
  BackendRequest,
  Delivery,
  SelfServiceDirectoryItem,
  SelfServicePort,
} from "../self-service/types";
import type { NavigationPort } from "./entry";
const props = defineProps<{
  delivery: Delivery;
  policy?: SelfServiceDirectoryItem;
  port: SelfServicePort | null;
  navigation: NavigationPort;
}>();
const emit = defineEmits<{ close: []; task: [request: string] }>();
const dialog = ref<HTMLDialogElement>();
const returnFocus = document.activeElement as HTMLElement | null;
onUnmounted(() => {
  returnFocus?.focus();
});
const form = ref<InstanceType<typeof PolicySelfService>>();
const original = ref<BackendRequest>();
const error = ref("");
const busy = ref(false);
const request = ref("");
const intent = computed(() => ({
  ...props.delivery.intent,
  version: props.policy?.policyVersion ?? props.delivery.intent.version,
  revision: props.policy?.policyRevision ?? props.delivery.intent.revision,
}));
const port = computed<SelfServicePort | null>(() =>
  props.port
    ? {
        ...props.port,
        async policy_invoke(input) {
          const result = await props.navigation({
            kind: "submit",
            intent: {
              ...intent.value,
              action: "invoke",
              parameters: input.choice.parameters,
            },
            request: input.choice.request,
          });
          if (result.kind !== "request") throw Error("unknown");
          original.value = result.value;
          return result.value;
        },
      }
    : null,
);
onMounted(async () => {
  await nextTick();
  dialog.value?.showModal();
  dialog.value?.querySelector<HTMLElement>("h2")?.focus();
  if (
    props.policy &&
    !(await form.value?.openPolicy(
      props.policy.policyId,
      props.delivery.intent.parameters,
      props.policy,
    ))
  )
    error.value = "目标版本已变化，请重新打开当前目标";
});
async function submitEvent() {
  if (busy.value || original.value) return;
  busy.value = true;
  request.value ||= crypto.randomUUID();
  try {
    const result = await props.navigation({
      kind: "submit",
      intent: intent.value,
      request: request.value,
    });
    if (result.kind !== "request") throw Error("unknown");
    original.value = result.value;
  } catch {
    error.value = "回执未确认，保留原请求；再次查询使用同一请求 ID。";
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <dialog
    ref="dialog"
    @cancel.prevent="emit('close')"
    class="external-entry"
    aria-labelledby="external-entry-title"
  >
    <header>
      <h2 id="external-entry-title" tabindex="-1">
        {{ delivery.intent.action === "invoke" ? "核对调用目标" : "目标详情" }}
      </h2>
      <button type="button" @click="emit('close')">返回原页面</button>
    </header>
    <CopyEntry :selector="delivery.intent.selector" />
    <p v-if="error" role="alert">{{ error }}</p>
    <PolicySelfService
      v-if="policy"
      ref="form"
      :port="port"
      :records="original ? [original] : []"
      :allow-new-requests="true"
    />
    <template v-else-if="delivery.intent.selector.kind === 'event'">
      <p>自定义事件：{{ delivery.intent.selector.name }}</p>
      <p>适用策略及逐项受理结果由原业务服务决定，提交后在任务中心查询。</p>
      <button
        v-if="delivery.intent.action === 'invoke'"
        :disabled="busy || !!original"
        @click="submitEvent"
      >
        确认提交事件
      </button>
    </template>
    <CopyEntry
      v-if="
        original?.action.kind === 'event' &&
        (original.action.receipt?.batchId || original.action.batch)
      "
      :selector="{
        kind: 'batch',
        id: original.action.receipt?.batchId ?? original.action.batch!,
      }"
    />
    <p v-if="request">原请求：{{ request }}</p>
    <button
      v-if="original"
      @click="
        emit(
          'task',
          original.action.kind === 'event'
            ? original.action.request
            : original.action.kind === 'policy'
              ? original.action.choice.request
              : '',
        )
      "
    >
      查看原请求
    </button>
  </dialog>
</template>
<style scoped>
.external-entry {
  position: fixed;
  inset: 10% 10% auto;
  width: min(900px, 80vw);
  max-height: 80vh;
  overflow: auto;
  z-index: 100;
  background: var(--ui-surface, white);
  color: inherit;
  border: 1px solid currentColor;
  border-radius: 12px;
  padding: 24px;
}
.external-entry header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
</style>
