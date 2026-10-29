<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import {
  Bot,
  Bug,
  GitPullRequest,
  Images,
  LayoutDashboard,
  Network,
  Lightbulb,
  ListTodo,
  Map,
  Play,
  RefreshCw,
  Settings,
  Sparkles,
  SquareCheckBig,
} from "lucide-vue-next";
import Modal from "./Modal.vue";
import ProjectGlyph from "./ProjectGlyph.vue";
import StatusIcon from "./StatusIcon.vue";
import StatusDot from "./StatusDot.vue";
import type { Item, Project, Session } from "../domain/types";
import { go, isOpen, openOrchestration, orchestrator, project, setupWithClaude, store, switchProject, syncNow } from "../state/store";

/** One box to reach anything: items by id or words, views, actions, other games, running sessions. */
type Entry =
  | { kind: "action"; id: string; label: string; icon: unknown; hint?: string; run: () => void }
  | { kind: "project"; id: string; label: string; project: Project; run: () => void }
  | { kind: "session"; id: string; label: string; session: Session; run: () => void }
  | { kind: "item"; id: string; label: string; item: Item; run: () => void };

const query = ref("");
const cursor = ref(0);
const input = ref<HTMLInputElement>();
const listEl = ref<HTMLElement>();

onMounted(() => input.value?.focus());

function close() {
  store.palette = false;
}

const actions = computed<Entry[]>(() => [
  { kind: "action", id: "new-task", label: "Nouvelle tâche", icon: SquareCheckBig, hint: "C", run: () => (store.capture = { kind: "task" }) },
  { kind: "action", id: "new-bug", label: "Signaler un bug", icon: Bug, hint: "B", run: () => (store.capture = { kind: "bug" }) },
  { kind: "action", id: "new-idea", label: "Noter une idée", icon: Lightbulb, hint: "I", run: () => (store.capture = { kind: "idea" }) },
  { kind: "action", id: "new-session", label: "Lancer un agent", icon: Play, hint: "A", run: () => (store.launch = {}) },
  { kind: "action", id: "orchestrate", label: orchestrator.value ? "Voir l'orchestrateur" : "Orchestrer des agents", icon: Network, hint: "O", run: openOrchestration },
  { kind: "action", id: "go-overview", label: "Aller à la vue d'ensemble", icon: LayoutDashboard, hint: "⌘1", run: () => go({ name: "overview" }) },
  { kind: "action", id: "go-roadmap", label: "Aller à la roadmap", icon: Map, hint: "⌘2", run: () => go({ name: "roadmap" }) },
  { kind: "action", id: "go-tasks", label: "Aller aux tâches", icon: ListTodo, hint: "⌘3", run: () => go({ name: "tasks" }) },
  { kind: "action", id: "go-backlog", label: "Aller aux idées et au backlog", icon: Lightbulb, hint: "⌘4", run: () => go({ name: "backlog" }) },
  { kind: "action", id: "go-agents", label: "Aller aux agents", icon: Bot, hint: "⌘5", run: () => go({ name: "agents" }) },
  { kind: "action", id: "go-assets", label: "Aller aux assets", icon: Images, hint: "⌘6", run: () => go({ name: "assets" }) },
  { kind: "action", id: "go-github", label: "Aller à GitHub", icon: GitPullRequest, hint: "⌘7", run: () => go({ name: "github" }) },
  { kind: "action", id: "sync", label: "Synchroniser les données", icon: RefreshCw, run: syncNow },
  { kind: "action", id: "setup", label: "Configurer le projet avec Claude", icon: Sparkles, run: () => setupWithClaude() },
  { kind: "action", id: "settings", label: "Réglages du projet", icon: Settings, hint: "⌘,", run: () => go({ name: "settings" }) },
]);

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Every word of the query must appear; an exact id wins. */
function score(text: string, q: string): number {
  const t = fold(text);
  const words = fold(q).split(/\s+/).filter(Boolean);
  if (!words.every((w) => t.includes(w))) return -1;
  return t.startsWith(words[0]) ? 2 : 1;
}

