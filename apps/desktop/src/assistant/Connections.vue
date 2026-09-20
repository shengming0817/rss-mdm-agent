<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount } from "vue";
import type {
  Connection,
  ConnectionSource,
  HistoryPreview,
  UserPreferences,
} from "@rss-mdm-agent/ai-contract";
import { operationMessage, type AssistantController } from "./controller";
import {
  enterCredential,
  discardCredential,
  userGeneration,
  nativeTestMode,
} from "../test-users";
const props = defineProps<{ controller: AssistantController }>();
const c = props.controller;
const generation = nativeTestMode ? userGeneration() : "";
let stagedCredential = "";
async function discardStaged() {
  const reference = stagedCredential;
  stagedCredential = "";
  if (reference) {
    try {
      await discardCredential(reference, generation);
    } catch {
      error.value = "凭据清理未完成，切换用户或重新启动时会再次核对。";
    }
  }
}
onBeforeUnmount(() => {
  void discardStaged();
});
const rows = ref<Connection[]>([]),
  prefs = ref<UserPreferences>({ schemaVersion: 5, kind: "userPreferences" });
const busy = ref(false),
  error = ref(""),
  editing = ref<Connection>();
const draftId = ref<string>(crypto.randomUUID());
const name = ref(""),
  provider = ref<Connection["provider"]>("codex"),
  sourceType = ref<ConnectionSource["type"]>("existing_login");
const directory = ref(""),
  apiUrl = ref(""),
  model = ref(""),
  profile = ref<Connection["profile"]>("conversation"),
  credentialRef = ref("");
const credentialType = ref<"api_key" | "auth_token" | "oauth_token">("api_key"),
  cliProfile = ref("");
const historyMode = ref("none"),
  recent = ref(5),
  preview = ref<HistoryPreview>();
