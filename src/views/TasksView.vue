<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronDown, ChevronRight, Flag, ListTodo, Plus, Search } from "lucide-vue-next";
import ItemRow from "../components/ItemRow.vue";
import PickMenu from "../components/PickMenu.vue";
import StatusIcon from "../components/StatusIcon.vue";
import type { Item, ItemStatus } from "../domain/types";
import { byPriority, store } from "../state/store";
import { useListNav } from "../lib/listnav";

const query = ref("");
const kind = ref<"all" | "task" | "bug">("all");
const ALL = "__all__";
const NONE = "__none__";
const milestone = ref<string>(ALL);
const showDone = ref(false);

const milestoneOptions = computed(() => [
  { id: ALL, label: "Tous les jalons" },
  { id: NONE, label: "Sans jalon" },
  ...store.milestones.map((m) => ({ id: m.id, label: m.title })),
]);
const milestoneLabel = computed(() => milestoneOptions.value.find((o) => o.id === milestone.value)?.label);

const RECENT = 14 * 86_400_000;

function matches(i: Item) {
  if (kind.value !== "all" && i.kind !== kind.value) return false;
  if (milestone.value === NONE && i.milestone) return false;
  if (milestone.value !== ALL && milestone.value !== NONE && i.milestone !== milestone.value) return false;
  const q = query.value.trim().toLowerCase();
  return !q || i.title.toLowerCase().includes(q) || i.id.toLowerCase().includes(q) || i.labels.some((l) => l.toLowerCase().includes(q));
}

const GROUPS: { status: ItemStatus; label: string }[] = [
  { status: "doing", label: "En cours" },
  { status: "review", label: "En revue" },
  { status: "todo", label: "À faire" },
  { status: "done", label: "Fait ces deux dernières semaines" },
];

const groups = computed(() =>
  GROUPS.map((g) => {
    let items = store.items.filter((i) => i.status === g.status && matches(i));
    if (g.status === "done") {
      items = items
        .filter((i) => Date.now() - Date.parse(i.closed ?? i.updated) < RECENT)
        .sort((a, b) => (b.closed ?? b.updated).localeCompare(a.closed ?? a.updated));
    } else {
      items.sort(byPriority);
    }
    return { ...g, items };
  }),
);

const visible = computed(() => groups.value.flatMap((g) => (g.status === "done" && !showDone.value ? [] : g.items)));
const selected = useListNav(visible);
const empty = computed(() => groups.value.every((g) => !g.items.length));
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Tâches</h1>
      <div class="segmented">
        <button :class="{ on: kind === 'all' }" @click="kind = 'all'">Tout</button>
        <button :class="{ on: kind === 'task' }" @click="kind = 'task'">Tâches</button>
        <button :class="{ on: kind === 'bug' }" @click="kind = 'bug'">Bugs</button>
      </div>
      <PickMenu v-model="milestone" :options="milestoneOptions" filterable>
        <template #default="{ toggle }">
          <button class="btn btn-ghost btn-sm" :class="{ active: milestone !== ALL }" @click="toggle">
            <Flag />{{ milestoneLabel }}
          </button>
        </template>
      </PickMenu>
      <span class="spacer" />
      <label class="find">
        <Search :size="14" />
        <input v-model="query" placeholder="Filtrer" @keydown.esc="query = ''" />
      </label>
      <button class="btn btn-solid" @click="store.capture = { kind: 'task', milestone: milestone !== ALL && milestone !== NONE ? milestone : undefined }">
        <Plus />Nouvelle tâche
      </button>
    </header>

    <div class="view-body">
      <div v-if="empty && !query" class="empty">
        <ListTodo />
        <p>Rien à faire pour l'instant. Une tâche se crée avec <kbd>C</kbd>, un bug avec <kbd>B</kbd>.</p>
      </div>
      <template v-for="g in groups" :key="g.status">
        <section v-if="g.items.length" class="group">
          <button v-if="g.status === 'done'" class="section-title toggle" @click="showDone = !showDone">
            <component :is="showDone ? ChevronDown : ChevronRight" :size="14" />
            <StatusIcon :status="g.status" />{{ g.label }}<span class="count">{{ g.items.length }}</span>
          </button>
          <h2 v-else class="section-title"><StatusIcon :status="g.status" />{{ g.label }}<span class="count">{{ g.items.length }}</span></h2>
          <div v-if="g.status !== 'done' || showDone" class="rows">
            <ItemRow v-for="i in g.items" :key="i.id" :item="i" show-milestone :selected="selected === i.id" />
          </div>
        </section>
      </template>
      <p v-if="empty && query" class="empty">Aucune tâche ne correspond à « {{ query }} ».</p>
    </div>
  </div>
</template>

<style scoped>
.group + .group {
  margin-top: 26px;
}
.rows {
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.toggle {
  cursor: pointer;
}
.toggle:hover {
  color: var(--text);
}
.active {
  color: var(--accent);
}
</style>
