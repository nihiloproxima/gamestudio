<script setup lang="ts">
/** A centered panel over a dimmed app. Escape or a click on the backdrop closes it. */
defineProps<{ width?: number; top?: boolean }>();
const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <div class="backdrop" :class="{ top }" @mousedown.self="emit('close')" @keydown.esc.stop.prevent="emit('close')">
    <div class="panel" :style="{ width: `${width ?? 560}px` }" role="dialog" aria-modal="true">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px;
  background: rgb(8 8 14 / 0.55);
  backdrop-filter: blur(3px);
  animation: fade 140ms ease;
}
.backdrop.top {
  align-items: flex-start;
  padding-top: 14vh;
}
.panel {
  max-width: 100%;
  max-height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-lg);
  background: var(--raised);
  box-shadow: var(--shadow-pop);
  animation: rise 180ms var(--ease-out);
}
@keyframes fade {
  from {
    opacity: 0;
  }
}
@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.985);
  }
}
</style>
