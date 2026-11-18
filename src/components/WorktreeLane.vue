<script setup lang="ts">
import { Archive, Bot, Code, FolderGit2, GitBranch, GitMerge, GitPullRequest, MonitorPlay, SquareTerminal, Trash2 } from "lucide-vue-next";
import AgentIcon from "./AgentIcon.vue";
import StatusDot from "./StatusDot.vue";
import type { Session, Worktree } from "../domain/types";
import { confirmAsk, fail, go, notify, itemsById, now, playGame, project, refreshWorktrees, startSession, statusOf, store } from "../state/store";
import { api } from "../lib/api";
import { mentionedIds, prForBranch } from "../lib/links";
import { ago, tilde } from "../lib/format";

/** One worktree with its branch state, its PR and the sessions running in it; without a worktree, sessions found nowhere. */
const props = defineProps<{ worktree?: Worktree; sessions: Session[]; selectedId?: string }>();

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

async function archiveWorktree(w: Worktree) {
  const base = project.value?.base ?? "la base";
  const text = `Archiver ${w.branch} ? Le worktree et la branche locale sont supprimés, son travail est déjà dans ${base}.`;
  if (!(await confirmAsk(text, "Archiver"))) return;
  try {
    await api.worktreeArchive(store.projectId, w.path);
    await refreshWorktrees();
    notify(`${w.branch} archivée`);
  } catch (e) {
    fail(e, "Archivage");
  }
}

const live = () => props.sessions.some((s) => s.status !== "offline");

/** Its work looks landed: nothing ahead of the base, or its PR merged. The core checks again before deleting. */
function landed(w: Worktree) {
  if (w.main || !w.branch || w.dirty > 0 || live()) return false;
  return (w.ahead === 0 && w.lastCommit !== null) || prForBranch(store.prs, w.branch)?.state === "merged";
}
</script>

<template>
  <section class="lane" :class="{ live: live() }">
    <template v-if="worktree">
      <header class="lane-head">
        <component :is="worktree.main ? FolderGit2 : GitBranch" :size="15" class="lane-icon" />
        <span class="branch code">{{ worktree.branch ?? worktree.head.slice(0, 7) }}</span>
        <span v-if="worktree.main" class="chip">principal</span>
        <StatusDot v-if="sessions.length" :status="statusOf(sessions)" />
        <span class="grow" />
        <div class="lane-tools">
          <button class="btn btn-ghost btn-sm btn-icon" title="Lancer le projet ici" @click="playGame(worktree.path)"><MonitorPlay /></button>
          <button class="btn btn-ghost btn-sm btn-icon" title="Terminal ici" @click="shellHere(worktree.path)"><SquareTerminal /></button>
          <button class="btn btn-ghost btn-sm btn-icon" title="Agent ici" @click="store.launch = { path: worktree.main ? undefined : worktree.path }"><Bot /></button>
          <button class="btn btn-ghost btn-sm btn-icon" title="Ouvrir dans VS Code" @click="code(worktree.path)"><Code /></button>
          <button
            v-if="!worktree.main"
            class="btn btn-ghost btn-sm btn-icon"
            title="Supprimer le worktree"
            :disabled="worktree.dirty > 0 || live()"
            @click="removeWorktree(worktree)"
          >
            <Trash2 />
          </button>
        </div>
      </header>
      <div class="facts">
        <span v-if="worktree.dirty" class="fact dirty">{{ worktree.dirty }} non commité{{ worktree.dirty > 1 ? "s" : "" }}</span>
        <span v-if="worktree.ahead" class="fact">{{ worktree.ahead }} en avance</span>
        <span v-if="worktree.behind" class="fact behind">{{ worktree.behind }} en retard</span>
        <button v-for="id in linkedIds(worktree)" :key="id" class="fact item code" @click="store.drawer = id">{{ id }}</button>
        <template v-for="pr in [prForBranch(store.prs, worktree.branch)]" :key="pr?.number ?? 'none'">
          <span v-if="pr" class="fact pr" :class="[pr.state, pr.checks]">
            <GitMerge v-if="pr.state === 'merged'" :size="11" /><GitPullRequest v-else :size="11" />#{{ pr.number }}
          </span>
        </template>
        <button v-if="landed(worktree)" class="fact archive" title="Supprimer le worktree et sa branche locale" @click="archiveWorktree(worktree)">
          <Archive :size="11" />Archiver
        </button>
      </div>
      <p v-if="worktree.lastCommit" class="commit"><span class="muted">{{ ago(worktree.lastCommitAt, now) }}</span> {{ worktree.lastCommit }}</p>
    </template>
    <header v-else class="lane-head"><span class="branch">Ailleurs</span></header>
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
</template>

<style scoped>
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
.fact.archive {
  color: var(--text);
  cursor: pointer;
}
.fact.archive:hover {
  background: var(--surface-1);
  color: var(--bright);
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
</style>
