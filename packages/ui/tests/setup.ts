import { config } from "@vue/test-utils";
import { ref } from "vue";
import { uiContext } from "../src/context";
import { uiContext as builtContext } from "@rss-mdm-agent/ui";
const context = {
  configuration: {
    platform: "macos" as const,
    theme: "system" as const,
    reducedMotion: false,
    highContrast: false,
  },
  portal: ref(document.body),
};
config.global.provide = {
  [uiContext as symbol]: context,
  [builtContext as symbol]: context,
};
