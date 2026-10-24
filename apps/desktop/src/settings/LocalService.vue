<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import Account from "./Account.vue";
import { accountBinding, recoverAccount } from "../test-users";
import DeviceAccess from "./DeviceAccess.vue";
import MdmRegistration from "./MdmRegistration.vue";
import { nativeService, type ServicePort } from "./native";
import type { SelfServicePort } from "../self-service/types";
import type { ServiceView, RegistrationStatus } from "./service-contract";
const props = withDefaults(
  defineProps<{
    port?: ServicePort;
    fixture?: boolean;
    executionPort?: SelfServicePort;
    active?: boolean;
  }>(),
  { active: true },
);
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
    [
      "binding_unknown",
      "binding_conflict",
      "binding_invalidated",
      "cleanup_pending",
    ].includes(value?.code ?? "") ||
    value?.code === "registration_login_required" ||
    value?.code === "registration_authentication_required" ||
    value?.code === "account/session/denied" ||
    value?.code === "account/login/denied"
  )
    showLogin.value = true;
  registrationError.value =
    value?.message ?? "注册操作未确认，请查询原请求后重试。";
}
function registrationFailure(code: RegistrationStatus["failure"]): string {
  switch (code) {
    case "invalidConfiguration":
      return "配置签名、组织或候选不匹配，请使用有效配置。";
    case "conflict":
      return "配置或原注册操作存在冲突，请核实原操作，不能覆盖机器身份。";
    case "denied":
      return "服务端拒绝注册，请检查资格或配置撤销状态。";
    case "expired":
      return "注册授权已过期，请核实原操作。";
    case "capacity":
      return "注册名额不足，请联系管理员。";
    case "unavailable":
      return "操作未确认，请保留原请求等待恢复。";
    default:
      return "";
  }
}
const native = props.fixture ? props.port : (props.port ?? nativeService());
async function refresh() {
  if (!native || pending.value || !props.active) return;
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
        registration.value = undefined;
        registrationError.value = "无法读取本机注册状态，请检查服务连接。";
      }
    } else registration.value = undefined;
  } catch {
    registration.value = undefined;
    view.value = { phase: "unavailable" };
  } finally {
    pending.value = false;
  }
}
async function requestRegistration(
  input?: import("../account/types").EnterpriseLogin,
) {
  if (!native || pending.value) return;
  pending.value = true;
  registering.value = true;
  try {
    registration.value = await native.requestRegistration(input);
    registrationError.value = "";
    showLogin.value = false;
  } catch (error) {
    failure(error);
  } finally {
    pending.value = false;
    registering.value = false;
  }
}
async function recoverRegistration(clear: boolean) {
  if (pending.value) return;
  pending.value = true;
  try {
    await recoverAccount(clear);
    showLogin.value = false;
    registrationError.value = "";
  } catch (error) {
    failure(error);
  } finally {
    pending.value = false;
    await refresh();
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
const agentReady = computed(
  () =>
    registration.value?.registered === true &&
    registration.value.credential === "active" &&
    registration.value.phase === "active" &&
    view.value?.phase === "connected" &&
    view.value.status.readiness.phase === "ready",
);
watch(
  () => props.active,
  (active) => {
    if (active) void refresh();
  },
);
onMounted(() => {
  void refresh();
  timer = setInterval(() => {
    if (props.active) void refresh();
  }, 30000);
});
onUnmounted(() => {
  clearInterval(timer);
  if (registering.value) void native?.cancelRegistration();
});
</script>
<template>
  <section class="registration-card" aria-label="本机执行服务">
    <header class="registration-heading">
      <div>
        <h3>Agent 设备注册</h3>
        <p>将这台电脑连接到组织的设备管理服务。</p>
      </div>
      <span class="registration-badge" :class="{ ready: agentReady }">{{
        agentReady ? "已注册" : registration ? "待完成" : "待核实"
      }}</span>
    </header>
    <p v-if="!native">请在桌面应用中查询本机服务。</p>
    <p v-else-if="pending && (!view || registering)">
      {{
        registering
          ? "正在继续原注册请求，网络恢复后将自动授权…"
          : "正在验证服务连接…"
      }}
    </p>
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
          : "已连接，执行服务暂未就绪。已有设备身份保留，请查看诊断后重新查询。"
      }}
    </p>
    <p v-else>服务不可用或对端认证未通过，请检查服务状态与访问许可。</p>
    <button v-if="registering" @click="native?.cancelRegistration()">
      停止注册等待
    </button>
    <dl v-if="view?.phase === 'connected'" class="registration-facts">
      <div>
        <dt>执行服务</dt>
        <dd>{{ view.status.build }} · {{ view.status.platform }}</dd>
      </div>
    </dl>
    <div v-if="registration">
      <p v-if="registration.failure" role="alert">
        {{ registrationFailure(registration.failure) }}
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
      <p v-else-if="registration.phase !== 'active'">
        正在处理原注册请求，授权后将自动签发并激活设备身份。
      </p>
      <dl class="registration-facts">
        <div>
          <dt>设备注册</dt>
          <dd>
            {{
              registration.registered ? "已建立设备身份" : "尚未建立设备身份"
            }}
          </dd>
        </div>
        <div>
          <dt>设备凭据</dt>
          <dd>
            {{
              registration.credential === "active"
                ? "有效"
                : registration.credential === "absent"
                  ? "尚未签发"
                  : registration.credential === "expired"
                    ? "已过期"
                    : registration.credential === "revoked"
                      ? "已撤销"
                      : "已退休"
            }}
          </dd>
        </div>
        <div v-if="registration.grantId">
          <dt>注册请求编号</dt>
          <dd>{{ registration.grantId }}</dd>
        </div>
        <div v-if="registration.publicKeyFingerprint">
          <dt>设备公钥指纹</dt>
          <dd class="registration-fingerprint">
            {{ registration.publicKeyFingerprint }}
          </dd>
        </div>
        <div v-if="registration.expiresAt">
          <dt>授权截止</dt>
          <dd>
            {{ new Date(registration.expiresAt * 1000).toLocaleString() }}
          </dd>
        </div>
      </dl>
    </div>
    <p v-if="registrationError" role="alert">{{ registrationError }}</p>
    <Account
      v-if="
        showLogin ||
        (!fixture &&
          [
            'binding_unknown',
            'cleanup_pending',
            'conflict',
            'invalidated',
          ].includes(accountBinding))
      "
      registration
      :loading="pending"
      @login="requestRegistration"
      @recover="recoverRegistration"
    />
    <button
      v-if="registration && !registration.registered && native"
      :disabled="pending"
      @click="requestRegistration()"
    >
      {{
        registration.phase === "unregistered"
          ? "登录并注册此设备"
          : "继续原注册请求"
      }}
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
  <DeviceAccess
    v-if="
      native &&
      view?.phase === 'connected' &&
      view.status.platform === 'windows'
    "
    :ready="agentReady"
    :active="active"
  />
  <MdmRegistration
    v-if="native"
    :service-port="native"
    :execution-port="executionPort"
    :fixture="fixture"
    :agent-ready="agentReady"
    :active="active"
  />
  <section v-else class="registration-card" aria-label="MDM 注册">
    <h3>MDM 注册</h3>
    <p>请在桌面应用中查询系统 MDM 状态并继续组织授权的注册任务。</p>
  </section>
