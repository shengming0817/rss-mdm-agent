<script setup lang="ts">
import { UserRound } from "@rss-mdm-agent/ui";
defineProps<{ expanded: boolean; active?: boolean }>();
const emit = defineEmits<{ open: [] }>();
</script>
<template>
  <button
    class="workspace-account"
    type="button"
    title="RSS Agent 设置"
    aria-label="RSS Agent 设置"
    :aria-expanded="expanded"
    :aria-current="active ? 'page' : undefined"
    @click="emit('open')"
  >
    <span class="account-avatar"
      ><UserRound :size="18" aria-hidden="true"
    /></span>
    <span class="account-label"
      ><strong>RSS Agent 设置</strong
      ><small><slot>账户与连接</slot></small></span
    >
  </button>
</template>
<style scoped>
.workspace-account {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 10px 12px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--rss-color-text);
  text-align: left;
  cursor: pointer;
}
.workspace-account:hover,
.workspace-account[aria-expanded="true"] {
  background: var(--rss-color-neutral-bg);
}
.account-avatar {
  display: grid;
  place-items: center;
  flex: none;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  color: var(--rss-color-text-muted);
  background: var(--rss-color-neutral-bg);
}
.workspace-account[aria-expanded="true"] .account-avatar {
  color: var(--rss-color-accent);
  background: var(--rss-color-accent-bg);
}
.account-label {
  min-width: 0;
}
.account-label > strong {
  font-size: 12px;
  font-weight: 550;
}
.account-label > small {
  display: block;
  color: var(--rss-color-text-muted);
  font-size: 10px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 150px;
}
.account-label small :deep(small) {
  display: none;
}
.account-label small :deep(strong) {
  font-weight: 400;
}
</style>
