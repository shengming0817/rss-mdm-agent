<script setup>
import { ref } from "vue";
import { RuntimeSurface } from "@rss-mdm-agent/ai-ui-bridge";
defineProps({ runtime: { type: Object, required: true } });
const mounted = ref(true);
const receipts = ref(0);
const errors = ref([]);
const failRenderer = ref(false);
const unavailable = async () => {
  throw new Error("injected module load failure");
};
</script>
<template>
  <button id="toggle" @click="mounted = !mounted">Toggle card</button>
  <button id="fail-renderer" @click="failRenderer = !failRenderer">
    Toggle renderer failure
  </button>
  <output id="receipts">{{ receipts }}</output>
  <output id="errors">{{ errors.join("; ") }}</output>
  <RuntimeSurface
    v-if="mounted"
    :key="String(failRenderer)"
    :renderer-factory="failRenderer ? unavailable : undefined"
    :runtime="runtime"
    session-id="session-1"
    instance-id="surface-instance-1"
    @receipt="receipts++"
    @error="errors.push($event.message)"
  />
</template>
