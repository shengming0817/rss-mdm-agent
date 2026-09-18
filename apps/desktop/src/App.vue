<script setup lang="ts">
import SelfService from "./self-service/SelfService.vue";
import { createController } from "./self-service/controller";
import { nativePort, newIdentity } from "./self-service/native";
import preview from "./self-service/preview";
import "./self-service/style.css";
const controller = createController(nativePort(), newIdentity, preview);
</script>
<template><SelfService :controller="controller" /></template>
