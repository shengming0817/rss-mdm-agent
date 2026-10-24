<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { currentUser } from "../test-users";
import {
  nativeDeviceAccessPort,
  type DeviceAccessPort,
} from "../self-service/native";
import type { SelfServiceCommands } from "../self-service/types";
type Result = SelfServiceCommands["device_access"]["output"];
type Directory = Extract<Result, { kind: "directory" }>;
type Task = Extract<Result, { kind: "tasks" }>["tasks"][number];
const props = withDefaults(
  defineProps<{
    port?: DeviceAccessPort;
    active?: boolean;
    ready?: boolean;
    platform?: string;
  }>(),
  { active: true, ready: true },
);
const mac = computed(() => props.platform === "macos");
const macStatus = ref<Extract<Result, { kind: "macStatus" }>>();
const windowsStatus = ref<Extract<Result, { kind: "windowsStatus" }>>();
function componentText(
  component: Extract<Result, { kind: "windowsStatus" }>["component"],
) {
  switch (component) {
    case "ready":
      return "组件检查通过，实际登录仍需验证";
    case "rebootRequired":
      return "组件已注册，请重启电脑后重试应用";
    case "notInstalled":
      return "组件未安装，请联系管理员";
    case "maintenanceDisabled":
      return "组件已由管理员停用，需维修后启用";
    case "unavailable":
      return "组件检查失败，请联系管理员维修";
  }
}

const directory = ref<Directory>(),
  tasks = ref<Task[]>([]),
  selected = ref<Task>();
const secret = ref(""),
  code = ref(""),
  password = ref("");
const busy = ref(false),
  message = ref(""),
  failure = ref("");
let epoch = 0,
  disposed = false;
const available = computed(
  () => props.active && props.ready && (props.port || currentUser.value),
);
function owner() {
  return props.port ?? (currentUser.value ? nativeDeviceAccessPort() : null);
}
function clearSecret() {
  secret.value = "";
  code.value = "";
  password.value = "";
  selected.value = undefined;
}
function reset() {
  epoch++;
  clearSecret();
  directory.value = undefined;
  macStatus.value = undefined;
  windowsStatus.value = undefined;
  tasks.value = [];
  busy.value = false;
  message.value = "";
  failure.value = "";
}
async function action(
  run: (port: DeviceAccessPort, valid: () => boolean) => Promise<void>,
) {
  const port = owner();
  if (!port || !available.value || busy.value) return;
  const own = ++epoch,
    generation = currentUser.value?.generation;
  const valid = () =>
    !disposed &&
    own === epoch &&
    generation === currentUser.value?.generation &&
    !!available.value;
  busy.value = true;
  failure.value = "";
  try {
    await run(port, valid);
  } catch {
    if (valid()) {
      failure.value = "操作尚未确认，请刷新状态后重试。";
      clearSecret();
    }
  } finally {
    if (valid()) busy.value = false;
  }
}
async function refresh() {
  await action(async (port, valid) => {
    if (!mac.value) {
      windowsStatus.value = undefined;
      const status = await port({ kind: "windowsStatus" });
      if (!valid()) return;
      if (status.kind !== "windowsStatus") throw new Error("response");
      windowsStatus.value = status;
    }
    const [listing, pending] = await Promise.all([
      port({ kind: "directory", cursor: null }),
      port({ kind: "tasks" }),
    ]);
    if (!valid()) return;
    if (listing.kind !== "directory" || pending.kind !== "tasks")
      throw new Error("response");
    directory.value = listing;
    tasks.value = pending.tasks;
    if (mac.value) {
      const status = await port({ kind: "macStatus" });
      if (valid() && status.kind === "macStatus") macStatus.value = status;
    }
  });
}
async function more() {
  const cursor = directory.value?.nextCursor;
  if (!cursor) return;
  await action(async (port, valid) => {
    const result = await port({ kind: "directory", cursor });
    if (!valid()) return;
    if (result.kind !== "directory") throw new Error("response");
    directory.value = {
      ...result,
      policies: [...(directory.value?.policies ?? []), ...result.policies],
    };
  });
}
async function enroll(policy: string) {
  await action(async (port, valid) => {
    const operationId = crypto.randomUUID();
    const begin = await port({ kind: "begin", operationId, policy });
    if (!valid()) return;
    if (begin.kind !== "challenge") throw new Error("response");
    const result = await port({ kind: "confirm", operation: operationId });
    if (!valid()) return;
    if (result.kind !== "binding") throw new Error("response");
    clearSecret();
    message.value = mac.value
      ? "关联已确认。请在系统设置的用户与群组中完成 Platform SSO 注册；登录、解锁及磁盘恢复按组织策略执行。"
      : "关联请求已确认。请刷新并完成本地验证器设置。";
  });
}
async function prepare(task: Task) {
  clearSecret();
  if (mac.value) {
    selected.value = task;
    return;
  }
  await action(async (port, valid) => {
    const result = await port({ kind: "prepare", task: task.task });
    if (!valid()) return;
    if (result.kind !== "setup" || result.task !== task.task)
      throw new Error("response");
    selected.value = task;
    secret.value = result.secret;
    message.value = "";
  });
}
async function confirmMac() {
  const task = selected.value;
  if (!task) return;
  const input = {
    kind: "prepareMac" as const,
    task: task.task,
    password: password.value,
  };
  password.value = "";
  await action(async (port, valid) => {
    try {
      const result = await port(input);
      if (!valid()) return;
      if (result.kind !== "prepared") throw new Error("response");
      clearSecret();
      message.value =
        "本机密码已提交给受控账号操作。Secure Token 和系统注册状态需分别确认。";
    } finally {
      input.password = "";
    }
  });
}
async function confirm() {
  const task = selected.value;
  if (!task) return;
  const input = {
    kind: "confirmFactor" as const,
    task: task.task,
    code: code.value,
    password: task.createName ? password.value : null,
  };
  code.value = "";
  password.value = "";
  await action(async (port, valid) => {
    try {
      const result = await port(input);
      if (!valid()) return;
      if (result.kind !== "prepared") throw new Error("response");
      clearSecret();
      message.value = "本地设置已保存，正在等待 Agent 应用。请刷新查看结果。";
    } finally {
      input.code = "";
      input.password = null;
    }
  });
}
function accountText(binding: Directory["bindings"][number]) {
  const account = binding.account;
  if (!account) return "账号待确认";
  if (account.platform === "macos") return "macOS 账号";
  switch (account.authority.kind) {
    case "local":
      return "Windows 本地账号";
    case "active_directory":
      return "AD 域账号";
    case "entra":
      return "Entra joined 账号";
  }
}
function bindingText(binding: Directory["bindings"][number]) {
  if (binding.effect?.state === "unbound") {
    switch (binding.effect.disposition) {
      case "keep":
        return "RSS 关联已移除，系统账号保持原状";
      case "disable":
        return "RSS 关联已移除，本机登录已限制";
      case "standard":
        return "RSS 关联已移除，本机管理员资格已移除";
    }
  }
  if (binding.revoked) return "已请求移除，等待本机确认";
  if (
    binding.appliedRevision === binding.revision &&
    binding.effect?.state === "applied"
  )
    return binding.configuration.enabled
      ? "服务端最近回执：RSS 登录已应用"
      : "服务端最近回执：RSS 登录已停用";
  if (binding.effect?.state === "failed") return "本机应用失败，需要处理后重试";
  if (binding.effect?.state === "unknown")
    return "本机结果未确定，请联系管理员核查";
  return "等待本机应用";
}
watch(
  [
    () => props.port,
    () => props.active,
    () => props.ready,
    () => currentUser.value?.generation,
  ],
  () => {
    reset();
    if (available.value) void refresh();
  },
);
onMounted(() => {
  if (available.value) void refresh();
});
onUnmounted(() => {
  disposed = true;
  reset();
});
</script>

