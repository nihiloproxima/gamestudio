<script setup lang="ts">
import type { ItemStatus } from "../domain/types";

/** Linear-style progress rings: the lifecycle reads from the shape, not only the color. */
defineProps<{ status: ItemStatus; size?: number }>();
</script>

<template>
  <svg :width="size ?? 14" :height="size ?? 14" viewBox="0 0 14 14" class="status" :style="{ color: `var(--st-${status})` }">
    <circle v-if="status === 'inbox'" cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-dasharray="1.6 1.9" />
    <circle v-else-if="status === 'backlog'" cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-dasharray="4 2.2" />
    <circle v-else-if="status === 'todo'" cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" stroke-width="1.5" />
    <template v-else-if="status === 'doing' || status === 'review'">
      <circle cx="7" cy="7" r="5.5" fill="none" stroke="currentColor" stroke-width="1.5" />
      <path :d="status === 'doing' ? 'M7 3.5 A3.5 3.5 0 0 1 7 10.5 Z' : 'M7 3.5 A3.5 3.5 0 1 1 3.5 7 L7 7 Z'" fill="currentColor" />
    </template>
    <template v-else-if="status === 'done'">
      <circle cx="7" cy="7" r="6.25" fill="currentColor" />
      <path d="M4.3 7.2 L6.2 9 L9.8 5.2" fill="none" stroke="var(--crust)" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
    </template>
    <template v-else>
      <circle cx="7" cy="7" r="6.25" fill="currentColor" />
      <path d="M4.8 4.8 L9.2 9.2 M9.2 4.8 L4.8 9.2" stroke="var(--crust)" stroke-width="1.5" stroke-linecap="round" />
    </template>
  </svg>
</template>

<style scoped>
.status {
  flex: none;
  display: block;
}
</style>
