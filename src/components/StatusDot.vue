<script setup lang="ts">
import type { AgentStatus } from "../domain/types";

/** An agent's state: amber and pulsing means it waits on you, the only use of that color. */
defineProps<{ status: AgentStatus }>();
</script>

<template>
  <span class="dot" :class="status" :title="{ waiting: 'Attend ta réponse', working: 'Travaille', idle: 'Au repos', offline: 'Hors ligne' }[status]" />
</template>

<style scoped>
.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--surface-1);
}
.waiting {
  background: var(--attention);
  animation: pulse 1.6s ease-in-out infinite;
}
.working {
  background: var(--working);
  box-shadow: 0 0 8px color-mix(in oklab, var(--working) 60%, transparent);
}
.idle {
  background: var(--overlay);
}
.offline {
  background: transparent;
  box-shadow: inset 0 0 0 1.5px var(--surface-1);
}
</style>
