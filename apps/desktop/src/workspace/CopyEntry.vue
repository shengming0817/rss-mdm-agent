<script setup lang="ts">
import { ref } from "vue";
import type { EntrySelector } from "../self-service/types";
import { copyText } from "../assistant/clipboard";
import { nativeNavigation } from "./entry";
const props = defineProps<{ selector: EntrySelector }>();
const port = nativeNavigation();
const message = ref("");
async function copy(cli: boolean) {
  try {
    const result = await port?.({ kind: "link", selector: props.selector });
    if (result?.kind !== "uri") throw Error("unavailable");
    message.value = (await copyText(
      cli
        ? `rssmdm open '${result.value.replaceAll("'", "%27")}'`
        : result.value,
    ))
      ? "已复制"
      : "无法写入剪贴板";
  } catch {
    message.value = "当前目标无法生成链接";
  }
}
</script>
<template>
  <span v-if="port" class="entry-copy">
    <button type="button" @click="copy(false)">复制链接</button>
    <button type="button" @click="copy(true)">复制 CLI</button>
    <span role="status">{{ message }}</span>
  </span>
</template>
