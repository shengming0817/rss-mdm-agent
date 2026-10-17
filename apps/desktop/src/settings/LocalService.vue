<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import Account from "./Account.vue";
import { nativeService, type ServicePort } from "./native";
import type { ServiceView, RegistrationStatus } from "./service-contract";
const props = defineProps<{ port?: ServicePort; fixture?: boolean }>();
const view = ref<ServiceView>();
const pending = ref(false);
const registering = ref(false);
const registration = ref<RegistrationStatus>();
const registrationError = ref("");
const showLogin = ref(false);
let timer: ReturnType<typeof setInterval> | undefined;
function failure(error: unknown) {
  const value = error as { code?: string; message?: string };
  if (
    value?.code === "registration_login_required" ||
    value?.code === "registration_authentication_required" ||
    value?.code === "account/session/denied" ||
    value?.code === "account/login/denied"
  )
    showLogin.value = true;
  registrationError.value =
    value?.message ?? "注册操作未确认，请查询原请求后重试。";
}
const native = props.fixture ? props.port : (props.port ?? nativeService());
async function refresh() {
  if (!native || pending.value) return;
  pending.value = true;
  try {
    view.value = await native.read();
    if (view.value.phase === "connected") {
      try {
        registration.value = await native.registration();
        if (
          registration.value.registered ||
          registrationError.value === "无法读取本机注册状态，请检查服务连接。"
        )
          registrationError.value = "";
      } catch {
        registrationError.value = "无法读取本机注册状态，请检查服务连接。";
      }
    }
  } catch {
    view.value = { phase: "unavailable" };
  } finally {
    pending.value = false;
  }
}
async function requestRegistration(organizationId?: string, login?: string) {
  if (!native || pending.value) return;
  pending.value = true;
  registering.value = true;
  try {
    registration.value = await native.requestRegistration(
      organizationId,
      login,
    );
    registrationError.value = "";
    showLogin.value = false;
  } catch (error) {
    failure(error);
  } finally {
    pending.value = false;
    registering.value = false;
  }
}
async function importRegistration() {
  if (!native || pending.value) return;
  pending.value = true;
  try {
    const value = await native.importRegistration();
    if (value) {
      registration.value = value;
      registrationError.value = "";
      showLogin.value = false;
    }
  } catch (error) {
    failure(error);
  } finally {
    pending.value = false;
  }
}
onMounted(() => {
  void refresh();
  timer = setInterval(() => {
    if (registration.value && !registration.value.registered) void refresh();
  }, 2000);
});
onUnmounted(() => {
  clearInterval(timer);
  if (registering.value) void native?.cancelRegistration();
});
</script>
<template>
  <section class="settings-card" aria-label="本机执行服务">
    <h2>本机执行服务</h2>
    <p v-if="!native">请在桌面应用中查询本机服务。</p>
    <p v-else-if="pending">
      {{
        registering
          ? "正在继续原注册请求，网络恢复后将自动授权…"
          : "正在验证服务连接…"
      }}
    </p>
    <button v-if="registering" @click="native?.cancelRegistration()">
      停止注册等待
    </button>
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
      <p v-if="registration.failure" role="alert">
        {{
          {
            invalidConfiguration:
              "配置签名、组织或候选不匹配，请使用有效配置。",
            conflict:
              "配置或原注册操作存在冲突，请核实原操作，不能覆盖机器身份。",
            denied: "服务端拒绝注册，请检查资格或配置撤销状态。",
            expired: "注册授权已过期，请核实原操作。",
            capacity: "注册名额不足，请联系管理员。",
            unavailable: "操作未确认，请保留原请求等待恢复。",
          }[registration.failure]
        }}
      </p>
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
        登录安装包所属组织以注册本机，或导入管理员提供的完整配置。
      </p>
      <p
        v-else-if="
          registration.phase === 'configurationUnknown' ||
          registration.phase === 'issuanceUnknown' ||
          registration.phase === 'activationUnknown'
        "
      >
        注册结果待核实，请保留本机状态并由管理员恢复原请求。
      </p>
      <p v-else-if="registration.phase === 'paused'">
        组织已暂停新注册。请保留原请求和密钥；恢复后在原有效期内继续。
      </p>
      <p v-else-if="registration.phase === 'rejected'">
        原注册请求或证书不可用，请由管理员检查。
      </p>
      <p v-else>正在处理原注册请求，授权后将自动签发并激活设备身份。</p>
      <p v-if="registration.grantId">请求编号：{{ registration.grantId }}</p>
      <p v-if="registration.publicKeyFingerprint">
        设备公钥指纹：{{ registration.publicKeyFingerprint }}
      </p>
      <p v-if="registration.expiresAt">
        授权截止：{{ new Date(registration.expiresAt * 1000).toLocaleString() }}
      </p>
    </div>
    <p v-if="registrationError" role="alert">{{ registrationError }}</p>
    <Account
      v-if="showLogin"
      registration
      :loading="pending"
      @login="requestRegistration"
    />
    <button
      v-if="registration && !registration.registered && native"
      :disabled="pending"
      @click="requestRegistration()"
    >
      登录并注册此设备
    </button>
    <button
      v-if="registration && !registration.registered && native"
      :disabled="pending"
      @click="importRegistration"
    >
      导入注册配置
    </button>
    <p>状态查询不代表任务获准或执行成功。</p>
    <button :disabled="!native || pending" @click="refresh">重新查询</button>
  </section>
</template>
