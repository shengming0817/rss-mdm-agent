<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, shallowRef } from "vue";
import {
  nativePersonalVault,
  type PersonalVaultPort,
  type PersonalVaultOperation,
  type PersonalVaultStatus,
} from "./native";
const props = defineProps<{ port?: PersonalVaultPort }>();
const port = props.port ?? nativePersonalVault();
const state = shallowRef<PersonalVaultStatus>();
const error = ref("");
const busy = ref(false);
const selected = ref("");
const deleting = shallowRef<{
  entry: PersonalVaultStatus["entries"][number];
  revision: number;
}>();
const resetOpen = ref(false);
const resetText = ref("");
const authorization = ref<"system" | "password">("system");
let generation = 0;
let readFailed = false;
let timer: ReturnType<typeof setInterval> | undefined;
const messages: Record<string, string> = {
  locked: "Vault 已锁定，请选择解锁方式。",
  cancelled: "操作已取消，请重试。",
  busy: "另一个 Vault 操作正在进行，请稍后重试。",
  input:
    "输入无效。新口令须至少 12 个字符、至多 1024 个 UTF-8 字节，两次输入须相同。",
  authentication: "认证失败，请核对口令或系统验证。",
  system_unavailable: "系统解锁不可用；如已设置个人口令，可使用口令解锁。",
  not_found: "材料不存在；如已设置个人口令，可使用口令解锁并重新建立系统保护。",
  corrupt: "Vault 材料损坏或不可用。原文件已保留，请勿覆盖。",
  unsupported: "此 Vault 格式或平台能力不受支持，原数据保持不变。",
  owner: "此 Vault 不属于当前 OS 用户，或目录权限不安全。",
  storage: "保存未完成，请检查本地存储；原材料不会自动清空。",
  conflict: "Vault 已发生变化，请刷新后重试。",
  capacity: "Vault 已达到容量限制。",
};
async function refresh() {
  if (!port || busy.value) return;
  const current = generation;
  try {
    const next = await port({ kind: "status" });
    if (current !== generation) return;
    state.value = next;
    if (readFailed) {
      error.value = "";
      readFailed = false;
    }
    if (!next.unlocked) {
      selected.value = "";
      deleting.value = undefined;
    }
  } catch (e) {
    if (current === generation) {
      state.value = undefined;
      readFailed = true;
      error.value = messages[String(e)] ?? "个人 Vault 暂不可用。";
    }
  }
}
async function run(operation: PersonalVaultOperation) {
  if (!port || (busy.value && operation.kind !== "lock")) return;
  const current = ++generation;
  busy.value = true;
  readFailed = false;
  error.value = "";
  if (operation.kind === "lock" && state.value)
    state.value = { ...state.value, unlocked: false, entries: [] };
  try {
    const next = await port(operation);
    if (current !== generation) return;
    state.value = next;
    selected.value = "";
    deleting.value = undefined;
    resetOpen.value = false;
    resetText.value = "";
  } catch (e) {
    if (current === generation)
      error.value = messages[String(e)] ?? "个人 Vault 操作未完成。";
  } finally {
    if (current === generation) busy.value = false;
  }
}
function chosen() {
  return state.value?.entries.find(
    (e) => e.credentialId.join("-") === selected.value,
  );
}
function confirmDeletion() {
  const entry = chosen();
  if (entry && state.value)
    deleting.value = { entry, revision: state.value.revision };
}
async function remove() {
  const target = deleting.value;
  if (target)
    await run({
      kind: "delete",
      credential_id: target.entry.credentialId,
      revision: target.revision,
    });
}
onMounted(() => {
  void refresh();
  timer = setInterval(() => {
    void refresh();
  }, 1500);
});
onBeforeUnmount(() => {
  generation++;
  if (timer) clearInterval(timer);
  state.value = undefined;
});
</script>
<template>
  <section class="personal-vault" aria-label="个人 Passkey Vault">
    <p>
      当前 OS 用户的个人凭据 ·
      仅本机，尚未同步。企业账号退出、退管和卸载不会主动清空此 Vault。
    </p>
    <p v-if="!port">请在原生桌面应用中管理个人 Passkey。</p>
    <template v-else>
      <p v-if="error" role="alert">{{ error }}</p>
      <p v-if="busy" role="status">请在系统窗口完成操作，或取消。</p>
      <button
        v-if="state && !state.initialized"
        :disabled="busy"
        @click="run({ kind: 'create' })"
      >
        创建个人 Vault
      </button>
      <template v-if="state?.initialized">
        <p>
          {{ state.unlocked ? "已解锁" : "已锁定" }} ·
          {{ state.passwordEnabled ? "已启用个人口令" : "系统解锁" }}
        </p>
        <div class="vault-actions">
          <button
            :disabled="busy"
            @click="run({ kind: 'unlock', method: 'system' })"
          >
            系统解锁
          </button>
          <button
            v-if="state.passwordEnabled"
            :disabled="busy"
            @click="run({ kind: 'unlock', method: 'password' })"
          >
            口令解锁
          </button>
          <button @click="run({ kind: 'lock' })">锁定 / 取消</button>
        </div>
        <template v-if="state.unlocked">
          <p v-if="!state.entries.length">
            尚无 Passkey。网站创建与使用入口由后续平台 Provider 提供。
          </p>
          <fieldset v-else>
            <legend>选择网站账号</legend>
            <label
              v-for="entry in state.entries"
              :key="entry.credentialId.join('-')"
            >
              <input
                v-model="selected"
                type="radio"
                name="personal-passkey"
                :value="entry.credentialId.join('-')"
              />
              <span
                >{{ entry.rpId }} · {{ entry.displayName || entry.username }}
                <small>{{ entry.username }}</small></span
              >
            </label>
          </fieldset>
          <button
            v-if="chosen() && !deleting"
            :disabled="busy"
            @click="confirmDeletion"
          >
            删除所选本地条目
          </button>
          <div v-if="deleting" role="group" aria-label="确认删除本地条目">
            <p>
              将删除 {{ deleting.entry.rpId }} ·
              {{ deleting.entry.displayName || deleting.entry.username }}（{{
                deleting.entry.username
              }}）
            </p>
            <p>
              仅删除本地条目；不会删除网站端公钥或其他副本。没有备份时无法恢复，需要在网站重新注册。
            </p>
            <button :disabled="busy" @click="remove">确认删除</button>
            <button @click="deleting = undefined">取消</button>
          </div>
        </template>
        <div class="vault-actions">
          <label v-if="state.passwordEnabled"
            >修改口令时验证方式
            <select v-model="authorization">
              <option value="system">系统认证</option>
              <option value="password">当前个人口令</option>
            </select>
          </label>
          <button
            :disabled="busy"
            @click="
              run({
                kind: 'set_password',
                authorization: state.passwordEnabled ? authorization : 'system',
              })
            "
          >
            {{ state.passwordEnabled ? "修改个人口令" : "设置个人口令" }}
          </button>
          <button
            v-if="state.passwordEnabled"
            :disabled="busy"
            @click="run({ kind: 'remove_password' })"
          >
            移除口令（需系统认证）
          </button>
          <button
            v-if="state.passwordEnabled"
            :disabled="busy"
            @click="run({ kind: 'repair_system' })"
          >
            用口令重建系统解锁
          </button>
        </div>
        <p>
          个人口令在原生安全输入窗口输入，不经过网页表单。口令不是备份；数据丢失不能仅凭口令恢复。改口令不撤销已经保存的历史密文。
        </p>
        <p>
          凭据由系统保护和本地加密保存；不宣称每把 Passkey
          私钥均为硬件不可导出。
        </p>
      </template>
      <button
        v-if="state?.initialized || error"
        :disabled="busy"
        @click="resetOpen = !resetOpen"
      >
        损坏 / 丢失处理
      </button>
      <div v-if="resetOpen" role="group" aria-label="重置个人 Vault">
        <p>
          先尝试其他解锁方式。重置会建立新空
          Vault，保留旧密文，但不保证能恢复旧凭据；不会修改网站端注册。
        </p>
        <label
          >输入 DELETE LOCAL VAULT 确认重置
          <input v-model="resetText" autocomplete="off"
        /></label>
        <button
          :disabled="busy || resetText !== 'DELETE LOCAL VAULT'"
          @click="run({ kind: 'reset', confirmation: resetText })"
        >
          系统确认并建立新 Vault
        </button>
        <button
          @click="
            resetOpen = false;
            resetText = '';
          "
        >
          取消
        </button>
      </div>
    </template>
  </section>
</template>
<style scoped>
.personal-vault {
  display: grid;
  gap: 12px;
}
.personal-vault p {
  margin: 0;
  line-height: 1.6;
}
.vault-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}
fieldset {
  border: 1px solid var(--border-color, #8885);
  border-radius: 8px;
  padding: 12px;
}
fieldset label {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px;
}
small {
  display: block;
  opacity: 0.75;
}
button {
  padding: 7px 12px;
  width: fit-content;
}
</style>
