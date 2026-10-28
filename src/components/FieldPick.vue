<script setup lang="ts">
import { computed } from "vue";
import { Flag } from "lucide-vue-next";
import PickMenu from "./PickMenu.vue";
import StatusIcon from "./StatusIcon.vue";
import PriorityIcon from "./PriorityIcon.vue";
import KindIcon from "./KindIcon.vue";
import { KINDS, PRIORITIES, STATUSES, label, type Item, type ItemKind, type ItemStatus, type Priority } from "../domain/types";
import { store, updateItem } from "../state/store";

/** One of an item's fields with the menu that changes it: an icon in rows (`compact`), a labelled chip elsewhere. */
const props = defineProps<{ item: Item; field: "status" | "priority" | "kind" | "milestone"; compact?: boolean }>();

const NONE = "__none__";

const options = computed(() => {
  switch (props.field) {
    case "status":
      return STATUSES;
    case "priority":
      return PRIORITIES;
    case "kind":
      return KINDS;
    default:
      return [{ id: NONE, label: "Sans jalon" }, ...store.milestones.map((m) => ({ id: m.id, label: m.title }))];
  }
});

const value = computed(() => (props.field === "milestone" ? (props.item.milestone ?? NONE) : props.item[props.field]));

const milestoneTitle = computed(() => store.milestones.find((m) => m.id === props.item.milestone)?.title);

function set(v: string) {
  if (v === value.value) return;
  if (props.field === "milestone") updateItem(props.item.id, { milestone: v === NONE ? "" : v });
  else updateItem(props.item.id, { [props.field]: v });
}
</script>

<template>
  <PickMenu :options="options" :model-value="value" :filterable="field === 'milestone'" @update:model-value="set">
    <template #default="{ toggle }">
      <button
        class="trigger"
        :class="{ compact }"
        :title="compact ? label(options, value as never) : undefined"
        @click.stop="toggle"
      >
        <StatusIcon v-if="field === 'status'" :status="item.status" />
        <PriorityIcon v-else-if="field === 'priority'" :priority="item.priority" />
        <KindIcon v-else-if="field === 'kind'" :kind="item.kind" />
        <Flag v-else :size="13" class="flag" :class="{ set: item.milestone }" />
        <span v-if="!compact" class="text">
          {{ field === "milestone" ? (milestoneTitle ?? "Sans jalon") : label(options, value as never) }}
        </span>
      </button>
    </template>
    <template #option="{ option }">
      <StatusIcon v-if="field === 'status'" :status="option.id as ItemStatus" />
      <PriorityIcon v-else-if="field === 'priority'" :priority="option.id as Priority" />
      <KindIcon v-else-if="field === 'kind'" :kind="option.id as ItemKind" />
      <Flag v-else :size="13" class="flag" :class="{ set: option.id !== NONE }" />
      <span class="text">{{ option.label }}</span>
    </template>
  </PickMenu>
</template>

<style scoped>
.trigger {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 28px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  background: var(--surface-0);
  font-size: var(--text-sm);
  font-weight: 550;
  transition: background 120ms;
}
.trigger:hover {
  background: var(--surface-1);
}
.trigger.compact {
  width: 24px;
  height: 24px;
  padding: 0;
  justify-content: center;
  background: transparent;
}
.trigger.compact:hover {
  background: var(--surface-0);
}
.text {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.flag {
  flex: none;
  color: var(--overlay);
}
.flag.set {
  color: var(--accent);
}
</style>
