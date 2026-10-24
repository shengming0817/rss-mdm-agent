<script setup lang="ts">
import { computed, inject, reactive, ref, watch } from "vue";
import {
  UiButton,
  UiDialog,
  UiSelect,
  UiSwitch,
  UiTextInput,
} from "@rss-mdm-agent/ui";
import { systemSettingsKey } from "./controller";
import type {
  ProxyEndpoint,
  SettingsPatch,
  SettingsSection,
  SettingsTarget,
} from "../self-service/types";
const props = defineProps<{ section: SettingsSection }>();
const controller = inject(systemSettingsKey, undefined);
const effective = computed<SettingsSection>(() =>
  props.section === "lock" ? "power" : props.section,
);
const response = computed(() =>
  controller?.state.observations.get(effective.value),
);
const snapshot = computed(() => response.value?.snapshot);
const targets = computed(() => snapshot.value?.targets ?? []);
const selected = ref("");
watch(targets, (values) => {
  if (!values.some((t) => t.id === selected.value))
    selected.value = values.find((t) => t.writable)?.id ?? values[0]?.id ?? "";
});
const target = computed(
  () => targets.value.find((t) => t.id === selected.value) ?? targets.value[0],
);
const value = (t: SettingsTarget | undefined, key: string) =>
  t?.fields.find((f) => f.key === key)?.value;
const mode = ref(""),
  x = ref(""),
  y = ref(""),
  displayOff = ref(""),
  sleep = ref(""),
  energy = ref(""),
  bypass = ref("");
const protocols = ["http", "https", "socks"] as const;
const proxies = reactive<Record<(typeof protocols)[number], ProxyEndpoint>>({
  http: { enabled: false, host: "", port: 0 },
  https: { enabled: false, host: "", port: 0 },
  socks: { enabled: false, host: "", port: 0 },
});
const ports = reactive({ http: "", https: "", socks: "" });
const original = ref("");
const submitting = ref(false);
const form = () =>
  JSON.stringify({
    mode: mode.value,
    x: x.value,
    y: y.value,
    displayOff: displayOff.value,
    sleep: sleep.value,
    energy: energy.value,
    bypass: bypass.value,
    proxies,
    ports,
  });
