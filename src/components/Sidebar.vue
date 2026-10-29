<script setup lang="ts">
import { computed } from "vue";
import {
  Bot,
  ChevronsUpDown,
  Cloud,
  Gamepad2,
  CloudOff,
  GitPullRequest,
  Images,
  LayoutDashboard,
  Lightbulb,
  ListTodo,
  Map,
  Plus,
  RefreshCw,
  RotateCcw,
  Search,
  Settings,
  TriangleAlert,
} from "lucide-vue-next";
import PickMenu from "./PickMenu.vue";
import ProjectGlyph from "./ProjectGlyph.vue";
import StatusDot from "./StatusDot.vue";
import AgentIcon from "./AgentIcon.vue";
import {
  go,
  isOpen,
  now,
  playGame,
  project,
  projectSessions,
  restartApp,
  sessionsByUrgency,
  store,
  switchProject,
  syncNow,
  waitingIn,
  type RouteName,
} from "../state/store";
import { since } from "../lib/format";

const nav = computed(() => {
  const open = store.items.filter(isOpen);
  const live = projectSessions.value.filter((s) => s.status !== "offline").length;
  const prs = store.prs?.filter((p) => p.state === "open" || p.state === "draft").length ?? 0;
  return [
    { id: "overview", label: "Vue d'ensemble", icon: LayoutDashboard, key: "1", count: 0 },
    { id: "roadmap", label: "Roadmap", icon: Map, key: "2", count: 0 },
    { id: "tasks", label: "Tâches", icon: ListTodo, key: "3", count: open.filter((i) => ["todo", "doing", "review"].includes(i.status)).length },
    { id: "backlog", label: "Idées et backlog", icon: Lightbulb, key: "4", count: open.filter((i) => i.status === "inbox").length, hot: true },
    { id: "agents", label: "Agents", icon: Bot, key: "5", count: live },
    { id: "assets", label: "Assets", icon: Images, key: "6", count: 0 },
    { id: "github", label: "GitHub", icon: GitPullRequest, key: "7", count: prs },
  ] as { id: RouteName; label: string; icon: unknown; key: string; count: number; hot?: boolean }[];
});

const projectOptions = computed(() => store.projects.map((p) => ({ id: p.id, label: p.name })));
const waitingElsewhere = computed(() => store.projects.some((p) => p.id !== store.projectId && waitingIn(p.id) > 0));

const sessions = computed(() => sessionsByUrgency(projectSessions.value).slice(0, 8));

const syncLabel = computed(() => {
  const s = store.sync;
  void now.value;
  if (!s) return "";
  if (s.syncing) return "Synchronisation…";
  if (s.error) return "Synchro en erreur";
  if (!s.remotes[store.projectId]) return "Pas de dépôt distant";
  return s.lastSync ? `Synchronisé ${since(s.lastSync)}` : "Synchronisé";
});

function goTo(id: RouteName) {
  go(id === "agents" ? { name: "agents" } : ({ name: id } as never));
}
</script>

