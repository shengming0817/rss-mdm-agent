<script setup lang="ts">
import { computed, nextTick, ref, watch, onBeforeUnmount } from "vue";
import type {
  Connection,
  ConnectionSource,
  ConnectionDraft,
} from "@rss-mdm-agent/ai-contract";
import {
  operationMessage,
  type AssistantController,
} from "../assistant/controller";
import { saveNativeConnection, nativeTestMode } from "../test-users";
const props = defineProps<{
  controller: AssistantController;
  active?: boolean;
}>();
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
const credentialType = ref<"api_key" | "auth_token">("api_key");
const replaceKey = ref(false),
  secret = ref(""),
  revealSecret = ref(false);
const testing = ref(new Map<string, boolean>());
const secretField = ref<HTMLInputElement>();
let disposed = false;
function clearSecret() {
  secret.value = "";
  revealSecret.value = false;
  if (secretField.value) secretField.value.value = "";
}
onBeforeUnmount(() => {
  disposed = true;
  clearSecret();
});
watch(
  () => props.active,
  (active) => {
    if (active === false) clearSecret();
  },
);
const needsSecret = computed(
  () =>
    sourceType.value === "custom_api" &&
    (replaceKey.value ||
      editing.value?.source.type !== "custom_api" ||
      editing.value.provider !== provider.value ||
      editing.value.source.apiUrl !== apiUrl.value.trim() ||
      (editing.value.source.credentialType ?? "api_key") !==
        (provider.value === "claude" ? credentialType.value : "api_key")),
);
watch([provider, sourceType, apiUrl, credentialType], clearSecret);
const labels = new Map([
  ["codex", "Codex"],
  ["claude", "Claude"],
  ["deepseek", "DeepSeek"],
]);
const testMessages = new Map([
  ["host", "AI Host 或模型进程不可用，请检查 Host 状态后重试。"],
  ["configuration", "配置无法解析，请检查地址、模型及本机配置。"],
  ["authentication", "认证失败，请检查或更换凭据后保存，再重新测试。"],
  ["provider", "网络或模型服务不可用，请检查网络及服务状态后重试。"],
  ["capability", "模型不支持当前用途，请修改模型或用途后保存。"],
  ["quota", "服务限额或额度不足，请检查额度后重试。"],
  ["timeout", "测试超时，请检查网络与服务状态后重试。"],
  ["cleanup", "测试进程未确认完成清理，请检查 Host 状态后重试。"],
]);
function connectionMessage(code: string) {
  if (code === "revision_conflict")
    return "已保存版本已变化，请核对最新记录后重新编辑或测试；当前草稿保留。";
  if (code === "authentication_required")
    return "请填写凭据；若已填写，请检查应用密钥存储是否可用。";
  if (code === "ai_unavailable")
    return "AI Host 不可用，配置尚未保存；恢复连接后重试。";
  if (code === "limit_exceeded")
    return "连接数量已达上限，请删除不用的连接后重试。";
  return operationMessage(code);
}
async function testConnection(row: Connection) {
  const runtime = c.runtime.value;
  if (!runtime || testing.value.has(row.connectionId)) return;
  testing.value.set(row.connectionId, true);
  error.value = "";
  try {
    const result = await runtime.testConnection(
      row.connectionId,
      row.configRevision,
    );
    if (disposed) return;
    if (result.lastTest?.outcome === "failed")
      error.value =
        testMessages.get(result.lastTest.stage) ?? "测试结果未确认，请重试。";
    await load();
  } catch (e) {
    if (!disposed) {
      error.value =
        e && typeof e === "object" && "code" in e
          ? connectionMessage(String(e.code))
          : "测试结果未确认，请重试。";
      await load().catch(() => {});
    }
  } finally {
    testing.value.delete(row.connectionId);
  }
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
  clearSecret();
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
      const candidate: ConnectionDraft = {
        connectionId: draftId.value,
        name: name.value.trim(),
        provider: provider.value,
        profile: profile.value,
        source,
      };
      const expected = old?.configRevision ?? null;
      if (nativeTestMode) {
        if (needsSecret.value && !secret.value) {
          error.value = "请输入凭据后保存。";
          return;
        }
        const pending = saveNativeConnection(
          candidate,
          expected,
          needsSecret.value ? secret.value : undefined,
        );
        clearSecret();
        await pending;
      } else await runtime.saveConnection(candidate, expected);
    } catch (failure) {
      await load().catch(() => {});
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
      await c.runtime.value.deleteConnection(
        row.connectionId,
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
        AI Host 未连接；可先编辑配置，恢复连接后再保存；当前草稿尚未持久化。
      </p>
      <fieldset :disabled="busy">
        <p>
          保存只更新配置；测试针对已保存版本。连接修改仅影响后续上下文，已接收请求保持原绑定。
        </p>
        <p>
          连接与默认选择仅属于当前测试用户。API
          密钥在下方密码框填写并加密保存。保存不发送模型请求；测试会发送简短请求，可能产生服务费用。
        </p>
        <div :inert="pendingRemoval ? true : undefined">
          <ul>
            <li v-for="row in rows" :key="row.connectionId">
              <strong>{{ row.name }}</strong> · {{ labels.get(row.provider) }} ·
              {{ row.status === "ready" ? "可用" : "未验证" }}
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
              <span v-if="row.lastTest?.outcome === 'failed'" role="status">
                · 最近测试失败：{{ testMessages.get(row.lastTest.stage) }}</span
              >
              <button
                type="button"
                :disabled="
                  testing.has(row.connectionId) ||
                  c.state.connection !== 'connected'
                "
                :aria-label="`测试连接 ${row.name}`"
                @click="testConnection(row)"
              >
                {{ testing.has(row.connectionId) ? "正在测试…" : "测试连接" }}
              </button>
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
            <label
              >名称（必填）<input v-model="name" required maxlength="64"
            /></label>
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
                >API 地址（必填）<input
                  v-model="apiUrl"
                  required
                  placeholder="HTTPS API 地址"
                  aria-describedby="connection-api-help"
              /></label>
              <p id="connection-api-help">
                必填：输入服务提供的 HTTPS API 地址。
              </p>
              <p role="status">
                {{
                  editing?.source.type === "custom_api" && !needsSecret
                    ? "已安全保存"
                    : "未设置"
                }}
              </p>
              <button
                v-if="editing?.source.type === 'custom_api' && !needsSecret"
                type="button"
                @click="replaceKey = true"
              >
                更换凭据
              </button>
              <label
                >API Key / Auth Token（{{
                  needsSecret ? "必填" : "已保存"
                }}）<input
                  ref="secretField"
                  v-model="secret"
                  :type="revealSecret ? 'text' : 'password'"
                  :disabled="!needsSecret"
                  :required="needsSecret"
                  aria-describedby="connection-secret-help"
                  autocomplete="off"
                  autocapitalize="none"
                  :spellcheck="false"
                  :placeholder="
                    needsSecret ? '输入凭据' : '已安全保存，不回填原值'
                  "
              /></label>
              <p id="connection-secret-help">
                {{
                  needsSecret
                    ? "必填：输入此连接的凭据。"
                    : "沿用已保存凭据；更换时需要重新填写。"
                }}
              </p>
              <button
                type="button"
                :disabled="!needsSecret"
                :aria-pressed="revealSecret"
                @click="revealSecret = !revealSecret"
              >
                {{ revealSecret ? "隐藏凭据" : "显示凭据" }}
              </button>
            </template>
            <label
              >模型（{{ sourceType === "custom_api" ? "必填" : "可选" }}）<input
                v-model="model"
                :required="sourceType === 'custom_api'"
                :placeholder="
                  sourceType === 'custom_api'
                    ? '输入服务支持的模型名称'
                    : '留空使用官方配置的默认模型'
                "
                aria-describedby="connection-model-help"
            /></label>
            <p id="connection-model-help">
              {{
                sourceType === "custom_api"
                  ? "必填：自定义 API 需要指定模型。"
                  : "可选：留空使用官方配置的默认模型。"
              }}
            </p>
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
                    受控设备任务
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
                保存配置</button
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
