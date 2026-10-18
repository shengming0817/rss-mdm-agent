<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { currentUser } from "../test-users";
import { nativePort } from "../self-service/native";
import PolicySelfService from "../self-service/PolicySelfService.vue";
import type { BackendRequest, BackendTask } from "../self-service/types";
import type { MdmStatus } from "./service-contract";
import { readMdmStatus } from "./native";
const status = ref<MdmStatus>(),
  error = ref(""),
  busy = ref(false);
const records = ref<BackendRequest[]>([]),
  available = ref<BackendTask[]>([]);
const port = ref(nativePort());
let timer: ReturnType<typeof setInterval> | undefined;
const text = computed(
  () =>
    ({
      unenrolled: "本机尚未注册 MDM。",
      this_organization: "本机检测到本组织的 MDM 注册。",
      other_organization: "本机已由其他组织管理，不会自动替换。",
      unknown: "无法确定本机 MDM 注册状态。",
    })[status.value?.state ?? "unknown"],
);
async function refresh() {
  if (busy.value) return;
  busy.value = true;
  try {
    status.value = await readMdmStatus();
    if (port.value) {
      const snapshot = await port.value.snapshot({
        after: null,
        selected: null,
      });
      records.value = snapshot.preparations;
      available.value = snapshot.available.filter(
        (t) => t.summary.kind === "enrollment",
      );
    }
    error.value = "";
  } catch {
    error.value = "无法读取本机服务，请刷新重试。";
    status.value = undefined;
    records.value = [];
    available.value = [];
  } finally {
    busy.value = false;
  }
}
watch(
  () => currentUser.value?.generation,
  () => {
    port.value = nativePort();
    records.value = [];
    void refresh();
  },
);
onMounted(() => {
  void refresh();
  timer = setInterval(refresh, 5000);
});
onUnmounted(() => clearInterval(timer));
</script>
<template>
  <section class="settings-card" aria-label="MDM 注册">
    <h2>MDM 注册</h2>
    <p>{{ text }}</p>
    <p v-if="error" role="status">{{ error }}</p>
    <button :disabled="busy" @click="refresh">刷新注册状态</button>
    <p v-if="available.length">
      组织已下发 MDM 注册任务，等待当前登录会话完成系统操作。
    </p>
    <PolicySelfService
      v-if="status"
      :port="port"
      :records="records"
      kind="enrollment"
      :allow-new-requests="status.state === 'unenrolled'"
      @changed="refresh"
    />
  </section>
</template>