const dirty = computed(
  () => original.value !== "" && original.value !== form(),
);
const formRevision = ref("");
const adoption = ref(0);
watch(
  [
    () => target.value?.id,
    () => snapshot.value?.revision,
    () => adoption.value,
  ],
  ([identity, revision], previous) => {
    if (
      dirty.value &&
      !(
        submitting.value &&
        ["applied", "preview", "restored"].includes(
          response.value?.effect ?? "",
        )
      ) &&
      previous &&
      identity === previous[0] &&
      revision !== previous[1]
    )
      return;
    const t = target.value;
    mode.value = String(value(t, "mode") ?? "");
    const origin = value(t, "origin") as { x?: number; y?: number } | null;
    x.value = String(origin?.x ?? "");
    y.value = String(origin?.y ?? "");
    displayOff.value = String(value(t, "displayOffMinutes") ?? "");
    sleep.value = String(value(t, "sleepMinutes") ?? "");
    energy.value = String(value(t, "mode") ?? "");
    bypass.value = Array.isArray(value(t, "bypass"))
      ? (value(t, "bypass") as string[]).join("\n")
      : "";
    for (const protocol of protocols) {
      const p = value(t, protocol) as ProxyEndpoint | null;
      proxies[protocol] = p ? { ...p } : { enabled: false, host: "", port: 0 };
      ports[protocol] = p?.port ? String(p.port) : "";
    }
    formRevision.value = String(revision ?? "");
    original.value = form();
  },
  { immediate: true },
);
const busy = computed(
  () => controller?.state.busy.has(effective.value) ?? false,
);
const conflict = computed(
  () => !!snapshot.value && formRevision.value !== snapshot.value.revision,
);
const editable = computed(
  () =>
    controller?.available &&
    response.value?.writeAllowed &&
    target.value?.writable &&
    !busy.value &&
    !conflict.value &&
    !!snapshot.value &&
    controller.state.now >= snapshot.value.collectedAtUnixMs &&
    controller.state.now - snapshot.value.collectedAtUnixMs < 30_000,
);
function refresh() {
  void controller?.load(effective.value, true);
  if (props.section === "lock") void controller?.load("lock", true);
}
const login = computed(() =>
  controller?.state.observations
    .get("lock")
    ?.snapshot?.targets.find((t) => t.id === "login"),
);
const error = computed(() => controller?.state.errors.get(effective.value));
const facts = computed(() =>
  [...(controller?.state.facts.values() ?? [])].filter((entry) =>
    controller
      ?.relevant(props.section)
      .some((s) => s.reference.id === entry.source.reference.id),
  ),
);
const labels: Record<string, string> = {
  http: "HTTP 代理",
  https: "HTTPS 代理",
  socks: "SOCKS 代理",
  bypass: "不使用代理的地址",
  pacEnabled: "自动配置脚本",
  wpadEnabled: "自动发现代理",
  enabled: "网络服务启用",
  mode: "当前模式",
  width: "逻辑宽度",
  height: "逻辑高度",
  pixelWidth: "像素宽度",
  pixelHeight: "像素高度",
  origin: "桌面位置",
  main: "主显示器",
  input: "声音输入",
  output: "声音输出",
  volume: "输出音量",
  displayOffMinutes: "关闭屏幕（分钟）",
  sleepMinutes: "系统睡眠（分钟）",
  requireLogin: "返回时要求登录",
  facts: "本机状态",
};
const factLabels: Record<string, string> = {
  id: "标识",
  name: "名称",
  model: "型号",
  manufacturer: "制造商",
  architecture: "架构",
  version: "版本",
  build: "系统构建",
  edition: "系统版本",
  serial: "序列号",
  hardware_uuid: "硬件标识",
  total_bytes: "总内存",
  size_bytes: "总容量",
  free_bytes: "可用空间",
  system_volume: "系统卷",
  container_id: "所属共享容器",
  filesystem: "文件系统",
  addresses: "接口地址",
  charge_percent: "电池电量",
  DeviceID: "卷",
  Size: "总容量",
  FreeSpace: "可用空间",
  DriveType: "驱动器类型",
  presence: "是否存在",
  configured: "配置状态",
  running: "运行状态",
  conversion: "加密转换状态",
  protection: "保护状态",
  lock: "锁定状态",
  algorithm: "加密算法",
  mode: "模式",
  health: "健康状态",
  signature_version: "签名版本",
  system_drive: "系统卷",
  brand: "处理器型号",
  logical_cores: "逻辑核心",
  physical_cores: "物理核心",
  frequency_hz: "频率",
  os_name: "操作系统",
  os_version: "系统版本",
};
const qualityLabels: Record<string, string> = {
  complete: "完整",
  missing: "未发现此项",
  partial: "部分数据",
  truncated: "结果不完整",
  failed: "读取失败",
  notCollected: "尚未采集",
  definitionUnavailable: "来源不可用",
  unsupported: "系统不支持此项读取",
  denied: "读取权限不足",
  expired: "已过期",
  unknown: "未知",
};
function factLabel(key: string) {
  const field = key.split(".").at(-1) ?? key;
  return factLabels[field] ?? field;
}
function factValue(key: string, v: unknown) {
  if (
    (key.endsWith("_bytes") || ["Size", "FreeSpace"].includes(key)) &&
    (typeof v === "number" || typeof v === "string") &&
    Number.isFinite(Number(v))
  )
    return `${(Number(v) / 1024 ** 3).toFixed(1)} GiB`;
  if (key.endsWith("charge_percent") && typeof v === "number")
    return `${v.toFixed(0)}%`;
  return display(v);
}
function sourceTitle(domains: string[]) {
  const names: Record<string, string> = {
    "hardware.os": "操作系统",
    "hardware.cpu": "处理器",
    "hardware.memory": "内存",
    "hardware.identity": "设备标识",
    "hardware.system": "机型",
    "hardware.volumes": "卷容量",
    "hardware.containers": "APFS 共享空间",
    "hardware.disks": "磁盘",
    "hardware.network": "网络接口",
    "hardware.battery": "电池",
    "security.encryption": "设备加密",
    "security.firewall": "防火墙",
    "security.protection": "系统防护",
  };
  return names[domains[0]] ?? "本机状态";
}
function factRows(data: unknown[]) {
  return data.flatMap((row) =>
    row &&
    typeof row === "object" &&
    "values" in row &&
    row.values &&
    typeof row.values === "object" &&
    !Array.isArray(row.values)
      ? [row.values as Record<string, unknown>]
      : [],
  );
}
function display(v: unknown): string {
  if (v === null || v === undefined) return "未知";
  if (typeof v === "boolean") return v ? "已启用" : "已关闭";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
const effectLabels = {
  pending: "已提交，等待执行",
  cancelled: "已取消，尚未开始执行",
  applied: "已回读确认修改",
  preview: "正在预览显示配置",
  restored: "已回读确认恢复",
  conflict: "系统状态与请求不一致",
  failed: "操作失败",
  unknown: "当前效果无法确认",
  openRequested: "已请求打开系统设置",
  observed: "已读取系统状态",
};
const confirmation = ref(false),
  patch = ref<SettingsPatch>();
const remaining = computed(() =>
  Math.max(
    0,
    Math.ceil(
      ((response.value?.keepBeforeUnixMs ?? 0) -
        (controller?.state.now ?? Date.now())) /
        1000,
    ),
  ),
);
function prepare() {
  const t = target.value;
  if (!t || !editable.value || !dirty.value) return;
  const base = (key: string) => value(t, key);
  if (effective.value === "display") {
    const origin = base("origin") as { x: number; y: number };
    if (
      !Number.isInteger(Number(x.value)) ||
      !Number.isInteger(Number(y.value))
    )
      return;
    patch.value = {
      kind: "display",
      target: t.id,
      mode: mode.value !== base("mode") ? mode.value : null,
      origin:
        Number(x.value) !== origin?.x || Number(y.value) !== origin?.y
          ? { x: Number(x.value), y: Number(y.value) }
          : null,
    };
  } else if (effective.value === "network") {
    const changed = (protocol: (typeof protocols)[number]) => {
      const p = { ...proxies[protocol], port: Number(ports[protocol] || 0) };
      return JSON.stringify(p) === JSON.stringify(base(protocol)) ? null : p;
    };
    if (
      protocols.some(
        (p) => !/^\d*$/.test(ports[p]) || Number(ports[p] || 0) > 65535,
      )
    )
      return;
    const entries = bypass.value
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    patch.value = {
      kind: "proxy",
      target: t.id,
      http: changed("http"),
      https: changed("https"),
      socks: changed("socks"),
      bypass:
        JSON.stringify(entries) === JSON.stringify(base("bypass"))
          ? null
          : entries,
    };
  } else if (effective.value === "power") {
    const timer = (v: string) => /^\d+$/.test(v) && Number(v) <= 1440;
    const displayChanged =
      displayOff.value !== String(base("displayOffMinutes") ?? "");
    const sleepChanged = sleep.value !== String(base("sleepMinutes") ?? "");
    if (
      (displayChanged && !timer(displayOff.value)) ||
      (sleepChanged && !timer(sleep.value))
    )
      return;
    patch.value = {
      kind: "power",
      target: t.id,
      displayOffMinutes: displayChanged ? Number(displayOff.value) : null,
      sleepMinutes: sleepChanged ? Number(sleep.value) : null,
      mode: energy.value !== base("mode") && energy.value ? energy.value : null,
    };
  } else return;
  confirmation.value = true;
}
async function submit() {
  if (!patch.value || !editable.value) return;
  confirmation.value = false;
  submitting.value = true;
  try {
    await controller?.save(patch.value, formRevision.value);
  } finally {
    submitting.value = false;
  }
}
</script>
<template>
  <div class="settings-live">
    <div class="setting-actions">
      <UiButton :disabled="!controller?.available || busy" @click="refresh"
        >刷新本机状态</UiButton
      >
      <UiButton
        :disabled="!controller?.available || busy"
        @click="controller?.openNative(section)"
        >打开系统设置</UiButton
      >
      <UiButton
        v-if="controller?.state.pending.has(effective)"
        @click="controller?.recover(effective)"
        >查询原任务</UiButton
      >
      <UiButton
        v-if="!busy && controller?.state.pending.has(effective)"
        @click="controller?.retry(effective)"
        >重试原提交</UiButton
      >
      <UiButton
        v-if="controller?.state.pending.has(effective)"
        @click="controller?.cancel(effective)"
        >取消操作</UiButton
      >
    </div>
    <p v-if="!controller?.available" role="status">
      本机服务不可用，尚未读取系统状态。
    </p>
    <p v-if="error" role="alert">{{ error }}</p>
    <UiButton
      v-if="conflict"
      @click="
        original = '';
        adoption++;
      "
      >放弃草稿，采用最新状态</UiButton
    >
    <p v-if="response?.request || response?.reason" role="status">
      {{ response ? effectLabels[response.effect] : ""
      }}<span v-if="response.reason"> · {{ response.reason }}</span>
    </p>
    <p v-if="snapshot" class="observation-time">
      {{ snapshot.source }} ·
      {{ new Date(snapshot.collectedAtUnixMs).toLocaleString()
      }}<span
        v-if="
          (controller?.state.now ?? 0) - snapshot.collectedAtUnixMs > 30_000
        "
      >
        · 状态已过期，请刷新后修改</span
      >
    </p>
    <div
      v-if="response?.effect === 'preview'"
      class="display-preview"
      role="alert"
    >
      <strong>保留此显示配置？{{ remaining }} 秒后自动恢复。</strong>
      <UiButton :disabled="remaining === 0" @click="controller?.keep(effective)"
        >保留配置</UiButton
      >
      <UiButton @click="controller?.cancel(effective)">恢复原配置</UiButton>
    </div>
    <label v-if="targets.length > 1" class="setting-select"
      >选择{{
        effective === "network"
          ? "网络服务"
          : effective === "display"
            ? "显示器"
            : "供电方式"
      }}<UiSelect
        v-model="selected"
        :options="targets.map((t) => ({ value: t.id, label: t.name }))"
    /></label>
    <section v-if="section === 'lock'">
      <h3>返回电脑时</h3>
      <div class="design-rows">
        <div class="design-row">
          <strong>要求登录</strong
          ><span>{{ display(value(login, "requireLogin")) }}</span>
        </div>
      </div>
      <p class="design-note">
        {{
          login?.fields.find((f) => f.key === "requireLogin")?.reason ??
          "登录要求独立于屏幕关闭和睡眠设置"
        }}
      </p>
    </section>
    <section v-if="target">
      <h3>{{ target.name }}</h3>
      <dl class="design-rows">
        <div
          v-for="field in target.fields"
          :key="field.key"
          class="design-row setting-row"
        >
          <dt>{{ labels[field.key] ?? field.key }}</dt>
          <dd>
            {{ display(field.value)
            }}<small v-if="field.quality !== 'complete'">
              · {{ field.reason ?? field.quality }}</small
            >
          </dd>
        </div>
      </dl>
      <p v-if="target.reason" class="design-note">{{ target.reason }}</p>
      <form
        v-if="
          ['network', 'display', 'power'].includes(effective) && target.writable
        "
        @submit.prevent="prepare"
      >
        <fieldset :disabled="!editable">
          <legend>修改{{ target.name }}的偏好</legend>
          <template v-if="effective === 'network'">
            <div
              v-for="protocol in protocols"
              :key="protocol"
              class="proxy-controls"
            >
              <label
                >{{ labels[protocol]
                }}<UiSwitch
                  v-model="proxies[protocol].enabled"
                  :aria-label="labels[protocol]" /></label
              ><label
                >服务器<UiTextInput
                  v-model="proxies[protocol].host"
                  :aria-label="`${protocol}服务器`" /></label
              ><label
                >端口<UiTextInput
                  v-model="ports[protocol]"
                  inputmode="numeric"
                  :aria-label="`${protocol}端口`"
              /></label>
            </div>
            <label
              >绕过代理的地址（每行一个）<UiTextInput
                v-model="bypass"
                multiline
            /></label>
            <p class="design-note">
              自动配置脚本、自动发现及未修改的协议保持原值。
            </p>
          </template>
          <template v-else-if="effective === 'display'">
            <label
              >分辨率与缩放模式<UiSelect
                v-model="mode"
                :options="
                  target.options.map((o) => ({ value: o.id, label: o.label }))
                "
            /></label>
            <p class="design-note">
              仅显示系统支持的模式；HiDPI 的逻辑尺寸与像素尺寸分别标示。
            </p>
            <label
              >桌面横坐标<UiTextInput v-model="x" inputmode="numeric" /></label
            ><label
              >桌面纵坐标<UiTextInput v-model="y" inputmode="numeric"
            /></label>
          </template>
          <template v-else>
            <label
              >闲置后关闭屏幕（分钟，0 为从不）<UiTextInput
                v-model="displayOff"
                inputmode="numeric" /></label
            ><label
              >闲置后系统睡眠（分钟，0 为从不）<UiTextInput
                v-model="sleep"
                inputmode="numeric" /></label
            ><label v-if="target.options.length"
              >电源模式<UiSelect
                v-model="energy"
                :options="
                  target.options.map((o) => ({ value: o.id, label: o.label }))
                "
            /></label>
          </template>
        </fieldset>
        <p v-if="conflict" role="alert">
          保留了未保存的修改，但系统状态已经变化。请重新选择目标并确认新状态。
        </p>
        <UiButton type="submit" :disabled="!editable || !dirty"
          >检查并应用修改</UiButton
        >
      </form>
    </section>
    <section
      v-for="entry in facts"
      :key="entry.source.reference.id"
      class="collected-facts"
    >
      <h3>{{ sourceTitle(entry.source.domains) }}</h3>
      <p class="observation-time">
        {{
          entry.value.collectedAt
            ? new Date(entry.value.collectedAt * 1000).toLocaleString()
            : "尚未采集"
        }}
        · {{ entry.value.quality
        }}<span v-if="entry.value.stale"> · 已过期</span
        ><span v-if="entry.value.next || !entry.value.enumerationComplete">
          · 枚举不完整</span
        >
      </p>
      <dl class="design-rows">
        <div
          v-for="(v, key) in entry.value.fields"
          :key="key"
          class="design-row"
        >
          <dt>{{ factLabel(String(key)) }}</dt>
          <dd>{{ factValue(String(key), v) }}</dd>
        </div>
      </dl>
      <dl
        v-for="(row, index) in factRows(entry.value.rows)"
        :key="index"
        class="design-rows fact-record"
      >
        <div v-for="(v, key) in row" :key="key" class="design-row">
          <dt>{{ factLabel(String(key)) }}</dt>
          <dd>{{ factValue(String(key), v) }}</dd>
        </div>
      </dl>
    </section>
    <p v-if="section === 'security'" class="design-note">
      应用权限需要在系统设置中逐项核对。
    </p>
    <UiDialog
      v-model:open="confirmation"
      title="确认系统设置修改"
      :description="`${target?.name ?? ''}：${effective === 'display' ? '先预览 15 秒，必须明确选择保留。' : '将修改所选目标，并重新读取实际值确认结果。'}`"
      ><p v-if="patch?.kind === 'power'">
        关闭屏幕与系统睡眠分别应用；不会修改返回时的登录要求。
      </p>
      <p v-if="patch?.kind === 'proxy'">仅修改已编辑的手动代理和绕过列表。</p>
      <UiButton @click="submit">确认修改</UiButton
      ><UiButton @click="confirmation = false">返回编辑</UiButton></UiDialog
    >
  </div>
</template>
<style scoped>
.setting-actions,
.display-preview {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  align-items: center;
  margin: 18px 0;
}
.setting-select,
fieldset > label {
  display: grid;
  gap: 8px;
  margin: 16px 0;
}
fieldset {
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  padding: 18px;
  margin: 20px 0;
}
legend {
  padding: 0 8px;
}
.proxy-controls {
  display: grid;
  grid-template-columns: 140px 1fr 100px;
  gap: 12px;
  margin: 12px 0;
}
.proxy-controls label {
  display: grid;
  gap: 8px;
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
  max-width: 65%;
  text-align: right;
}
.observation-time {
  color: var(--rss-color-text-muted);
  font-size: 12px;
}
.display-preview {
  padding: 18px;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
}
pre {
  overflow: auto;
  max-height: 360px;
  font-size: 12px;
}
.collected-facts {
  margin-top: 24px;
}
</style>