</template>

<style scoped>
.registration-card {
  min-width: 0;
  padding: 22px 24px;
  margin-top: 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
  overflow-wrap: anywhere;
}
.registration-heading {
  display: flex;
  align-items: start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}
h3 {
  font-size: 14px;
  margin: 0 0 6px;
  font-weight: 600;
}
p {
  font-size: 12px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
  margin: 10px 0;
}
.registration-heading p {
  font-size: 11px;
  margin: 0;
}
.registration-badge {
  flex: none;
  font-size: 10px;
  border-radius: 999px;
  padding: 4px 9px;
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text-muted);
}
.registration-badge.ready {
  background: var(--rss-color-success-bg);
  color: var(--rss-color-success);
}
.registration-facts {
  margin: 16px 0;
}
.registration-facts > div {
  display: grid;
  grid-template-columns: 130px minmax(0, 1fr);
  gap: 12px;
  padding: 9px 0;
  border-top: 1px solid var(--rss-color-border);
  font-size: 12px;
  line-height: 1.7;
}
dt {
  color: var(--rss-color-text-muted);
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.registration-fingerprint {
  font-family: var(--rss-font-mono);
  font-size: 11px;
}
.registration-card button {
  min-height: 34px;
  padding: 7px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 7px;
  background: var(--rss-color-surface);
  font-size: 12px;
  margin: 4px 8px 4px 0;
}
.registration-card button:hover:enabled {
  background: var(--rss-color-neutral-bg);
}
.registration-card :deep(.account) {
  margin: 18px 0;
}
@media (max-width: 700px) {
  .registration-card {
    padding: 18px 16px;
  }
  .registration-facts > div {
    grid-template-columns: 100px minmax(0, 1fr);
  }
}
</style>
