<script setup lang="ts">
import { computed, ref } from "vue";
import { Archive, ArrowRight, Bug, ChevronDown, ChevronRight, CornerDownLeft, Inbox, Lightbulb, ListTodo, X } from "lucide-vue-next";
import ItemRow from "../components/ItemRow.vue";
import StatusIcon from "../components/StatusIcon.vue";
import type { ItemKind, ItemStatus } from "../domain/types";
import { byPriority, createItem, store, updateItem } from "../state/store";
import { useListNav } from "../lib/listnav";

/** Where raw ideas and bug reports land, get triaged, and wait refined until they earn a place in the plan. */
const text = ref("");
const kind = ref<ItemKind>("idea");
const showDropped = ref(false);

async function capture() {
  const title = text.value.trim();
  if (!title) return;
  text.value = "";
  await createItem({ title, kind: kind.value });
}

const inbox = computed(() =>
  store.items.filter((i) => i.status === "inbox").sort((a, b) => b.created.localeCompare(a.created)),
);
const backlog = computed(() => store.items.filter((i) => i.status === "backlog").sort(byPriority));
const dropped = computed(() =>
  store.items
    .filter((i) => i.status === "dropped")
    .sort((a, b) => (b.closed ?? b.updated).localeCompare(a.closed ?? a.updated))
    .slice(0, 30),
);

const visible = computed(() => [...inbox.value, ...backlog.value, ...(showDropped.value ? dropped.value : [])]);
const selected = useListNav(visible);

function move(id: string, status: ItemStatus) {
  updateItem(id, { status });
}
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Idées et backlog</h1>
      <span class="spacer" />
    </header>

    <div class="view-body">
      <form class="capture" @submit.prevent="capture">
        <div class="segmented">
          <button type="button" :class="{ on: kind === 'idea' }" @click="kind = 'idea'"><Lightbulb />Idée</button>
          <button type="button" :class="{ on: kind === 'bug' }" @click="kind = 'bug'"><Bug />Bug</button>
        </div>
        <input
          v-model="text"
          class="capture-input"
          :placeholder="kind === 'idea' ? 'Une idée pour plus tard ? Tape-la, Entrée la range ici.' : 'Un bug remarqué ? Décris-le en une ligne.'"
        />
        <kbd v-if="text"><CornerDownLeft :size="11" /></kbd>
      </form>

      <section class="group">
        <h2 class="section-title"><StatusIcon status="inbox" />À trier<span class="count">{{ inbox.length }}</span></h2>
        <p v-if="!inbox.length" class="calm"><Inbox :size="15" />Tout est trié.</p>
        <div v-for="i in inbox" :key="i.id" class="triage">
          <ItemRow :item="i" :selected="selected === i.id" />
          <div class="actions">
            <button class="btn btn-quiet btn-sm" title="Garder pour plus tard" @click="move(i.id, 'backlog')">
              <Archive />Backlog
            </button>
            <button class="btn btn-quiet btn-sm" title="Prévoir maintenant" @click="move(i.id, 'todo')">
              <ListTodo />À faire
            </button>
            <button class="btn btn-ghost btn-sm btn-icon" title="Abandonner" @click="move(i.id, 'dropped')">
              <X />
            </button>
          </div>
        </div>
      </section>

      <section class="group">
        <h2 class="section-title"><StatusIcon status="backlog" />Backlog<span class="count">{{ backlog.length }}</span></h2>
        <p v-if="!backlog.length" class="calm">Le backlog est vide : ce qui est trié sans être prévu attend ici.</p>
        <div v-for="i in backlog" :key="i.id" class="triage">
          <ItemRow :item="i" show-milestone :selected="selected === i.id" />
          <div class="actions">
            <button class="btn btn-quiet btn-sm" title="Prévoir maintenant" @click="move(i.id, 'todo')">
              <ArrowRight />À faire
            </button>
          </div>
        </div>
      </section>

      <section v-if="dropped.length" class="group">
        <button class="section-title toggle" @click="showDropped = !showDropped">
          <component :is="showDropped ? ChevronDown : ChevronRight" :size="14" />
          <StatusIcon status="dropped" />Abandonnées<span class="count">{{ dropped.length }}</span>
        </button>
        <template v-if="showDropped">
          <ItemRow v-for="i in dropped" :key="i.id" :item="i" :selected="selected === i.id" />
        </template>
      </section>
    </div>
  </div>
</template>

<style scoped>
.capture {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 48px;
  padding: 0 14px 0 10px;
  margin-bottom: 28px;
  border-radius: var(--radius-md);
  background: var(--raised);
  border: 1px solid var(--line-strong);
  transition:
    border-color 140ms,
    box-shadow 140ms;
}
.capture:focus-within {
  border-color: var(--accent-line);
  box-shadow: 0 0 0 4px var(--accent-soft);
}
.capture-input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: none;
  outline: none;
  font-size: var(--text-md);
  user-select: text;
}
.capture-input::placeholder {
  color: var(--overlay);
}
.group + .group {
  margin-top: 28px;
}
.calm {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  color: var(--overlay);
  font-size: var(--text-sm);
}
.triage {
  position: relative;
}
.actions {
  position: absolute;
  top: 50%;
  right: 8px;
  display: none;
  gap: 4px;
  padding-left: 24px;
  transform: translateY(-50%);
  background: linear-gradient(90deg, transparent, var(--base) 24px);
}
.triage:hover .actions {
  display: flex;
}
.toggle {
  cursor: pointer;
}
.toggle:hover {
  color: var(--text);
}
</style>
