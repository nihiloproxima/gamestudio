<script setup lang="ts">
import { Sparkles, SquareTerminal, Hexagon } from "lucide-vue-next";
import type { AgentKind } from "../domain/types";

defineProps<{ agent: AgentKind; size?: number }>();
</script>

<template>
  <Sparkles v-if="agent === 'claude'" :size="size ?? 14" class="agent claude" />
  <Hexagon v-else-if="agent === 'codex'" :size="size ?? 14" class="agent codex" />
  <SquareTerminal v-else :size="size ?? 14" class="agent shell" />
</template>

<style scoped>
.agent {
  flex: none;
}
.claude {
  color: #e8a68a;
}
.codex {
  color: #b4befe;
}
.shell {
  color: var(--overlay);
}
</style>
