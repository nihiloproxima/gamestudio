<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Bug, CornerDownLeft, Lightbulb, SquareCheckBig } from "lucide-vue-next";
import Modal from "./Modal.vue";
import PickMenu from "./PickMenu.vue";
import PriorityIcon from "./PriorityIcon.vue";
import { PRIORITIES, label, type ItemKind, type Priority } from "../domain/types";
import { createItem, project, store } from "../state/store";

/** The fastest way in: a title and Enter. Tasks go to "À faire", bugs and ideas to the inbox for triage. */
const kind = ref<ItemKind>(store.capture?.kind ?? "task");
const title = ref("");
const body = ref("");
const priority = ref<Priority>("normal");
const NONE = "__none__";
const milestone = ref(store.capture?.milestone ?? NONE);
const input = ref<HTMLInputElement>();

const milestoneOptions = computed(() => [{ id: NONE, label: "Sans jalon" }, ...store.milestones.map((m) => ({ id: m.id, label: m.title }))]);
const milestoneLabel = computed(() => milestoneOptions.value.find((o) => o.id === milestone.value)?.label);

onMounted(() => input.value?.focus());

async function submit(open: boolean) {
  if (!title.value.trim()) return;
  const created = await createItem(
    {
      title: title.value.trim(),
      kind: kind.value,
      priority: priority.value,
      milestone: milestone.value === NONE ? undefined : milestone.value,
      body: body.value,
    },
    open,
  );
  if (created) store.capture = null;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Enter" && (e.metaKey || !(e.target instanceof HTMLTextAreaElement))) {
    e.preventDefault();
    submit(e.metaKey && e.target instanceof HTMLInputElement);
  }
}
</script>

<template>
  <Modal :width="600" top @close="store.capture = null">
    <div class="capture" @keydown="onKey">
      <header>
        <span class="where">{{ project?.name }}</span>
        <div class="segmented">
          <button :class="{ on: kind === 'task' }" @click="kind = 'task'"><SquareCheckBig />Tâche</button>
          <button :class="{ on: kind === 'bug' }" @click="kind = 'bug'"><Bug />Bug</button>
          <button :class="{ on: kind === 'idea' }" @click="kind = 'idea'"><Lightbulb />Idée</button>
        </div>
      </header>
      <input
        ref="input"
        v-model="title"
        class="title"
        :placeholder="{ task: 'Que faut-il faire ?', bug: 'Qu\'est-ce qui ne marche pas ?', idea: 'Quelle idée ?' }[kind]"
      />
      <textarea v-model="body" class="body" rows="3" placeholder="Détails, si besoin (Markdown)" />
      <footer>
        <PickMenu v-model="priority" :options="PRIORITIES">
          <template #default="{ toggle }">
            <button class="btn btn-quiet btn-sm" @click="toggle"><PriorityIcon :priority="priority" />{{ label(PRIORITIES, priority) }}</button>
          </template>
          <template #option="{ option }">
            <PriorityIcon :priority="option.id as Priority" /><span class="text">{{ option.label }}</span>
          </template>
        </PickMenu>
        <PickMenu v-if="store.milestones.length" v-model="milestone" :options="milestoneOptions" filterable>
          <template #default="{ toggle }">
            <button class="btn btn-quiet btn-sm" @click="toggle">{{ milestoneLabel }}</button>
          </template>
        </PickMenu>
        <span class="hint">
          {{ kind === "task" ? "Va dans À faire" : "Va dans À trier" }}, <kbd>⌘</kbd><kbd>↵</kbd> crée et ouvre
        </span>
        <button class="btn btn-solid" :disabled="!title.trim()" @click="submit(false)">Créer<CornerDownLeft /></button>
      </footer>
    </div>
  </Modal>
</template>

<style scoped>
.capture {
  display: flex;
  flex-direction: column;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px 0 20px;
}
.where {
  color: var(--accent);
  font-size: var(--text-xs);
  font-weight: 650;
}
.title {
  margin: 14px 20px 0;
  border: 0;
  background: none;
  outline: none;
  color: var(--bright);
  font-size: var(--text-xl);
  font-weight: 600;
  user-select: text;
}
.title::placeholder,
.body::placeholder {
  color: var(--overlay);
}
.body {
  margin: 8px 20px 0;
  border: 0;
  background: none;
  outline: none;
  resize: none;
  font-size: var(--text-md);
  line-height: 1.5;
  user-select: text;
}
footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--line);
}
.hint {
  flex: 1;
  color: var(--overlay);
  font-size: var(--text-xs);
  text-align: right;
}
</style>