const selected = computed(() =>
  c.connectionReady.value ? (c.view.value?.selectedConnectionId ?? "") : "",
);
const labels = new Map([
  ["codex", "Codex"],
  ["claude", "Claude"],
  ["deepseek", "DeepSeek"],
]);
async function load() {
  if (!c.runtime.value) return;
  const catalog = await c.runtime.value.connections();
  rows.value = catalog.connections;
  c.state.connections = catalog.connections;
  prefs.value = catalog.preferences;
}
async function run(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await action();
  } catch (e) {
    error.value =
      e && typeof e === "object" && "code" in e
        ? operationMessage(String(e.code))
        : "连接操作未确认，请重试";
  } finally {
    busy.value = false;
  }
}
watch(
  () => c.runtime.value,
  (runtime) => {
    if (runtime) void run(load);
  },
  { immediate: true },
);
watch(
  () => c.state.selected,
  () => {
    preview.value = undefined;
    historyMode.value = "none";
  },
);
watch(provider, (value) => {
  if (value === "deepseek") sourceType.value = "custom_api";
  if (value === "claude" && sourceType.value === "existing_login")
    sourceType.value = "existing_api";
  if (value !== "codex") profile.value = "conversation";
});
watch(
  [provider, sourceType, credentialType],
  () => {
    void discardStaged();
    credentialRef.value = "";
  },
  { flush: "sync" },
);
function edit(row?: Connection) {
  void discardStaged();
  draftId.value = row?.connectionId ?? crypto.randomUUID();
  editing.value = row;
  name.value = row?.name ?? "";
  provider.value = row?.provider ?? "codex";
  sourceType.value = row?.source.type ?? "existing_login";
  directory.value =
    row && row.source.type !== "custom_api" ? row.source.directory : "";
  apiUrl.value = row?.source.type === "custom_api" ? row.source.apiUrl : "";
  credentialType.value =
    row?.source.type === "custom_api"
      ? (row.source.credentialType ?? "api_key")
      : "api_key";
  cliProfile.value =
    row?.source.type === "existing_api" ? (row.source.profile ?? "") : "";
  model.value = row?.source.model ?? "";
  profile.value = row?.profile ?? "conversation";
  credentialRef.value =
    row?.source.type === "custom_api" ? row.credentialRef : "";
}
async function secure() {
  const previous = stagedCredential;
  await discardStaged();
  if (credentialRef.value === previous) credentialRef.value = "";
  credentialRef.value = await enterCredential();
  stagedCredential = credentialRef.value;
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
        : sourceType.value === "existing_api"
          ? {
              type: "existing_api",
              directory: directory.value.trim(),
              ...(provider.value === "codex" && cliProfile.value.trim()
                ? { profile: cliProfile.value.trim() }
                : {}),
              ...(model.value.trim() ? { model: model.value.trim() } : {}),
            }
          : {
              type: "existing_login",
              directory: directory.value.trim(),
              ...(model.value.trim() ? { model: model.value.trim() } : {}),
            };
    const reference =
      source.type === "custom_api"
        ? credentialRef.value
        : (old?.credentialRef ?? `external-${crypto.randomUUID()}`);
    if (!reference) throw new Error("credential_required");
    try {
      await runtime.saveConnection(
        {
          schemaVersion: 5,
          kind: "connection",
          connectionId: draftId.value,
          name: name.value.trim(),
          provider: provider.value,
          configRevision: (old?.configRevision ?? 0) + 1,
          credentialRevision: old
            ? old.credentialRevision + Number(reference !== old.credentialRef)
            : 1,
          accountRef: old?.accountRef ?? crypto.randomUUID(),
          profile: profile.value,
          status: "unverified",
          source,
          credentialRef: reference,
        },
        old?.configRevision ?? null,
      );
    } catch (failure) {
      if (stagedCredential) {
        await discardStaged();
        credentialRef.value = "";
      }
      await load().catch(() => {});
      throw failure;
    }
    stagedCredential = "";
    edit();
    await load();
    c.state.history.clear();
    preview.value = undefined;
  });
}
async function choose(id: string, fresh = false) {
  await run(async () => {
    if (!c.runtime.value || !c.state.selected) return;
    c.state.history.delete(c.state.selected);
    preview.value = undefined;
    historyMode.value = "none";
    await c.runtime.value.selectConnection(c.state.selected, id, fresh);
  });
}
async function defaultConnection(id: string) {
  await run(async () => {
    if (!c.runtime.value) return;
    prefs.value = await c.runtime.value.savePreferences({
      defaultConnectionId: { set: id },
    });
  });
}
async function remove(row: Connection) {
  await run(async () => {
    if (!c.runtime.value) return;
    await c.runtime.value.saveConnection(
      { ...row, configRevision: row.configRevision + 1, status: "deleted" },
      row.configRevision,
    );
    await load();
  });
}
async function history() {
  c.state.history.delete(c.state.selected);
  preview.value = undefined;
  if (historyMode.value === "none") return;
  await run(async () => {
    if (!c.runtime.value || !selected.value) return;
    preview.value = await c.runtime.value.previewHistory(
      c.state.selected,
      selected.value,
      historyMode.value === "recent" ? recent.value : undefined,
    );
  });
}
</script>
<template>
  <section class="connections" aria-label="AI 连接">
    <details>
      <summary>管理 AI 连接（{{ rows.length }}）</summary>
      <p>
        连接与默认选择仅属于当前测试用户。API
        密钥在原生安全输入框中填写。验证会发送一条简短测试请求，可能产生服务费用。
      </p>
      <ul>
        <li v-for="row in rows" :key="row.connectionId">
          <strong>{{ row.name }}</strong> · {{ labels.get(row.provider) }} ·
          {{ row.status === "ready" ? "可用" : "需要更新认证" }}
          <span v-if="prefs.defaultConnectionId === row.connectionId">
            · 默认</span
          >
          <button :disabled="busy" @click="edit(row)">编辑</button
          ><button
            :disabled="
              busy ||
              row.status !== 'ready' ||
              prefs.defaultConnectionId === row.connectionId
            "
            @click="defaultConnection(row.connectionId)"
          >
            设为默认</button
          ><button :disabled="busy" @click="remove(row)">删除</button>
        </li>
      </ul>
      <p v-if="provider === 'claude' && sourceType === 'existing_login'">
        当前无法可靠核验 Claude 已有登录的账号身份，请选择已有 API 配置或自定义
        API。
      </p>
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
            <option v-if="provider === 'codex'" value="existing_login">
              已有 CLI 登录
            </option>
            <option v-if="provider !== 'deepseek'" value="existing_api">
              已有 CLI API 配置
            </option>
            <option value="custom_api">自定义 API</option>
          </select></label
        >
        <label v-if="sourceType !== 'custom_api'"
          >配置目录<input
            v-model="directory"
            required
            placeholder="配置目录的绝对路径"
        /></label>
        <label v-if="sourceType === 'existing_api' && provider === 'codex'"
          >Codex profile<input
            v-model="cliProfile"
            placeholder="留空使用 CLI 默认 profile"
        /></label>
        <template v-if="sourceType === 'custom_api'"
          ><label v-if="provider === 'claude'"
            >凭据类型<select v-model="credentialType">
              <option value="api_key">API Key</option>
              <option value="auth_token">Auth Token</option>
              <option value="oauth_token">OAuth Token</option>
            </select></label
          ><label
            >API 地址<input v-model="apiUrl" required placeholder="https://…"
          /></label>
          <div>
            <button
              type="button"
              :disabled="busy || !nativeTestMode"
              @click="run(secure)"
            >
              {{ credentialRef ? "更换安全凭据" : "填写安全凭据" }}</button
            ><span v-if="credentialRef"> 已选择凭据</span>
          </div></template
        >
        <label
          >模型<input
            v-model="model"
            :required="sourceType === 'custom_api'"
            placeholder="模型名称"
        /></label>
        <label
          >工具<select v-model="profile">
            <option value="conversation">仅对话</option>
            <option v-if="provider === 'codex'" value="controlled_tools">
              受控测试工具
            </option>
          </select></label
        >
        <div>
          <button
            :disabled="
              busy ||
              c.state.connection !== 'connected' ||
              (sourceType === 'custom_api' && !credentialRef) ||
              (provider === 'claude' && sourceType === 'existing_login')
            "
          >
            验证并保存</button
          ><button type="button" :disabled="busy" @click="edit()">
            清空表单
          </button>
        </div>
      </form>
    </details>
    <template v-if="c.view.value">
      <p v-if="!c.connectionReady.value" role="status">
        当前会话需要选择可用连接。删除连接不会删除历史。
      </p>
      <label
        >本会话连接
        <select
          :value="selected"
          :disabled="busy"
          @change="choose(($event.target as HTMLSelectElement).value)"
        >
          <option value="" disabled>请选择连接</option>
          <option
            v-for="row in rows"
            :key="row.connectionId"
            :value="row.connectionId"
          >
            {{ row.name }} · {{ labels.get(row.provider) }}
          </option>
        </select></label
      >
      <button
        :disabled="busy || !selected || c.busy.value"
        @click="choose(selected, true)"
      >
        下次输入开始新上下文
      </button>
      <p>
        切换连接后，已接收的输入先按原连接完成；下一条输入使用新连接。对话记录始终保留。
      </p>
      <label
        >带入历史
        <select
          v-model="historyMode"
          :disabled="busy || c.busy.value"
          @change="history"
        >
          <option value="none">不带入（默认）</option>
          <option value="recent">最近 N 轮已完成对话</option>
          <option value="all">全部已完成对话</option>
        </select></label
      >
      <template v-if="historyMode === 'recent'"
        ><input
          v-model.number="recent"
          aria-label="历史轮数"
          type="number"
          min="1"
          max="10000"
        /><button :disabled="busy" @click="history">更新预览</button></template
      >
      <div v-if="preview" class="history-preview">
        <p>
          以下文本将随下一条输入发送给所选连接。不会带入工具输出、系统指令或原始附件。
        </p>
        <pre>{{ preview.text || "没有可带入的已完成对话" }}</pre>
        <button
          :disabled="busy"
          @click="c.state.history.set(c.state.selected, preview)"
        >
          确认带入这些文本</button
        ><span v-if="c.state.history.has(c.state.selected)"> 已确认</span>
      </div>
    </template>
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
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
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
</style>
