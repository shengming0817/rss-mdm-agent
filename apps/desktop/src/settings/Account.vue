<script setup lang="ts">
import { onMounted, ref } from "vue";

import {
  currentUser,
  nativeTestMode,
  loadOrganizations,
  saveOrganization,
  type Organization,
} from "../test-users";
const props = defineProps<{ loading: boolean }>();
const emit = defineEmits<{
  guest: [];
  logout: [];
  login: [organizationId: string, login: string];
}>();

const organizations = ref<Organization[]>([]),
  selected = ref(""),
  login = ref("");
const label = ref(""),
  origin = ref(""),
  tenantId = ref(""),
  message = ref("");
async function load() {
  if (!nativeTestMode) return;
  try {
    organizations.value = await loadOrganizations();
  } catch {
    message.value = "组织配置不可用，请重试";
  }
}
async function save() {
  if (props.loading) return;
  try {
    organizations.value = await saveOrganization({
      id: "",
      label: label.value,
      origin: origin.value,
      tenantId: tenantId.value,
    });
    selected.value =
      organizations.value.find(
        (o) => o.tenantId === tenantId.value && o.label === label.value,
      )?.id ?? "";
    message.value = "组织连接已保存";
  } catch {
    message.value = "无法保存组织连接，请检查 HTTPS 地址、租户 UUID 和本地存储";
  }
}
onMounted(load);
</script>
<template>
  <section aria-label="企业账户" class="account">
    <h3>企业登录</h3>
    <p v-if="currentUser?.identity?.mode === 'enterprise'">
      当前组织：{{ currentUser.user.displayName }} ·
      {{ currentUser.identity.principalId }}
    </p>
    <p v-else-if="currentUser?.identity?.mode === 'guest'">
      当前为本地访客，非企业身份。
    </p>
    <p>
      使用组织的本地账号登录。切换组织需重新登录；密码在系统原生输入框填写。
    </p>
    <form @submit.prevent="emit('login', selected, login)">
      <select
        v-model="selected"
        aria-label="组织连接"
        :disabled="loading || !nativeTestMode"
      >
        <option value="" disabled>选择组织</option>
        <option
          v-for="organization in organizations"
          :key="organization.id"
          :value="organization.id"
        >
          {{ organization.label }} — {{ organization.origin }}
        </option>
      </select>
      <input
        v-model="login"
        aria-label="企业账号"
        autocomplete="username"
        :disabled="loading || !nativeTestMode"
        maxlength="256"
      />
      <button
        :disabled="loading || !nativeTestMode || !selected || !login.trim()"
      >
        登录所选组织
      </button>
    </form>
    <details>
      <summary>添加组织连接</summary>
      <form @submit.prevent="save">
        <input
          v-model="label"
          aria-label="组织名称"
          placeholder="组织名称"
          maxlength="64"
        />
        <input
          v-model="origin"
          aria-label="组织服务地址"
          placeholder="https://mdm.example.com"
        />
        <input
          v-model="tenantId"
          aria-label="租户 UUID"
          placeholder="租户 UUID"
        />
        <button :disabled="loading || !nativeTestMode">保存组织连接</button>
      </form>
    </details>
    <button :disabled="loading || !nativeTestMode" @click="emit('guest')">
      不登录使用
    </button>
    <button v-if="currentUser" :disabled="loading" @click="emit('logout')">
      退出当前账户
    </button>
    <p>访客与测试用户使用独立本地数据。企业账号不自动认领这些数据。</p>
    <p v-if="message" role="status">{{ message }}</p>
  </section>
</template>
<style scoped>
.account {
  display: grid;
  gap: 12px;
}
form {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
input,
select {
  min-width: 0;
  max-width: 100%;
  padding: 8px;
}
</style>
