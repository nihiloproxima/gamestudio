<script setup lang="ts">
import { computed, onMounted } from "vue";
import { Bot, Code, Network, Play, Plus, Power, RotateCcw } from "lucide-vue-next";
import AgentIcon from "../components/AgentIcon.vue";
import StatusDot from "../components/StatusDot.vue";
import TerminalView from "../components/TerminalView.vue";
import WorktreeLane from "../components/WorktreeLane.vue";
import type { Session, Worktree } from "../domain/types";
import {
  confirmAsk,
  fail,
  go,
  now,
  openOrchestration,
  playGame,
  project,
  projectSessions,
  refreshWorktrees,
  resumeSession,
  sessionsByUrgency,
  startSession,
  statusOf,
  stopSession,
  store,
} from "../state/store";
import { api } from "../lib/api";
import { since, tilde } from "../lib/format";

/** Every line of work in parallel: each worktree with its branch state, its PR and the agents running in it. */
onMounted(refreshWorktrees);

const selectedId = computed(() => (store.route.name === "agents" ? store.route.session : undefined));
const selected = computed(() => store.sessions.find((s) => s.id === selectedId.value));

/** The worktree a session runs in: the deepest one holding its directory. */
function home(s: Session, list: Worktree[]): Worktree | undefined {
  return list
    .filter((w) => s.cwd === w.path || s.cwd.startsWith(`${w.path}/`))
    .sort((a, b) => b.path.length - a.path.length)[0];
}

/** The orchestrator a session works for, following `parent` up; none for a session started by hand. */
function orchestratorOf(s: Session, byId: Map<string, Session>): Session | undefined {
  const seen = new Set<string>();
  for (let at: Session | undefined = s; at && !seen.has(at.id); at = at.parent ? byId.get(at.parent) : undefined) {
    if (at.role === "orchestrator") return at;
    seen.add(at.id);
  }
}

interface Lane {
  worktree?: Worktree;
  sessions: Session[];
}

/** Worktrees as lanes; those of an orchestration (its own and its agents') gathered under their orchestrator. */
const board = computed(() => {
  const list = store.worktrees ?? [];
  const sessions = sessionsByUrgency(projectSessions.value);
  const byId = new Map(projectSessions.value.map((s) => [s.id, s]));
  const lanes: Lane[] = list.map((w) => ({ worktree: w, sessions: sessions.filter((s) => home(s, list) === w) }));
  const placed = new Set(lanes.flatMap((l) => l.sessions.map((s) => s.id)));
  const elsewhere = sessions.filter((s) => !placed.has(s.id));

  const groups = sessions
    .filter((s) => s.role === "orchestrator")
    .map((orchestrator) => ({ orchestrator, lanes: [] as Lane[], all: [] as Session[] }));
  const groupOf = (s: Session) => groups.find((g) => g.orchestrator === orchestratorOf(s, byId));
  const loose: Lane[] = [];
  for (const lane of lanes) {
    // The main checkout stays on its own: an orchestration never works there.
    const group = lane.worktree?.main ? undefined : lane.sessions.map(groupOf).find(Boolean);
    (group?.lanes ?? loose).push(lane);
  }
  const stray: Session[] = [];
  for (const s of elsewhere) {
    const group = groupOf(s);
    if (group) group.lanes.push({ sessions: [s] });
    else stray.push(s);
  }
  for (const g of groups) {
    // The orchestrator's own worktree first, then its agents'.
    g.lanes.sort((a, b) => Number(!a.sessions.includes(g.orchestrator)) - Number(!b.sessions.includes(g.orchestrator)));
    g.all = g.lanes.flatMap((l) => l.sessions);
  }
  return { groups, lanes: loose, elsewhere: stray };
});

function shellHere(path: string) {
  startSession({ project: store.projectId, agent: "shell", place: "path", path });
}

async function code(path: string) {
  try {
    await api.openInVscode(path);
  } catch (e) {
    fail(e, "VS Code");
  }
}

