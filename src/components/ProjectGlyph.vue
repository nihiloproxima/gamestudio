<script setup lang="ts">
import type { Project } from "../domain/types";

/** A game's mark: its color, its initial. */
defineProps<{ project: Pick<Project, "name" | "color">; size?: number }>();
</script>

<template>
  <span class="glyph" :style="{ '--c': project.color, '--s': `${size ?? 26}px` }">{{ project.name.slice(0, 1) }}</span>
</template>

<style scoped>
.glyph {
  flex: none;
  display: grid;
  place-items: center;
  width: var(--s);
  height: var(--s);
  border-radius: calc(var(--s) * 0.3);
  background:
    radial-gradient(120% 120% at 20% 10%, color-mix(in oklab, var(--c) 55%, white) 0%, var(--c) 45%, color-mix(in oklab, var(--c) 55%, black) 100%);
  color: color-mix(in oklab, var(--c) 25%, black);
  font-size: calc(var(--s) * 0.5);
  font-weight: 750;
  font-stretch: 125%;
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 0.35),
    0 2px 8px -2px color-mix(in oklab, var(--c) 60%, transparent);
}
</style>
