<script setup lang="ts">
defineProps<{ kind: string; size: number }>();
</script>
<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="kind === 'office'">
      <rect x="3" y="7" width="18" height="14" rx="2" />
      <path
        d="M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M3 12a20 20 0 0 0 18 0M10 13h4"
      />
    </template>
    <template v-else-if="kind === 'design'">
      <path d="m15 4 5 5M4 20l5-1 12-12a2.1 2.1 0 0 0-5-5L4 14z" />
    </template>
    <template v-else-if="kind === 'code'">
      <path d="m8 5-7 7 7 7m8-14 7 7-7 7M14 3l-4 18" />
    </template>
    <template v-else-if="kind === 'history'">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 6v6l4 2" />
    </template>
    <template v-else-if="kind === 'logout'">
      <path d="M9 4H4v16h5m5-12 4 4-4 4m-6-4h10" />
    </template>
    <template v-else-if="kind === 'tools'">
      <path
        d="m14 6 4-4a7 7 0 0 0-8 9L3 18a2.1 2.1 0 0 0 3 3l7-7a7 7 0 0 0 9-8l-4 4-4-4z"
      />
    </template>
    <template v-else-if="['network', '连接与网络'].includes(kind)">
      <path d="M3 9a14 14 0 0 1 18 0M6 12a9 9 0 0 1 12 0M9 15a4 4 0 0 1 6 0" />
      <circle cx="12" cy="19" r=".7" fill="currentColor" />
    </template>
    <template v-else-if="['display', 'lock'].includes(kind)">
      <rect x="3" y="4" width="18" height="13" rx="2" />
      <path d="M8 21h8m-4-4v4" />
    </template>
    <template v-else-if="kind === 'sound'">
      <path
        d="m4 9 4 0 5-4v14l-5-4H4zM17 8a7 7 0 0 1 0 8m3-11a11 11 0 0 1 0 14"
      />
    </template>
    <template v-else-if="kind === 'power'">
      <rect x="2" y="6" width="18" height="12" rx="3" />
      <path d="M23 10v4M6 10v4m4-4v4m4-4v4" />
    </template>
    <template v-else-if="kind === 'storage'">
      <path d="m5 4-3 12v4h20v-4L19 4zM2 16h20M6 19h.01M10 19h.01" />
    </template>
    <template
      v-else-if="
        ['security', 'privacy', 'support', '支持与诊断'].includes(kind)
      "
    >
      <path d="m12 3 9 4v6c0 5-9 9-9 9S3 18 3 13V7zM8 12l3 3 5-6" />
    </template>
    <template v-else-if="kind === 'appearance'">
      <path
        d="M12 3a9 9 0 1 0 0 18h1a2 2 0 0 0 1-3.7 1.7 1.7 0 0 1 1-3.3h2a4 4 0 0 0 4-4c0-4-4-7-9-7z"
      />
      <circle cx="7" cy="10" r=".7" />
      <circle cx="10" cy="6.5" r=".7" />
      <circle cx="15" cy="7" r=".7" />
    </template>
    <template v-else-if="kind === 'connection'">
      <path d="m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3z" />
    </template>
    <template v-else-if="kind === 'notifications'">
      <path d="M5 10a7 7 0 0 1 14 0v5l2 3H3l2-3zM9 21h6" />
    </template>
    <template v-else-if="kind === 'updates'">
      <path
        d="M20 8a8 8 0 0 0-14-3L3 8m0-5v5h5M4 16a8 8 0 0 0 14 3l3-3m0 5v-5h-5"
      />
    </template>
    <template v-else-if="kind === 'discover'">
      <circle cx="12" cy="12" r="9" />
      <path d="m16 8-2.5 5.5L8 16l2.5-5.5z" />
    </template>
    <template v-else-if="kind === 'about'">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v6m0-10v.01" />
    </template>
    <template v-else-if="kind === 'installed'">
      <path d="M5 3h14v18H5zM8 8h8m-8 4h8m-8 4h5" />
    </template>
    <template v-else-if="kind === 'account'">
      <circle cx="12" cy="7" r="4" />
      <path d="M4 22v-3a8 8 0 0 1 16 0v3" />
    </template>
    <template v-else-if="['all', 'overview', '全部'].includes(kind)">
      <rect x="3" y="3" width="7" height="7" rx="1.5" />
      <rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="3" y="14" width="7" height="7" rx="1.5" />
      <rect x="14" y="14" width="7" height="7" rx="1.5" />
    </template>
    <template v-else>
      <path d="M3 6h18M3 12h18M3 18h18" />
      <circle cx="8" cy="6" r="2" fill="var(--rss-color-bg)" />
      <circle cx="16" cy="12" r="2" fill="var(--rss-color-bg)" />
      <circle cx="9" cy="18" r="2" fill="var(--rss-color-bg)" />
    </template>
  </svg>
</template>