async function stop(s: Session) {
  const what = s.agent === "shell" ? "ce terminal" : s.agent === "game" ? "le jeu" : `l'agent ${s.agent} sur « ${s.title} »`;
  if (await confirmAsk(`Arrêter ${what} ? Ce qui tourne dans la session est interrompu.`, "Arrêter")) stopSession(s.id);
}

const STATUS_TEXT = { waiting: "attend ta réponse", working: "travaille", idle: "au repos", offline: "arrêté" };
const AGENT_NAME = { claude: "Claude", codex: "Codex", opencode: "OpenCode", shell: "Terminal", game: "Le jeu" };

function agentName(s: Session) {
  const name = (s.role === "orchestrator" ? "L'orchestrateur" : AGENT_NAME[s.agent]) + (s.effort ? ` en effort ${s.effort}` : "");
  return s.role === "worker" ? `${name}, pour l'orchestrateur,` : name;
}

function statusText(s: Session) {
  return s.agent === "game" && s.status !== "offline" ? "lancé" : STATUS_TEXT[s.status];
}
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Agents</h1>
      <span class="muted">{{ projectSessions.filter((s) => s.status !== "offline").length }} session(s) en cours</span>
      <span class="spacer" />
      <button class="btn btn-quiet" @click="openOrchestration()"><Network />Orchestrer</button>
      <button class="btn btn-solid" @click="store.launch = {}"><Play />Lancer un agent</button>
    </header>

    <div class="split">
      <aside class="lanes">
        <p v-if="store.worktreesError" class="error">{{ store.worktreesError }}</p>
        <section v-for="g in board.groups" :key="g.orchestrator.id" class="orchestration">
          <button class="orch-head" :class="{ on: g.orchestrator.id === selectedId }" @click="go({ name: 'agents', session: g.orchestrator.id })">
            <Network :size="15" class="orch-icon" />
            <span class="orch-title">{{ g.orchestrator.title }}</span>
            <StatusDot :status="statusOf(g.all)" />
          </button>
          <div class="orch-work">
            <button
              v-for="w in g.all.filter((s) => s.role === 'worker' && s.item)"
              :key="w.id"
              class="fact"
              :title="w.title"
              @click="store.drawer = w.item"
            >
              <StatusDot :status="w.status" /><span class="code">{{ w.item }}</span>
            </button>
            <span v-if="!g.all.some((s) => s.role === 'worker')" class="muted">Aucun agent lancé pour l'instant</span>
          </div>
          <WorktreeLane v-for="l in g.lanes" :key="l.worktree?.path ?? l.sessions[0].id" :worktree="l.worktree" :sessions="l.sessions" :selected-id="selectedId" />
        </section>
        <WorktreeLane v-for="l in board.lanes" :key="l.worktree!.path" :worktree="l.worktree" :sessions="l.sessions" :selected-id="selectedId" />
        <WorktreeLane v-if="board.elsewhere.length" :sessions="board.elsewhere" :selected-id="selectedId" />
      </aside>

      <section class="stage">
        <template v-if="selected">
          <header class="stage-head">
            <StatusDot :status="selected.status" />
            <div class="who">
              <h2>
                <button v-if="selected.item" class="item-id code" @click="store.drawer = selected.item">{{ selected.item }}</button>
                {{ selected.title }}
              </h2>
              <p class="muted" :title="selected.cwd">
                <AgentIcon :agent="selected.agent" :size="12" />
                <span class="ellipsis">
                  {{ agentName(selected) }} {{ statusText(selected) }} {{ since(selected.statusAt, now) }}, dans
                  <span class="code">{{ tilde(selected.cwd) }}</span>
                </span>
              </p>
            </div>
            <button class="btn btn-ghost btn-sm" @click="code(selected.cwd)"><Code />VS Code</button>
            <button class="btn btn-ghost btn-sm" @click="shellHere(selected.cwd)"><Plus />Terminal ici</button>
            <button v-if="selected.agent === 'game' && selected.status !== 'offline'" class="btn btn-quiet btn-sm" @click="playGame(selected.cwd)">
              <RotateCcw />Relancer
            </button>
            <button v-if="selected.status === 'offline'" class="btn btn-quiet btn-sm" @click="resumeSession(selected.id)"><RotateCcw />Reprendre</button>
            <button class="btn btn-ghost btn-sm" :title="selected.status === 'offline' ? 'Oublier' : 'Arrêter'" @click="stop(selected)">
              <Power />{{ selected.status === "offline" ? "Oublier" : "Arrêter" }}
            </button>
          </header>
          <p v-if="selected.message && selected.status === 'waiting'" class="ask">{{ selected.message }}</p>
          <TerminalView v-if="selected.status !== 'offline'" :key="selected.id" :session="selected.id" />
          <div v-else class="empty offline">
            <RotateCcw />
            <p>
              Cette session s'est arrêtée (redémarrage ou fermeture).
              <template v-if="selected.agent === 'claude'">Claude reprendra {{ selected.claudeSession ? "sa conversation" : "la dernière conversation du dossier" }}.</template>
            </p>
            <button class="btn btn-quiet" @click="resumeSession(selected.id)"><RotateCcw />Reprendre</button>
          </div>
        </template>
        <div v-else class="empty hello">
          <Bot />
          <p v-if="projectSessions.length">Choisis une session à gauche pour voir son terminal.</p>
          <p v-else>Aucun agent sur {{ project?.name }}. Lance-en un sur une tâche, dans un worktree à lui ou dans le dépôt principal.</p>
          <button class="btn btn-solid" @click="store.launch = {}"><Play />Lancer un agent</button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.split {
  display: grid;
  grid-template-columns: 340px 1fr;
  flex: 1;
  min-height: 0;
}
.lanes {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px 12px 24px 16px;
  overflow-y: auto;
  border-right: 1px solid var(--line);
}
.orchestration {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 4px 4px 4px 7px;
  border-radius: var(--radius-md);
  background: color-mix(in oklab, var(--accent) 5%, transparent);
  border: 1px solid var(--accent-line);
  box-shadow: inset 2px 0 0 var(--accent);
}
.orch-head {
  display: flex;
  align-items: flex-start;
  gap: 7px;
  padding: 6px;
  border-radius: var(--radius-sm);
  text-align: left;
}
.orch-head:hover {
  background: var(--surface-0);
}
.orch-head.on {
  background: var(--accent-soft);
}
.orch-head .dot {
  margin-top: 5px;
}
.orch-icon {
  flex: none;
  margin-top: 1px;
  color: var(--accent);
}
.orch-title {
  flex: 1;
  min-width: 0;
  color: var(--bright);
  font-size: var(--text-sm);
  font-weight: 600;
  font-stretch: 125%;
  line-height: 1.35;
}
.orch-work {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
  padding: 0 6px 4px 28px;
  font-size: var(--text-xs);
}
.fact {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 20px;
  padding: 0 7px;
  border-radius: 4px;
  background: var(--surface-0);
  color: var(--accent);
  font-size: var(--text-2xs);
  font-weight: 600;
}
.fact:hover {
  background: var(--surface-1);
}
.error {
  padding: 8px;
  color: var(--fail);
  font-size: var(--text-sm);
}

.stage {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: #1e1e2e;
}
.stage-head {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: none;
  padding: 12px 16px 12px 20px;
  background: var(--base);
  border-bottom: 1px solid var(--line);
}
.who {
  flex: 1;
  min-width: 0;
}
.who h2 {
  overflow: hidden;
  font-size: var(--text-md);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.who p {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 3px;
  font-size: var(--text-xs);
}
.ellipsis {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.item-id {
  margin-right: 4px;
  color: var(--accent);
  font-size: var(--text-sm);
  font-stretch: 100%;
}
.ask {
  flex: none;
  padding: 10px 20px;
  background: color-mix(in oklab, var(--attention) 12%, var(--base));
  border-bottom: 1px solid color-mix(in oklab, var(--attention) 30%, transparent);
  color: var(--bright);
  font-size: var(--text-sm);
}
.hello,
.offline {
  flex: 1;
  align-content: center;
  background: var(--base);
}
.hello p,
.offline p {
  max-width: 420px;
}
</style>
