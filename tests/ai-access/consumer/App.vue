<script setup>
import { ref } from "vue";
import {
  RuntimeSurface,
  createSurfaceRenderer,
} from "@rss-mdm-agent/ai-ui-bridge";
const props = defineProps({ runtime: { type: Object, required: true } });
const mounted = ref(true),
  receipts = ref(0),
  errors = ref([]),
  cardVersion = ref(0);
let failLoad = 0,
  failReplace = 0;
const loseResponse = ref(false),
  attempts = ref([]);
const originalAction = props.runtime.action.bind(props.runtime);
props.runtime.action = async (request) => {
  attempts.value.push(request.metadata.commandId);
  const receipt = await originalAction(request);
  if (loseResponse.value) {
    loseResponse.value = false;
    throw new Error("injected response loss after commit");
  }
  return receipt;
};
const factory = async (container, options) => {
  if (failLoad > 0) {
    failLoad--;
    throw new Error("injected module load failure");
  }
  const renderer = await createSurfaceRenderer(container, options);
  return {
    replace(...args) {
      if (failReplace > 0) {
        failReplace--;
        throw new Error("injected renderer failure");
      }
      renderer.replace(...args);
    },
    clear: () => renderer.clear(),
    dispose: () => renderer.dispose(),
    retry: () => renderer.retry(),
  };
};
</script>
<template>
  <button id="toggle" @click="mounted = !mounted">Toggle card</button>
  <button
    id="fail-renderer"
    @click="
      failLoad++;
      cardVersion++;
    "
  >
    Fail renderer load once
  </button>
  <button
    id="fail-replace"
    @click="
      failReplace++;
      cardVersion++;
    "
  >
    Fail renderer replace once
  </button>
  <button id="lose-response" @click="loseResponse = true">
    Lose next response
  </button>
  <output id="receipts">{{ receipts }}</output>
  <output id="attempts">{{ attempts.join(",") }}</output>
  <output id="errors">{{ errors.join("; ") }}</output>
  <RuntimeSurface
    v-if="mounted"
    :key="cardVersion"
    :renderer-factory="factory"
    :runtime="runtime"
    session-id="session-1"
    instance-id="surface-instance-1"
    @receipt="receipts++"
    @error="errors.push($event.message)"
  />
</template>
