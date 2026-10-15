<script setup lang="ts">
import {
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger,
  TooltipPortal,
  TooltipContent,
} from "reka-ui";
import { useUiContext } from "../context";
defineProps<{ text: string }>();
const { portal } = useUiContext();
</script>
<template>
  <TooltipProvider :delay-duration="300"
    ><TooltipRoot
      ><TooltipTrigger as-child><slot /></TooltipTrigger
      ><TooltipPortal v-if="portal"
        ><TooltipContent class="ui-popup ui-tooltip" :side-offset="4">{{
          text
        }}</TooltipContent></TooltipPortal
      ></TooltipRoot
    ></TooltipProvider
  >
</template>
