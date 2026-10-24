<script setup lang="ts">
import { watch, onBeforeUnmount } from "vue";
import { currentUser } from "../test-users";
import { createDiagnostics } from "./diagnostics";
import { nativeDiagnostics, type DiagnosticPort } from "./native";
const props = withDefaults(
  defineProps<{ active?: boolean; port?: DiagnosticPort | null }>(),
  { active: true },
);
const c = createDiagnostics(
    props.port === undefined ? nativeDiagnostics() : (props.port ?? undefined),
  ),
  s = c.state;
const labels = {
  host: "AI Host 状态",
  service: "执行服务状态",
  device: "设备平台与注册摘要",
  execution: "授权任务状态摘要",
};
watch(
  () => currentUser.value?.generation,
  () => c.clear(),
);
watch(
  () => props.active,
  (active) => {
    if (!active) c.clear();
  },
);
onBeforeUnmount(() => c.dispose());
</script>
<template>
  <section aria-label="诊断范围与预览">
    <h3>诊断范围</h3>
    <label v-for="(label, key) in labels" :key="key"
      ><input
        v-model="s.scopes[key]"
        type="checkbox"
        :disabled="s.busy"
        @change="c.clear"
      />{{ label }}</label
    >
    <p>
      仅包含所选范围的脱敏摘要；不包含密钥、对话、端点、个人路径、脚本参数或原始输出。
    </p>
    <button
      :disabled="
        !c.available || s.busy || !Object.values(s.scopes).some(Boolean)
      "
      @click="c.prepare"
    >
      预览诊断
    </button>
    <p v-if="!c.available">诊断导出需要桌面原生服务。</p>
    <section v-if="s.preview" aria-label="导出内容预览">
      <p>{{ s.preview.bytes }} 字节 · 文件将保存以下内容</p>
      <pre>{{ JSON.stringify(s.preview.content, null, 2) }}</pre>
      <button :disabled="s.busy" @click="c.save">选择位置并导出</button>
      <button :disabled="s.busy" @click="c.clear">关闭预览</button>
    </section>
    <p v-if="s.busy" role="status">正在处理…</p>
    <p v-if="s.message" role="status">{{ s.message }}</p>
  </section>
</template>
