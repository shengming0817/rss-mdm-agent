<script setup lang="ts">
import { onMounted, ref, useId } from "vue";
import SettingsGlyph from "../system-settings/SettingsGlyph.vue";

import {
  currentUser,
  nativeTestMode,
  loadAccountSettings,
  accountErrorMessage,
  saveOrganization,
  type Organization,
} from "../test-users";
const props = defineProps<{
  loading: boolean;
  message?: string;
  registration?: boolean;
}>();
const emit = defineEmits<{
  guest: [];
  logout: [];
  login: [organizationId: string, login: string];
}>();

const fieldId = useId();
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
    const settings = await loadAccountSettings();
    organizations.value = settings.organizations;
    selected.value = settings.selected ?? "";
  } catch {
    message.value = "组织配置不可用，请重试";
  }
}
async function save() {
  if (props.loading) return;
  try {
    const saved = await saveOrganization({
      id: "",
      label: label.value,
      origin: origin.value,
      tenantId: tenantId.value,
    });
    organizations.value = [
      ...organizations.value.filter((o) => o.id !== saved.id),
      saved,
    ];
    selected.value = saved.id;
    message.value = "组织连接已保存";
  } catch (error) {
    message.value = accountErrorMessage(error);
  }
}
onMounted(load);
</script>
<template>
  <section aria-label="企业账户" class="account">
    <header class="account-card-heading">
      <span class="account-card-icon"
        ><SettingsGlyph kind="office" :size="21"
      /></span>
      <div>
        <h3>{{ registration ? "登录并注册此设备" : "企业登录" }}</h3>
        <p>连接你的组织，进入工作空间。</p>
      </div>
    </header>
    <p v-if="props.message" class="account-feedback" role="alert">
      {{ props.message }}
    </p>
    <p
      v-if="currentUser?.identity?.mode === 'user_device'"
      class="account-feedback"
    >
      当前组织：{{ currentUser.user.displayName }} ·
      {{ currentUser.identity.principalId }}
    </p>
    <p
      v-else-if="currentUser?.identity?.mode === 'device'"
      class="account-feedback"
    >
      当前使用本机设备工作区，可访问设备获授权的自助服务。
    </p>
    <form @submit.prevent="emit('login', selected, login)">
      <div class="account-field">
        <label :for="`account-organization-${fieldId}`">组织连接</label
        ><select
          :id="`account-organization-${fieldId}`"
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
      </div>
      <div class="account-field">
        <label :for="`account-login-${fieldId}`">企业账号</label
        ><input
          :id="`account-login-${fieldId}`"
          v-model="login"
          aria-label="企业账号"
          autocomplete="username"
          placeholder="输入企业账号"
          :disabled="loading || !nativeTestMode"
          maxlength="256"
        />
      </div>
      <p
        v-if="selected"
        class="account-connection-meta"
        aria-label="当前组织连接"
      >
        {{ organizations.find((item) => item.id === selected)?.origin }} ·
        {{ organizations.find((item) => item.id === selected)?.tenantId }}
      </p>
      <p class="account-form-help">
        使用组织的本地账号登录。切换组织需重新登录；密码在系统原生输入框填写。
      </p>
      <button
        class="account-primary"
        :disabled="loading || !nativeTestMode || !selected || !login.trim()"
      >
        {{ registration ? "登录并注册此设备" : "登录所选组织" }}
      </button>
    </form>
    <details class="organization-details">
      <summary>添加组织连接</summary>
      <p :id="`organization-help-${fieldId}`">
        服务地址只填写 HTTPS origin，不含路径或查询；租户填写管理员提供的 UUID。
      </p>
      <form @submit.prevent="save">
        <div class="account-field">
          <label :for="`organization-label-${fieldId}`">组织名称</label
          ><input
            :id="`organization-label-${fieldId}`"
            v-model="label"
            aria-label="组织名称"
            placeholder="组织名称"
            maxlength="64"
          />
        </div>
        <div class="account-field">
          <label :for="`organization-origin-${fieldId}`">组织服务地址</label
          ><input
            :id="`organization-origin-${fieldId}`"
            :aria-describedby="`organization-help-${fieldId}`"
            v-model="origin"
            aria-label="组织服务地址"
            placeholder="https://mdm.example"
          />
        </div>
        <div class="account-field account-field-wide">
          <label :for="`organization-tenant-${fieldId}`">租户 UUID</label
          ><input
            :id="`organization-tenant-${fieldId}`"
            :aria-describedby="`organization-help-${fieldId}`"
            v-model="tenantId"
            aria-label="租户 UUID"
            placeholder="租户 UUID"
          />
        </div>
        <button class="account-primary" :disabled="loading || !nativeTestMode">
          保存组织连接
        </button>
      </form>
    </details>
    <div v-if="!registration" class="account-session-actions">
      <button :disabled="loading || !nativeTestMode" @click="emit('guest')">
        不登录使用</button
      ><button v-if="currentUser" :disabled="loading" @click="emit('logout')">
        退出当前账户
      </button>
    </div>
    <p class="account-data-note">
      访客与测试用户使用独立本地数据。企业账号不自动认领这些数据。
    </p>
    <p v-if="message" class="account-feedback" role="status">{{ message }}</p>
  </section>
