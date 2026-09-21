<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { Connection, ConnectionSource } from "@rss-mdm-agent/ai-contract";
import {
  operationMessage,
  type AssistantController,
} from "../assistant/controller";
import { saveNativeConnection, nativeTestMode } from "../test-users";
const props = defineProps<{ controller: AssistantController }>();
const c = props.controller;
const rows = computed(() => c.state.connections),
  prefs = computed(() => c.state.preferences);
const busy = ref(false),
  error = ref(""),
  editing = ref<Connection>();
const pendingRemoval = ref<Connection>();
const removalDialog = ref<HTMLElement>();
let removalTrigger: HTMLElement | undefined;
const draftId = ref<string>(crypto.randomUUID());
const name = ref(""),
  provider = ref<Connection["provider"]>("codex"),
  sourceType = ref<ConnectionSource["type"]>("existing_config");
const directory = ref(""),
  apiUrl = ref(""),
  model = ref(""),
  profile = ref<Connection["profile"]>("conversation");
const replaceKey = ref(false);
const credentialType = ref<"api_key" | "auth_token">("api_key");
const labels = new Map([
  ["codex", "Codex"],
  ["claude", "Claude"],
  ["deepseek", "DeepSeek"],
]);
function connectionMessage(code: string) {
  if (code === "limit_exceeded")
    return "连接验证达到服务限额，请稍后重新验证；原连接未更改。";
  if (code === "unsupported_capability")
    return "所选模型不支持当前连接用途，或实际模型与配置不一致。请明确选择兼容模型后重新验证；不会自动换模型。";
  if (code === "unavailable")
    return "模型验证结果未确认，请检查服务后重新验证；原连接未更改。";
  return operationMessage(code);
}
async function load() {
  await c.refreshConnections();
}
async function run(action: () => Promise<void>) {
  if (busy.value || c.state.connection !== "connected") return;
  busy.value = true;
  error.value = "";
  try {
    await action();
  } catch (e) {
    error.value =
      e && typeof e === "object" && "code" in e
        ? connectionMessage(String(e.code))
        : "连接操作未确认，请重试";
  } finally {
    busy.value = false;
  }
}
watch(provider, (value) => {
  if (value === "deepseek") sourceType.value = "custom_api";
  if (value !== "codex") profile.value = "conversation";
});
function edit(row?: Connection) {
  replaceKey.value = false;
  draftId.value = row?.connectionId ?? crypto.randomUUID();
  editing.value = row;
  name.value = row?.name ?? "";
  provider.value = row?.provider ?? "codex";
  sourceType.value = row?.source.type ?? "existing_config";
  directory.value =
    row && row.source.type !== "custom_api" ? (row.source.directory ?? "") : "";
  apiUrl.value = row?.source.type === "custom_api" ? row.source.apiUrl : "";
  credentialType.value =
    row?.source.type === "custom_api"
      ? (row.source.credentialType ?? "api_key")
      : "api_key";
  model.value = row?.source.model ?? "";
  profile.value = row?.profile ?? "conversation";
}
async function save() {
  await run(async () => {
    const runtime = c.runtime.value;
    if (!runtime) return;
    const old = editing.value;
    const source: ConnectionSource =
      sourceType.value === "custom_api"
        ? {
            type: "custom_api",
            apiUrl: apiUrl.value.trim(),
            model: model.value.trim(),
            credentialType:
              provider.value === "claude" ? credentialType.value : "api_key",
          }
        : {
            type: "existing_config",
            ...(directory.value.trim()
              ? { directory: directory.value.trim() }
              : {}),
            ...(model.value.trim() ? { model: model.value.trim() } : {}),
          };
    try {
      const candidate: Connection = {
        schemaVersion: 5,
        kind: "connection",
        connectionId: draftId.value,
        name: name.value.trim(),
        provider: provider.value,
        configRevision: (old?.configRevision ?? 0) + 1,
        profile: profile.value,
        status: "unverified",
        source,
      };
      const expected = old?.configRevision ?? null;
      const targetChanged =
        old?.source.type === "custom_api" &&
        source.type === "custom_api" &&
        (old.provider !== candidate.provider ||
          old.source.apiUrl !== source.apiUrl ||
          (old.source.credentialType ?? "api_key") !==
            (source.credentialType ?? "api_key"));
      if (nativeTestMode)
        await saveNativeConnection(
          candidate,
          expected,
          replaceKey.value ||
            targetChanged ||
            old?.source.type !== "custom_api",
        );
      else await runtime.saveConnection(candidate, expected);
    } catch (failure) {
      await load().catch(() => {});
      if (
        failure &&
        typeof failure === "object" &&
        "code" in failure &&
        failure.code === "revision_conflict"
      )
        edit();
      throw failure;
    }
    edit();
    await load();
    c.state.history.clear();
  });
}
async function defaultConnection(id: string) {
  await run(async () => {
    if (!c.runtime.value) return;
    c.state.preferences = await c.runtime.value.savePreferences({
      defaultConnectionId: { set: id },
    });
  });
}
async function remove(row: Connection) {
  await run(async () => {
    if (!c.runtime.value) return;
    try {
      await c.runtime.value.saveConnection(
        { ...row, configRevision: row.configRevision + 1, status: "deleted" },
        row.configRevision,
      );
      closeRemoval();
      await load();
    } catch (failure) {
      await load().catch(() => {});
      if (
        failure &&
        typeof failure === "object" &&
        "code" in failure &&
        failure.code === "revision_conflict"
      )
        closeRemoval();
      throw failure;
    }
  });
}
async function requestRemoval(row: Connection, event: Event) {
  removalTrigger = event.currentTarget as HTMLElement;
  pendingRemoval.value = row;
  await nextTick();
  removalDialog.value?.querySelector<HTMLElement>("button")?.focus();
}
function closeRemoval() {
  pendingRemoval.value = undefined;
  const trigger = removalTrigger;
  removalTrigger = undefined;
  void nextTick(() => trigger?.focus());
}
function containRemovalFocus(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    closeRemoval();
    return;
  }
  if (event.key !== "Tab" || !removalDialog.value) return;
  const buttons = [
    ...removalDialog.value.querySelectorAll<HTMLElement>("button"),
  ];
  if (!buttons.length) return;
  const first = buttons[0],
    last = buttons.at(-1)!;
  if (event.shiftKey && event.target === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && event.target === last) {
    event.preventDefault();
    first.focus();
  }
}
</script>
<template>
  <section class="connections" aria-label="AI 连接">
    <div>
      <h3>个人 AI 连接（{{ rows.length }}）</h3>
      <p v-if="c.state.connection !== 'connected'" role="status">
        AI Host 未连接；可先编辑配置，恢复连接后再验证并保存。
      </p>
      <fieldset :disabled="busy">
        <p>
          连接修改仅影响后续上下文；已接收请求保持原绑定。验证会产生新修订，失败不改变原配置。
        </p>
        <p>
          连接与默认选择仅属于当前测试用户。API
          密钥在原生安全输入框中填写。验证会发送一条简短测试请求，可能产生服务费用。
        </p>
        <div :inert="pendingRemoval ? true : undefined">
          <ul>
            <li v-for="row in rows" :key="row.connectionId">
              <strong>{{ row.name }}</strong> · {{ labels.get(row.provider) }} ·
              {{ row.status === "ready" ? "可用" : "需要更新认证" }}
              <span>
                ·
                {{
                  row.source.type === "existing_config"
                    ? row.source.directory
                      ? "官方本机配置 · 指定目录"
                      : "官方本机配置 · 默认目录"
                    : "自定义 API"
                }}
                · {{ row.source.model || "官方配置决定模型" }} · 修订
                {{ row.configRevision }}</span
              >
              <span v-if="prefs.defaultConnectionId === row.connectionId">
                · 默认</span
              >
              <button
                :aria-label="`编辑连接 ${row.name}`"
                :disabled="busy"
                @click="edit(row)"
              >
                编辑</button
              ><button
                :disabled="
                  busy ||
                  c.state.connection !== 'connected' ||
                  row.status !== 'ready' ||
                  prefs.defaultConnectionId === row.connectionId
                "
                :aria-label="`将连接 ${row.name} 设为默认`"
                @click="defaultConnection(row.connectionId)"
              >
                设为默认</button
              ><button
                :aria-label="`删除连接 ${row.name}`"
                :disabled="busy || c.state.connection !== 'connected'"
                @click="requestRemoval(row, $event)"
              >
                删除
              </button>
            </li>
          </ul>
          <form class="connection-form" @submit.prevent="save">
            <h3>{{ editing ? "编辑连接" : "添加连接" }}</h3>
            <label>名称<input v-model="name" required maxlength="64" /></label>
            <label
              >服务<select v-model="provider">
                <option value="codex">Codex</option>
                <option value="claude">Claude</option>
                <option value="deepseek">DeepSeek</option>
              </select></label
            >
            <label
              >认证来源<select v-model="sourceType">
                <option v-if="provider !== 'deepseek'" value="existing_config">
                  本机已有配置
                </option>
                <option value="custom_api">自定义 API</option>
              </select></label
            >
            <template v-if="sourceType === 'custom_api'"
              ><label v-if="provider === 'claude'"
                >凭据类型<select v-model="credentialType">
                  <option value="api_key">API Key</option>
                  <option value="auth_token">Auth Token</option>
                </select></label
              ><label
                >API 地址<input
                  v-model="apiUrl"
                  required
                  placeholder="HTTPS API 地址"
              /></label>
              <label v-if="editing?.source.type === 'custom_api'">
                <input v-model="replaceKey" type="checkbox" />更换 API 密钥
              </label>
              <p>
                {{
                  editing?.source.type === "custom_api" && !replaceKey
                    ? "保存时保留原密钥。"
                    : "点击验证并保存后，在原生输入框填写 API 密钥。"
                }}
              </p>
            </template>
            <label
              >模型<input
                v-model="model"
                :required="sourceType === 'custom_api'"
                placeholder="留空使用官方配置的默认模型"
            /></label>
            <details>
              <summary>高级选项</summary>
              <label v-if="sourceType !== 'custom_api'"
                >配置目录<input
                  v-model="directory"
                  placeholder="留空使用 ~/.codex 或 ~/.claude"
              /></label>
              <label
                >工具<select v-model="profile">
                  <option value="conversation">仅对话</option>
                  <option v-if="provider === 'codex'" value="controlled_tools">
                    受控测试工具
                  </option>
                </select></label
              >
            </details>
            <div>
              <button
                :disabled="
                  busy ||
                  c.state.connection !== 'connected' ||
                  (sourceType === 'custom_api' && !nativeTestMode)
                "
              >
                验证并保存</button
              ><button type="button" :disabled="busy" @click="edit()">
                清空表单
              </button>
            </div>
          </form>
        </div>
        <div
          v-if="pendingRemoval"
          ref="removalDialog"
          role="alertdialog"
          aria-modal="true"
          aria-label="删除连接确认"
          @keydown="containRemovalFocus"
        >
          <p>
            确认删除 {{ pendingRemoval.name }}（{{
              labels.get(pendingRemoval.provider)
            }}）？连接将不可再使用，保存的 API 密钥会删除；所有会话历史保留。
          </p>
          <button :disabled="busy" @click="closeRemoval">取消删除</button>
          <button
            :disabled="busy || c.state.connection !== 'connected'"
            @click="remove(pendingRemoval)"
          >
            确认删除
          </button>
        </div>
      </fieldset>
    </div>
    <p v-if="busy" role="status">正在处理连接…</p>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.connections {
  padding: 16px;
  border: 1px solid #d6e0df;
  border-radius: 10px;
  margin: 16px 0;
}
.connections p {
  color: #536765;
  font-size: 13px;
}
.connections button {
  margin: 4px;
}
.connection-form {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr));
  gap: 12px;
  margin: 16px 0;
}
.connection-form h3 {
  grid-column: 1/-1;
}
.connection-form label {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
input,
select {
  border: 1px solid #c6d6d3;
  border-radius: 6px;
  padding: 8px;
}
.history-preview pre {
  white-space: pre-wrap;
  max-height: 280px;
  overflow: auto;
  padding: 12px;
  background: #f3f6f5;
}
fieldset {
  border: 0;
  padding: 0;
  min-width: 0;
}
input,
select {
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
}
</style>