<template>
  <aside class="sidebar">
    <div class="drag" data-tauri-drag-region />

    <div class="game">
      <PickMenu class="game-pick" :options="projectOptions" :model-value="store.projectId" @update:model-value="switchProject">
        <template #default="{ toggle }">
          <button v-if="project" class="switcher" @click="toggle">
            <ProjectGlyph :project="project" :size="30" />
            <span class="who">
              <span class="name">{{ project.name }}</span>
              <span class="key">{{ project.key }}</span>
            </span>
            <span v-if="waitingElsewhere" class="elsewhere" title="Un agent t'attend dans un autre projet" />
            <ChevronsUpDown :size="15" class="chev" />
          </button>
        </template>
        <template #option="{ option }">
          <ProjectGlyph :project="store.projects.find((p) => p.id === option.id)!" :size="20" />
          <span class="text">{{ option.label }}</span>
          <span v-if="waitingIn(option.id)" class="waiting-count">{{ waitingIn(option.id) }}</span>
          <kbd>⌥{{ store.projects.findIndex((p) => p.id === option.id) + 1 }}</kbd>
        </template>
      </PickMenu>
      <button v-if="project" class="play" title="Lancer le jeu (P)" :disabled="!project.present" @click="playGame()">
        <Gamepad2 :size="16" />
      </button>
    </div>

    <div class="quick">
      <button class="search" @click="store.palette = true">
        <Search :size="14" />
        <span>Rechercher, agir…</span>
        <kbd>⌘K</kbd>
      </button>
      <button class="add" title="Nouvelle tâche (C)" @click="store.capture = { kind: 'task' }">
        <Plus :size="16" />
      </button>
    </div>

    <nav class="nav">
      <button
        v-for="n in nav"
        :key="n.id"
        class="nav-item"
        :class="{ on: store.route.name === n.id }"
        @click="goTo(n.id)"
      >
        <component :is="n.icon" :size="16" class="icon" />
        <span class="label">{{ n.label }}</span>
        <span v-if="n.count" class="count" :class="{ hot: n.hot }">{{ n.count }}</span>
        <kbd class="hint">⌘{{ n.key }}</kbd>
      </button>
    </nav>

    <section class="sessions">
      <header>
        <span>Sessions</span>
        <button class="btn btn-ghost btn-sm btn-icon" title="Nouvelle session" @click="store.launch = {}">
          <Plus :size="14" />
        </button>
      </header>
      <button
        v-for="s in sessions"
        :key="s.id"
        class="session"
        :class="{ on: store.route.name === 'agents' && store.route.session === s.id }"
        @click="go({ name: 'agents', session: s.id })"
      >
        <StatusDot :status="s.status" />
        <AgentIcon :agent="s.agent" :size="13" />
        <span class="title">{{ s.item ? `${s.item} ${s.title}` : s.title }}</span>
      </button>
      <p v-if="!sessions.length" class="none">Aucun agent ne tourne</p>
    </section>

    <button v-if="store.update?.state === 'ready'" class="update" @click="restartApp">
      <RotateCcw :size="14" />
      <span>
        <strong>Gamestudio {{ store.update.latest?.replace(/^v/, "") }} est prête</strong>
        <span>Redémarrer pour l'utiliser</span>
      </span>
    </button>

    <footer class="foot">
      <button class="sync" :class="{ error: store.sync?.error }" :title="store.sync?.error ?? store.sync?.remotes[store.projectId] ?? ''" @click="syncNow">
        <RefreshCw v-if="store.sync?.syncing" :size="13" class="spin" />
        <TriangleAlert v-else-if="store.sync?.error" :size="13" />
        <CloudOff v-else-if="!store.sync?.remotes[store.projectId]" :size="13" />
        <Cloud v-else :size="13" />
        <span>{{ syncLabel }}</span>
      </button>
      <button
        class="btn btn-ghost btn-sm btn-icon"
        :class="{ on: store.route.name === 'settings' }"
        title="Réglages"
        @click="go({ name: 'settings' })"
      >
        <Settings :size="15" />
      </button>
    </footer>
  </aside>
</template>

<style scoped>
.sidebar {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 0 10px 10px;
  background:
    radial-gradient(140% 50% at 0% 0%, color-mix(in oklab, var(--accent) 9%, transparent), transparent 70%),
    var(--mantle);
  border-right: 1px solid var(--line);
  transition: background 400ms ease;
}
.drag {
  flex: none;
  height: 40px;
}
.game {
  display: flex;
  align-items: center;
  gap: 4px;
}
.game-pick {
  flex: 1;
  min-width: 0;
}
.switcher {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 7px 8px;
  border-radius: var(--radius-md);
  text-align: left;
  transition: background 120ms;
}
.switcher:hover {
  background: rgb(255 255 255 / 0.04);
}
.who {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}
.name {
  color: var(--bright);
  font-size: var(--text-lg);
  font-weight: 650;
  font-stretch: 125%;
  line-height: 1.1;
}
.key {
  color: var(--overlay);
  font-family: var(--font-code);
  font-size: var(--text-2xs);
}
.chev {
  color: var(--overlay);
}
.elsewhere {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--attention);
  animation: pulse 1.6s ease-in-out infinite;
}
.waiting-count {
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 9px;
  background: var(--attention);
  color: var(--crust);
  font-size: var(--text-2xs);
  font-weight: 700;
  text-align: center;
  line-height: 18px;
}

