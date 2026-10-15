<script setup lang="ts">
import { computed, provide, ref } from "vue";
import { ConfigProvider } from "reka-ui";
import { uiContext, type UiPlatform, type UiTheme } from "../context";
const props = withDefaults(defineProps<{
  platform: UiPlatform;
  theme?: UiTheme;
  reducedMotion?: boolean;
  highContrast?: boolean;
}>(), { theme: "system", reducedMotion: false, highContrast: false });
const portal = ref<HTMLElement>();
provide(uiContext, { configuration: props, portal });
const attributes = computed(() => ({
  "data-platform": props.platform,
  "data-theme": props.theme,
  "data-reduced-motion": props.reducedMotion || undefined,
  "data-high-contrast": props.highContrast || undefined,
}));
// ref: reka-ui v2.10.5 ConfigProvider/ConfigProvider.vue — scoped teleport target.
</script>
<template>
  <div class="rss-ui ui-provider" v-bind="attributes">
    <ConfigProvider locale="zh-CN" :teleport-to="portal">
      <slot />
      <div ref="portal" class="ui-overlays" />
    </ConfigProvider>
  </div>
</template>
