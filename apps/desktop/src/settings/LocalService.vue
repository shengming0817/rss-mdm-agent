<script setup lang="ts">
import { onMounted, ref } from "vue";
import { nativeService, type ServicePort } from "./native";
import type { ServiceView } from "./service-contract";
const props = defineProps<{ port?: ServicePort; fixture?: boolean }>();
const view = ref<ServiceView>();
const pending = ref(false);
const native = props.fixture ? props.port : (props.port ?? nativeService());
async function refresh() {
  if (!native || pending.value) return;
  pending.value = true;
  try {
    view.value = await native.read();
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
      尚未安装，请由管理员安装匹配的执行服务。
    </p>
    <p v-else-if="view?.phase === 'configurationRequired'">
      服务已登记，但缺少可信配置，请由管理员完成部署。
    </p>
    <p v-else-if="view?.phase === 'mismatch'">
      部署产物或协议不匹配，请显式刷新匹配候选。
    </p>
    <p v-else-if="view?.phase === 'rejected'">
      安装清单或产物校验失败，请由管理员检查安装。
    </p>
    <p v-else-if="view?.phase === 'connected'">
      {{
        view.status.readiness.phase === "ready"
          ? "已连接，执行服务就绪。"
          : view.status.readiness.phase === "registrationRequired"
            ? "已连接，尚未完成可信注册。"
            : "已连接，持久状态尚未就绪；请保留数据并由管理员检查。"
      }}
    </p>
    <p v-else>服务不可用或对端认证未通过，请检查服务状态与访问许可。</p>
    <p>状态查询不代表任务获准或执行成功。</p>
    <button :disabled="!native || pending" @click="refresh">重新查询</button>
  </section>
</template>
