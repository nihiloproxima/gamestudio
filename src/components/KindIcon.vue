<script setup lang="ts">
import { Bug, Lightbulb, SquareCheckBig } from "lucide-vue-next";
import type { ItemKind } from "../domain/types";

defineProps<{ kind: ItemKind; size?: number }>();
</script>

<template>
  <Bug v-if="kind === 'bug'" :size="size ?? 14" class="kind bug" />
  <Lightbulb v-else-if="kind === 'idea'" :size="size ?? 14" class="kind idea" />
  <SquareCheckBig v-else :size="size ?? 14" class="kind task" />
</template>

<style scoped>
.kind {
  flex: none;
}
.bug {
  color: var(--kind-bug);
}
.idea {
  color: var(--kind-idea);
}
.task {
  color: var(--overlay);
}
</style>
