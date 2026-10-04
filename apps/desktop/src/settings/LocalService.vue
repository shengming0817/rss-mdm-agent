<script setup lang="ts">
import { onMounted, ref } from "vue";
import { nativeService, type ServicePort } from "./native";
import type { ServiceView, RegistrationStatus } from "./service-contract";
const props = defineProps<{ port?: ServicePort; fixture?: boolean }>();
const view = ref<ServiceView>();
const pending = ref(false);
const registration = ref<RegistrationStatus>();
const registrationError = ref(false);
const native = props.fixture ? props.port : (props.port ?? nativeService());
async function refresh() {
  if (!native || pending.value) return;
  pending.value = true;
  try {
    view.value = await native.read();
    if (view.value.phase === "connected" && native.registration) {
      try {
        registration.value = await native.registration();
        registrationError.value = false;
      } catch {
        registrationError.value = true;
      }
    }
  } catch {
    view.value = { phase: "unavailable" };
  } finally {
    pending.value = false;
  }
}
async function requestRegistration() {
  if (!native?.requestRegistration || pending.value) return;
  pending.value = true;
  try {
    registration.value = await native.requestRegistration();
    registrationError.value = false;
  } catch {
    registrationError.value = true;
  } finally {
    pending.value = false;
  }
}
onMounted(refresh);
</script>
<template>
  <section class="settings-card" aria-label="本机执行服务">
    <h2>本机执行服务</h2>
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
    <div v-if="registration">
      <p v-if="registration.credential === 'expired'">
        设备证书已过期，请由管理员为原注册恢复证书。
      </p>
      <p v-else-if="registration.credential === 'revoked'">
        设备证书已撤销，请由管理员检查原注册。
      </p>
      <p v-else-if="registration.credential === 'retired'">
        设备证书已退休，请恢复原换证操作。
      </p>
      <p v-else-if="registration.phase === 'active'">本机设备身份已激活。</p>
      <p v-else-if="registration.phase === 'unregistered'">
        请发起注册，再在组织网页批准这台设备。
      </p>
      <p
        v-else-if="
          registration.phase === 'issuanceUnknown' ||
          registration.phase === 'activationUnknown'
        "
      >
        注册结果待核实，请保留本机状态并由管理员恢复原请求。
      </p>
      <p v-else-if="registration.phase === 'rejected'">
        原注册请求或证书不可用，请由管理员检查。
      </p>
      <p v-else>正在处理原注册请求，请在组织网页核对并批准。</p>
      <p v-if="registration.grantId">请求编号：{{ registration.grantId }}</p>
      <p v-if="registration.publicKeyFingerprint">
        设备公钥指纹：{{ registration.publicKeyFingerprint }}
      </p>
      <p v-if="registration.expiresAt">
        授权截止：{{ new Date(registration.expiresAt * 1000).toLocaleString() }}
      </p>
    </div>
    <p v-if="registrationError">无法读取或发起注册，请检查桌面访问许可。</p>
    <button
      v-if="
        registration?.phase === 'unregistered' && native?.requestRegistration
      "
      :disabled="pending"
      @click="requestRegistration"
    >
      发起设备注册
    </button>
    <p>状态查询不代表任务获准或执行成功。</p>
    <button :disabled="!native || pending" @click="refresh">重新查询</button>
  </section>
</template>
