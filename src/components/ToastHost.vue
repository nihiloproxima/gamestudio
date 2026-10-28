<script setup lang="ts">
import { store } from "../state/store";
</script>

<template>
  <div class="toasts" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="t in store.toasts" :key="t.id" class="toast" :class="t.tone">
        <span class="text selectable">{{ t.text }}</span>
        <button
          v-if="t.action"
          class="action"
          @click="
            t.action.run();
            store.toasts = store.toasts.filter((x) => x.id !== t.id);
          "
        >
          {{ t.action.label }}
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toasts {
  position: fixed;
  bottom: 18px;
  left: 50%;
  z-index: 70;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  transform: translateX(-50%);
  pointer-events: none;
}
.toast {
  display: flex;
  align-items: center;
  gap: 14px;
  max-width: 560px;
  padding: 10px 14px;
  border-radius: var(--radius-md);
  background: var(--surface-0);
  box-shadow: var(--shadow-pop);
  font-size: var(--text-sm);
  pointer-events: auto;
}
.toast.error {
  background: color-mix(in oklab, var(--fail) 22%, var(--raised));
  color: var(--bright);
}
.action {
  color: var(--accent);
  font-weight: 650;
}
.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 160ms ease,
    transform 200ms var(--ease-out);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
</style>
