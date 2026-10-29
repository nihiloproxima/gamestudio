<script setup lang="ts">
import { computed, onMounted } from "vue";
import {
  Bot,
  Code,
  FolderGit2,
  GitBranch,
  GitMerge,
  GitPullRequest,
  Network,
  Play,
  Plus,
  Power,
  RotateCcw,
  SquareTerminal,
  Trash2,
} from "lucide-vue-next";
import AgentIcon from "../components/AgentIcon.vue";
import StatusDot from "../components/StatusDot.vue";
import TerminalView from "../components/TerminalView.vue";
import type { Session, Worktree } from "../domain/types";
import {
  confirmAsk,
  fail,
  go,
  itemsById,
  now,
  openOrchestration,
  orchestrator,
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
import { mentionedIds, prForBranch } from "../lib/links";
import { ago, since, tilde } from "../lib/format";

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

const lanes = computed(() => {
  const list = store.worktrees ?? [];
  const sessions = sessionsByUrgency(projectSessions.value);
  const lanes = list.map((w) => ({ worktree: w, sessions: sessions.filter((s) => home(s, list) === w) }));
  const placed = new Set(lanes.flatMap((l) => l.sessions.map((s) => s.id)));
  return { lanes, elsewhere: sessions.filter((s) => !placed.has(s.id)) };
});

function linkedIds(w: Worktree) {
  return mentionedIds(w.branch, project.value?.key ?? "").filter((id) => itemsById.value.has(id));
}

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

async function removeWorktree(w: Worktree) {
  if (!(await confirmAsk(`Supprimer le worktree ${w.branch ?? tilde(w.path)} ? La branche est conservée.`, "Supprimer"))) return;
  try {
    await api.worktreeRemove(store.projectId, w.path);
    await refreshWorktrees();
  } catch (e) {
    fail(e, "Suppression du worktree");
  }
}

async function stop(s: Session) {
  const what = s.agent === "shell" ? "ce terminal" : `l'agent ${s.agent} sur « ${s.title} »`;
  if (await confirmAsk(`Arrêter ${what} ? Ce qui tourne dans la session est interrompu.`, "Arrêter")) stopSession(s.id);
}

const STATUS_TEXT = { waiting: "attend ta réponse", working: "travaille", idle: "au repos", offline: "arrêté" };
const AGENT_NAME = { claude: "Claude", codex: "Codex", shell: "Terminal" };

function agentName(s: Session) {
  if (s.role === "orchestrator") return "L'orchestrateur";
  return s.role === "worker" ? `${AGENT_NAME[s.agent]}, pour l'orchestrateur,` : AGENT_NAME[s.agent];
}
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Agents</h1>
      <span class="muted">{{ projectSessions.filter((s) => s.status !== "offline").length }} session(s) en cours</span>
      <span class="spacer" />
      <button class="btn btn-quiet" @click="openOrchestration()"><Network />{{ orchestrator ? "Orchestrateur" : "Orchestrer" }}</button>
      <button class="btn btn-solid" @click="store.launch = {}"><Play />Lancer un agent</button>
    </header>

    <div class="split">
      <aside class="lanes">
        <p v-if="store.worktreesError" class="error">{{ store.worktreesError }}</p>
        <section v-for="{ worktree: w, sessions } in lanes.lanes" :key="w.path" class="lane" :class="{ live: sessions.some((s) => s.status !== 'offline') }">
          <header class="lane-head">
            <component :is="w.main ? FolderGit2 : GitBranch" :size="15" class="lane-icon" />
            <span class="branch code">{{ w.branch ?? w.head.slice(0, 7) }}</span>
            <span v-if="w.main" class="chip">principal</span>
            <StatusDot v-if="sessions.length" :status="statusOf(sessions)" />
            <span class="grow" />
            <div class="lane-tools">
              <button class="btn btn-ghost btn-sm btn-icon" title="Terminal ici" @click="shellHere(w.path)"><SquareTerminal /></button>
              <button class="btn btn-ghost btn-sm btn-icon" title="Agent ici" @click="store.launch = { path: w.main ? undefined : w.path }"><Bot /></button>
              <button class="btn btn-ghost btn-sm btn-icon" title="Ouvrir dans VS Code" @click="code(w.path)"><Code /></button>
              <button
                v-if="!w.main"
                class="btn btn-ghost btn-sm btn-icon"
                title="Supprimer le worktree"
                :disabled="w.dirty > 0 || sessions.some((s) => s.status !== 'offline')"
                @click="removeWorktree(w)"
              >
                <Trash2 />
              </button>
            </div>
          </header>
          <div class="facts">
            <span v-if="w.dirty" class="fact dirty">{{ w.dirty }} non commité{{ w.dirty > 1 ? "s" : "" }}</span>
            <span v-if="w.ahead" class="fact">{{ w.ahead }} en avance</span>
            <span v-if="w.behind" class="fact behind">{{ w.behind }} en retard</span>
            <button v-for="id in linkedIds(w)" :key="id" class="fact item code" @click="store.drawer = id">{{ id }}</button>
            <template v-for="pr in [prForBranch(store.prs, w.branch)]" :key="pr?.number ?? 'none'">
              <span v-if="pr" class="fact pr" :class="[pr.state, pr.checks]">
                <GitMerge v-if="pr.state === 'merged'" :size="11" /><GitPullRequest v-else :size="11" />#{{ pr.number }}
              </span>
            </template>
          </div>
          <p v-if="w.lastCommit" class="commit"><span class="muted">{{ ago(w.lastCommitAt, now) }}</span> {{ w.lastCommit }}</p>
          <button
            v-for="s in sessions"
            :key="s.id"
            class="session"
            :class="{ on: s.id === selectedId }"
            @click="go({ name: 'agents', session: s.id })"
          >
            <StatusDot :status="s.status" />
            <AgentIcon :agent="s.agent" :size="13" />
            <span class="title">{{ s.item ? `${s.item} ${s.title}` : s.title }}</span>
            <span class="when">{{ ago(s.statusAt, now) }}</span>
          </button>
        </section>

        <section v-if="lanes.elsewhere.length" class="lane">
          <header class="lane-head"><span class="branch">Ailleurs</span></header>
          <button
            v-for="s in lanes.elsewhere"
            :key="s.id"
            class="session"
            :class="{ on: s.id === selectedId }"
            @click="go({ name: 'agents', session: s.id })"
          >
            <StatusDot :status="s.status" />
            <AgentIcon :agent="s.agent" :size="13" />
            <span class="title">{{ s.title }}</span>
            <span class="when">{{ ago(s.statusAt, now) }}</span>
          </button>
        </section>
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
                  {{ agentName(selected) }} {{ STATUS_TEXT[selected.status] }} {{ since(selected.statusAt, now) }}, dans
                  <span class="code">{{ tilde(selected.cwd) }}</span>
                </span>
              </p>
            </div>
            <button class="btn btn-ghost btn-sm" @click="code(selected.cwd)"><Code />VS Code</button>
            <button class="btn btn-ghost btn-sm" @click="shellHere(selected.cwd)"><Plus />Terminal ici</button>
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
.lane {
  padding: 10px 8px 8px;
  border-radius: var(--radius-md);
  background: var(--raised);
  border: 1px solid var(--line);
}
.lane.live {
  border-color: var(--line-strong);
}
.lane-head {
  display: flex;
  align-items: center;
  gap: 7px;
  min-height: 26px;
  padding: 0 4px;
}
.lane-icon {
  color: var(--overlay);
}
.branch {
  min-width: 0;
  overflow: hidden;
  color: var(--bright);
  font-size: var(--text-sm);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.grow {
  flex: 1;
}
.lane-tools {
  display: flex;
  opacity: 0;
  transition: opacity 120ms;
}
.lane:hover .lane-tools {
  opacity: 1;
}
.facts {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin: 4px 4px 0;
}
.fact {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 18px;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--surface-0);
  color: var(--subtext);
  font-size: var(--text-2xs);
  font-weight: 600;
}
.fact.dirty {
  color: var(--pr-high);
}
.fact.behind {
  color: var(--overlay);
}
.fact.item {
  color: var(--accent);
}
.fact.pr.merged {
  color: var(--review);
}
.fact.pr.fail {
  color: var(--fail);
}
.fact.pr.pass {
  color: var(--ok);
}
.commit {
  margin: 6px 4px 4px;
  overflow: hidden;
  color: var(--subtext);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 30px;
  margin-top: 2px;
  padding: 0 8px;
  border-radius: var(--radius-sm);
  text-align: left;
  font-size: var(--text-sm);
}
.session:hover {
  background: var(--surface-0);
}
.session.on {
  background: var(--accent-soft);
  box-shadow: inset 2px 0 0 var(--accent);
}
.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.when {
  color: var(--overlay);
  font-size: var(--text-2xs);
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
