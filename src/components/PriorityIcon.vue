<script setup lang="ts">
import type { Priority } from "../domain/types";

defineProps<{ priority: Priority }>();
</script>

<template>
  <svg width="14" height="14" viewBox="0 0 14 14" class="prio" :class="priority" :aria-label="priority">
    <template v-if="priority === 'urgent'">
      <rect x="1" y="1" width="12" height="12" rx="3" fill="currentColor" />
      <rect x="6.2" y="3.5" width="1.6" height="4.6" rx="0.8" fill="var(--crust)" />
      <circle cx="7" cy="10" r="0.95" fill="var(--crust)" />
    </template>
    <template v-else>
      <rect x="1.5" y="8" width="2.6" height="4" rx="0.8" fill="currentColor" />
      <rect x="5.7" y="5" width="2.6" height="7" rx="0.8" :fill="priority === 'low' ? 'var(--surface-1)' : 'currentColor'" />
      <rect x="9.9" y="2" width="2.6" height="10" rx="0.8" :fill="priority === 'high' ? 'currentColor' : 'var(--surface-1)'" />
    </template>
  </svg>
</template>

<style scoped>
.prio {
  flex: none;
  display: block;
  color: var(--subtext);
}
.urgent {
  color: var(--pr-urgent);
}
.high {
  color: var(--pr-high);
}
.low {
  color: var(--overlay);
}
</style>