<template>
  <section
    class="device-access"
    :aria-label="mac ? 'Mac Platform SSO' : 'Windows RSS 登录'"
  >
    <h3>{{ mac ? "Mac Platform SSO" : "Windows RSS 登录" }}</h3>
    <p v-if="mac">
      使用当前本地账号关联 RSS，由系统按组织配置使用 Secure Enclave 或 Password
      注册与认证。登录窗口、锁屏和 FileVault
      分别受策略约束；离线可用性及恢复入口需按实际配置核实。Secure
      Token、FileVault 和 SSO 是独立状态。
    </p>
    <p v-else>
      在 RSS 登录选项中，使用 Windows
      密码和验证器验证码登录；恢复密钥可代替验证码。系统原有登录选项继续保留。
    </p>
    <p v-if="!mac">
      首次启用或设备重新加入 Entra 后，请注销 Windows
      并重新登录，再关联当前账号。
    </p>
    <p v-if="!available">请登录 RSS 账户并保持当前本地会话解锁。</p>
    <template v-else>
      <button :disabled="busy" @click="refresh">刷新状态</button>
      <p v-if="!mac" role="status">
        本机当前状态：{{
          windowsStatus
            ? componentText(windowsStatus.component)
            : "尚未取得本机观察"
        }}。 当前账号因子：{{
          windowsStatus?.factorActive === true
            ? "已启用"
            : windowsStatus?.factorActive === false
              ? "未启用"
              : "未知"
        }}。
        <span v-if="windowsStatus?.signInRequired"
          >请注销 Windows
          并重新登录，再关联账号；注销与重启组件是不同操作。</span
        >
        管理员组配置修改后，已有会话的权限需注销并重新登录才会刷新。
      </p>
      <p v-if="macStatus" role="status">
        模式：{{
          macStatus.configuration?.authentication === "password"
            ? "Password"
            : macStatus.configuration
              ? "Secure Enclave"
              : "待同步配置"
        }}； 扩展：{{
          macStatus.extensionInstalled ? "已验证安装" : "未安装或需修复"
        }}； 设备注册：{{
          macStatus.deviceRegistered === null
            ? "未知"
            : macStatus.deviceRegistered
              ? "已注册"
              : "未注册"
        }}； 用户注册：{{
          macStatus.userRegistered === null
            ? "未知"
            : macStatus.userRegistered
              ? "已注册"
              : "未注册"
        }}； SSO：{{
          macStatus.loginReady === null
            ? "服务暂不可用"
            : macStatus.loginReady
              ? "服务端允许认证，尚需实际登录确认"
              : "待注册或修复"
        }}。 系统用户注册：{{
          macStatus.nativeUserRegistered === null
            ? "暂无系统回执"
            : macStatus.nativeUserRegistered
              ? "已观察"
              : "未注册"
        }}。
      </p>
      <p
        v-if="macStatus?.configuration?.authentication === 'password'"
        role="status"
      >
        密码交换：{{
          macStatus.passwordExchange?.state === "succeeded"
            ? "服务端已验证"
            : macStatus.passwordExchange?.state === "rejected"
              ? "凭据被拒绝"
              : macStatus.passwordExchange?.state === "unavailable"
                ? "服务暂不可用"
                : "尚无确定结果"
        }}。 本地密码与登录钥匙串同步仍须分别核实；请从系统设置的 Platform SSO
        入口完成注册或修复。离线本地改密不会自动更新组织密码。
      </p>
      <p v-if="failure" role="alert">{{ failure }}</p>
      <p v-if="message" role="status">{{ message }}</p>
      <ul v-if="directory?.bindings.length">
        <li v-for="binding in directory.bindings" :key="binding.id">
          {{ accountText(binding) }} · {{ bindingText(binding) }}
        </li>
      </ul>
      <div v-for="policy in directory?.policies" :key="policy.policy">
        <p>策略 {{ policy.policy }} · 版本 {{ policy.version }}</p>
        <p>
          RSS 登录：{{ policy.configuration.enabled ? "启用" : "停用" }}；
          账户角色：{{
            policy.configuration.role === "administrator"
              ? "管理员"
              : policy.configuration.role === "standard"
                ? "标准用户"
                : "保留现有角色"
          }}
        </p>
        <button :disabled="busy" @click="enroll(policy.policy)">
          关联当前 Windows / macOS 账号
        </button>
      </div>
      <button v-if="directory?.nextCursor" :disabled="busy" @click="more">
        更多可用策略
      </button>
      <p>
        已有绑定的修改、新账户创建和认证材料重置由管理员在后台授权。
        本地管理员身份不授予 RSS 管理权限。
      </p>
      <div v-for="task in tasks" :key="task.task">
        <p v-if="task.createName">
          组织已授权创建本地账户：{{ task.createName }}
        </p>
        <button :disabled="busy" @click="prepare(task)">
          {{
            mac
              ? task.createName
                ? "设置本机账号密码"
                : "完成 Secure Token 授权"
              : "设置验证器"
          }}
        </button>
      </div>
      <form
        v-if="mac && selected"
        autocomplete="off"
        @submit.prevent="confirmMac"
      >
        <label
          >{{ selected.createName ? "新本地账号密码" : "当前本地账号密码"
          }}<input
            v-model="password"
            type="password"
            maxlength="256"
            required
            autocomplete="new-password"
        /></label>
        <p>
          密码只在本机用于建号。若系统需要 Secure Token
          授权，请由合格本机管理员完成；未确认前不会显示已完成。
        </p>
        <button :disabled="busy" type="submit">
          {{ selected.createName ? "确认建号" : "由系统请求管理员授权" }}
        </button>
        <button :disabled="busy" type="button" @click="clearSecret">
          取消
        </button>
      </form>
      <form
        v-if="!mac && selected && secret"
        autocomplete="off"
        @submit.prevent="confirm"
      >
        <p>
          在验证器中添加时间型动态口令，手动输入以下密钥。仅在此设置页面显示。
        </p>
        <label
          >验证器密钥
          <input :value="secret" readonly autocomplete="off" spellcheck="false"
        /></label>
        <label
          >6 位验证码
          <input
            v-model="code"
            inputmode="numeric"
            pattern="[0-9]{6}"
            maxlength="6"
            required
            autocomplete="one-time-code"
        /></label>
        <label v-if="selected.createName"
          >新 Windows 账户密码
          <input
            v-model="password"
            type="password"
            maxlength="256"
            required
            autocomplete="new-password"
        /></label>
        <p v-if="selected.createName">
          密码只在本机用于创建账户，不上传到服务器。
        </p>
        <button :disabled="busy" type="submit">确认并应用</button>
        <button :disabled="busy" type="button" @click="clearSecret">
          关闭设置
        </button>
      </form>
    </template>
  </section>
</template>

<style scoped>
.device-access {
  min-width: 0;
  padding: 20px;
  border: 1px solid var(--border-color, #ddd);
  border-radius: 12px;
}
p {
  line-height: 1.6;
}
label {
  display: grid;
  gap: 6px;
  margin: 12px 0;
}
input {
  padding: 8px;
  max-width: 100%;
}
button {
  margin: 6px 8px 6px 0;
}
</style>