const entries = computed<Entry[]>(() => {
  const q = query.value.trim();
  const projects: Entry[] = store.projects
    .filter((p) => p.id !== store.projectId)
    .map((p) => ({ kind: "project", id: `p-${p.id}`, label: `Basculer vers ${p.name}`, project: p, run: () => switchProject(p.id) }));
  const sessions: Entry[] = store.sessions
    .filter((s) => s.project === store.projectId)
    .map((s) => ({ kind: "session", id: `s-${s.id}`, label: s.item ? `${s.item} ${s.title}` : s.title, session: s, run: () => go({ name: "agents", session: s.id }) }));
  const items: Entry[] = [...store.items]
    .sort((a, b) => Number(isOpen(b)) - Number(isOpen(a)) || b.updated.localeCompare(a.updated))
    .map((i) => ({ kind: "item", id: i.id, label: `${i.id} ${i.title}`, item: i, run: () => (store.drawer = i.id) }));

  if (!q) return [...actions.value.slice(0, 4), ...projects, ...sessions.slice(0, 4), ...items.slice(0, 6)];
  const exact = items.find((e) => e.id.toLowerCase() === q.toLowerCase());
  const ranked = [...items, ...actions.value, ...projects, ...sessions]
    .map((e) => ({ e, s: score(e.label, q) }))
    .filter((x) => x.s >= 0)
    .sort((a, b) => b.s - a.s)
    .map((x) => x.e)
    .slice(0, 40);
  return exact ? [exact, ...ranked.filter((e) => e !== exact)] : ranked;
});

watch(query, () => (cursor.value = 0));

function run(e: Entry | undefined) {
  if (!e) return;
  close();
  e.run();
}

function onKey(e: KeyboardEvent) {
  const n = entries.value.length;
  if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
    e.preventDefault();
    if (n) cursor.value = (cursor.value + 1) % n;
  } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
    e.preventDefault();
    if (n) cursor.value = (cursor.value - 1 + n) % n;
  } else if (e.key === "Enter") {
    e.preventDefault();
    run(entries.value[cursor.value]);
  } else {
    return;
  }
  nextTick(() => listEl.value?.querySelector(".cursor")?.scrollIntoView({ block: "nearest" }));
}
</script>

<template>
  <Modal :width="640" top @close="close">
    <div class="palette" @keydown="onKey">
      <input ref="input" v-model="query" class="query" :placeholder="`Chercher dans ${project?.name ?? 'le projet'}, ou une action…`" />
      <div ref="listEl" class="list">
        <button
          v-for="(e, i) in entries"
          :key="e.id"
          class="entry"
          :class="{ cursor: i === cursor }"
          @mouseenter="cursor = i"
          @click="run(e)"
        >
          <template v-if="e.kind === 'item'">
            <StatusIcon :status="e.item.status" />
            <span class="id code">{{ e.item.id }}</span>
            <span class="text">{{ e.item.title }}</span>
          </template>
          <template v-else-if="e.kind === 'project'">
            <ProjectGlyph :project="e.project" :size="18" />
            <span class="text">{{ e.label }}</span>
          </template>
          <template v-else-if="e.kind === 'session'">
            <StatusDot :status="e.session.status" />
            <span class="text">{{ e.label }}</span>
            <span class="hint">session</span>
          </template>
          <template v-else>
            <component :is="e.icon" :size="15" class="icon" />
            <span class="text">{{ e.label }}</span>
            <kbd v-if="e.hint">{{ e.hint }}</kbd>
          </template>
        </button>
        <p v-if="!entries.length" class="none">
          Rien ne correspond. <button class="link" @click="(store.capture = { kind: 'task' }), close()">Créer une tâche ?</button>
        </p>
      </div>
    </div>
  </Modal>
</template>

<style scoped>
.palette {
  display: flex;
  flex-direction: column;
  max-height: 60vh;
}
.query {
  height: 54px;
  padding: 0 20px;
  border: 0;
  border-bottom: 1px solid var(--line);
  background: none;
  outline: none;
  color: var(--bright);
  font-size: var(--text-lg);
  user-select: text;
}
.query::placeholder {
  color: var(--overlay);
}
.list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 6px;
}
.entry {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 36px;
  padding: 0 12px;
  border-radius: var(--radius-sm);
  text-align: left;
}
.entry.cursor {
  background: var(--surface-0);
}
.icon {
  color: var(--subtext);
}
.id {
  width: 60px;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.hint {
  color: var(--overlay);
  font-size: var(--text-xs);
}
.none {
  padding: 16px 12px;
  color: var(--overlay);
  font-size: var(--text-sm);
}
.link {
  color: var(--accent);
}
</style>
