<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ServiceView } from "./service-contract";
const view = ref<ServiceView>();
const pending = ref(false);
const native = isTauri();
async function refresh() {
  if (!native || pending.value) return;
  pending.value = true;
  try {
    view.value = await invoke<ServiceView>("local_service_status");
  } catch {
    view.value = { phase: "unavailable" };
  } finally {
    pending.value = false;
  }
}
onMounted(refresh);
</script>
<template>
  <section class="settings-card" aria-label="本机安全服务">
    <h2>本机安全服务</h2>
    <p v-if="!native">请在桌面应用中查询本机服务。</p>
    <p v-else-if="pending">正在验证服务连接…</p>
    <p v-else-if="view?.phase === 'notInstalled'">
      尚未安装，请由管理员安装实验室产物。
    </p>
    <p v-else-if="view?.phase === 'rejected'">
      安装清单或产物校验失败，请由管理员检查安装。
    </p>
    <p v-else-if="view?.phase === 'connected'">已连接，当前仅提供状态查询。</p>
    <p v-else>服务不可用或对端认证未通过，请检查服务状态与访问许可。</p>
    <p>实验室状态查询。连接成功不代表任务获准或执行成功。</p>
    <button :disabled="!native || pending" @click="refresh">重新查询</button>
  </section>
</template>