</template>
<style scoped>
.account {
  padding: 24px;
  border: 1px solid var(--rss-color-border);
  border-radius: 12px;
  background: var(--rss-color-surface);
  min-width: 0;
}
.account-card-heading {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 24px;
}
.account-card-icon {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex: none;
  color: var(--rss-color-accent);
  background: var(--rss-color-accent-bg);
  border-radius: 11px;
}
h3 {
  font-size: 15px;
  margin: 0 0 6px;
  font-weight: 600;
}
p {
  font-size: 12px;
  line-height: 1.8;
  color: var(--rss-color-text-muted);
  margin: 0;
  overflow-wrap: anywhere;
}
form {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 18px 16px;
}
.account-field {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 9px;
}
label {
  font-size: 12px;
  font-weight: 500;
  color: var(--rss-color-text);
}
input,
select {
  width: 100%;
  min-width: 0;
  max-width: 100%;
  min-height: 40px;
  padding: 10px 12px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  background: var(--rss-color-bg);
  color: var(--rss-color-text);
  font: inherit;
  font-size: 13px;
}
input:focus-visible,
select:focus-visible,
button:focus-visible,
summary:focus-visible {
  outline: 2px solid var(--rss-color-accent);
  outline-offset: 3px;
}
.account-field-wide,
.account-form-help,
.account-connection-meta {
  grid-column: 1/-1;
}
button {
  justify-self: start;
  min-height: 36px;
  border: 1px solid var(--rss-color-border);
  border-radius: 8px;
  padding: 9px 15px;
  background: var(--rss-color-surface);
  color: var(--rss-color-text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
button:hover:enabled {
  background: var(--rss-color-neutral-bg);
}
button.account-primary {
  grid-column: 1/-1;
  color: var(--rss-color-surface);
  background: var(--rss-color-accent);
  border-color: transparent;
}
button:disabled,
input:disabled,
select:disabled {
  opacity: 0.55;
  cursor: default;
}
.organization-details {
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid var(--rss-color-border);
}
summary {
  cursor: pointer;
  color: var(--rss-color-text);
  font-size: 12px;
  font-weight: 500;
}
.organization-details > p {
  margin: 16px 0;
}
.account-session-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 24px;
}
.account-data-note {
  margin-top: 16px;
  font-size: 11px;
}
.account-feedback {
  padding: 12px;
  border-radius: 8px;
  background: var(--rss-color-neutral-bg);
  color: var(--rss-color-text);
  margin-bottom: 18px;
}
@media (max-width: 600px) {
  .account {
    padding: 18px;
  }
  form {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