.quick {
  display: flex;
  gap: 6px;
  margin: 12px 0 14px;
}
.search {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  height: 30px;
  padding: 0 8px 0 10px;
  border-radius: var(--radius-sm);
  background: var(--crust);
  border: 1px solid var(--line);
  color: var(--overlay);
  font-size: var(--text-sm);
  transition: border-color 120ms;
}
.search:hover {
  border-color: var(--line-strong);
}
.search span {
  flex: 1;
  text-align: left;
}
.play {
  display: grid;
  flex: none;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--line);
  color: var(--accent);
  transition:
    background 120ms,
    border-color 120ms;
}
.play:hover:not(:disabled) {
  background: var(--accent-soft);
  border-color: var(--accent-line);
}
.play:disabled {
  color: var(--overlay);
  opacity: 0.5;
}
.add {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: var(--radius-sm);
  background: var(--accent);
  color: var(--crust);
  transition: background 120ms;
}
.add:hover {
  background: var(--accent-ink);
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 32px;
  padding: 0 8px 0 10px;
  border-radius: var(--radius-sm);
  color: var(--subtext);
  font-size: var(--text-md);
  font-weight: 520;
  transition:
    background 120ms,
    color 120ms;
}
.nav-item:hover {
  background: rgb(255 255 255 / 0.035);
  color: var(--text);
}
.nav-item.on {
  background: var(--accent-soft);
  color: var(--bright);
}
.nav-item.on .icon {
  color: var(--accent);
}
.label {
  flex: 1;
  text-align: left;
}
.count {
  color: var(--overlay);
  font-size: var(--text-xs);
  font-weight: 600;
}
.count.hot {
  color: var(--kind-idea);
}
.hint {
  display: none;
}
.nav-item:hover .hint {
  display: inline-flex;
}
.nav-item:hover .count {
  display: none;
}

.sessions {
  display: flex;
  flex-direction: column;
  gap: 1px;
  flex: 1;
  min-height: 0;
  margin-top: 22px;
  overflow-y: auto;
}
.sessions header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 4px 6px 10px;
  color: var(--overlay);
  font-size: var(--text-xs);
  font-weight: 600;
}
.session {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 8px 0 11px;
  border-radius: var(--radius-sm);
  color: var(--subtext);
  font-size: var(--text-sm);
  text-align: left;
}
.session:hover {
  background: rgb(255 255 255 / 0.035);
  color: var(--text);
}
.session.on {
  background: var(--surface-0);
  color: var(--bright);
}
.session .title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.none {
  padding: 2px 11px;
  color: var(--overlay);
  font-size: var(--text-xs);
}

.update {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
  padding: 9px 11px;
  border-radius: var(--radius-md);
  background: var(--accent-soft);
  border: 1px solid var(--accent-line);
  text-align: left;
  transition: background 120ms;
}
.update:hover {
  background: color-mix(in oklab, var(--accent) 22%, transparent);
}
.update svg {
  flex: none;
  color: var(--accent);
}
.update > span {
  display: flex;
  flex-direction: column;
  font-size: var(--text-xs);
}
.update strong {
  color: var(--bright);
  font-weight: 600;
}
.update span span {
  color: var(--subtext);
}
.foot {
  display: flex;
  align-items: center;
  gap: 4px;
  padding-top: 10px;
  border-top: 1px solid var(--line);
}
.sync {
  display: flex;
  align-items: center;
  gap: 7px;
  flex: 1;
  min-width: 0;
  height: 26px;
  padding: 0 8px;
  border-radius: var(--radius-sm);
  color: var(--overlay);
  font-size: var(--text-xs);
  text-align: left;
}
.sync span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sync:hover {
  background: rgb(255 255 255 / 0.035);
  color: var(--subtext);
}
.sync.error {
  color: var(--fail);
}
.foot .on {
  color: var(--accent);
}
</style>
