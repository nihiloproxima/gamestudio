<script setup lang="ts" generic="T extends string">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { Check } from "lucide-vue-next";

/** A small popover list to pick one value: arrows, Enter, typing filters, Escape or a click outside closes. */
const props = defineProps<{
  options: { id: T; label: string }[];
  modelValue?: T;
  align?: "left" | "right";
  filterable?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: T] }>();

const open = ref(false);
const query = ref("");
const cursor = ref(0);
const root = ref<HTMLElement>();
const input = ref<HTMLInputElement>();
const list = ref<HTMLElement>();

const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  return q ? props.options.filter((o) => o.label.toLowerCase().includes(q)) : props.options;
});

function toggle() {
  open.value = !open.value;
}

watch(open, async (isOpen) => {
  if (!isOpen) return;
  query.value = "";
  cursor.value = Math.max(0, props.options.findIndex((o) => o.id === props.modelValue));
  await nextTick();
  (input.value ?? list.value)?.focus();
});

watch(query, () => (cursor.value = 0));

function pick(id: T) {
  emit("update:modelValue", id);
  open.value = false;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    const n = shown.value.length;
    if (n) cursor.value = (cursor.value + (e.key === "ArrowDown" ? 1 : -1) + n) % n;
  } else if (e.key === "Enter") {
    e.preventDefault();
    const o = shown.value[cursor.value];
    if (o) pick(o.id);
  } else if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    open.value = false;
  }
}

function onOutside(e: MouseEvent) {
  if (open.value && root.value && !root.value.contains(e.target as Node)) open.value = false;
}
document.addEventListener("mousedown", onOutside);
onBeforeUnmount(() => document.removeEventListener("mousedown", onOutside));

defineExpose({ toggle });
</script>

<template>
  <div ref="root" class="pick">
    <slot :toggle="toggle" :open="open" />
    <Transition name="pop">
      <div v-if="open" class="menu" :class="align ?? 'left'" @keydown="onKey">
        <input v-if="filterable" ref="input" v-model="query" class="filter" placeholder="Filtrer…" />
        <div ref="list" class="list" tabindex="-1">
          <button
            v-for="(o, i) in shown"
            :key="o.id"
            class="opt"
            :class="{ cursor: i === cursor }"
            @mouseenter="cursor = i"
            @click="pick(o.id)"
          >
            <slot name="option" :option="o"><span class="text">{{ o.label }}</span></slot>
            <Check v-if="o.id === modelValue" :size="13" class="check" />
          </button>
          <p v-if="!shown.length" class="none">Aucun résultat</p>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.pick {
  position: relative;
  display: inline-flex;
}
.menu {
  position: absolute;
  top: calc(100% + 6px);
  z-index: 60;
  min-width: 200px;
  max-width: 320px;
  padding: 4px;
  border-radius: var(--radius-md);
  background: var(--raised);
  box-shadow: var(--shadow-pop);
}
.menu.left {
  left: 0;
}
.menu.right {
  right: 0;
}
.filter {
  width: 100%;
  height: 30px;
  margin-bottom: 4px;
  padding: 0 8px;
  border: 0;
  border-bottom: 1px solid var(--line-strong);
  background: transparent;
  outline: none;
}
.list {
  max-height: 300px;
  overflow-y: auto;
  outline: none;
}
.opt {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 30px;
  padding: 0 9px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  text-align: left;
}
.opt.cursor {
  background: var(--surface-0);
}
.opt :deep(.text) {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.check {
  margin-left: auto;
  color: var(--accent);
}
.none {
  padding: 8px 10px;
  color: var(--overlay);
  font-size: var(--text-sm);
}
.pop-enter-active,
.pop-leave-active {
  transition:
    opacity 110ms ease,
    transform 140ms var(--ease-out);
}
.pop-enter-from,
.pop-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
</style>
