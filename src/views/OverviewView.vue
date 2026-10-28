<script setup lang="ts">
import { computed } from "vue";
import { Bot, Bug, CircleCheck, GitMerge, GitPullRequest, Lightbulb, Play, Plus, Sparkles, SquareCheckBig } from "lucide-vue-next";
import ItemRow from "../components/ItemRow.vue";
import ProjectGlyph from "../components/ProjectGlyph.vue";
import StatusDot from "../components/StatusDot.vue";
import AgentIcon from "../components/AgentIcon.vue";
import Markdown from "../components/Markdown.vue";
import {
  activeMilestone,
  byPriority,
  go,
  isOpen,
  now,
  progressOf,
  project,
  projectSessions,
  sessionsByUrgency,
  setupWithClaude,
  store,
} from "../state/store";
import { ago, dateLabel, daysUntil } from "../lib/format";
import { openExternal } from "../lib/native";

/** The morning glance: who waits on me, what moves, how far the current milestone is. */
const open = computed(() => store.items.filter(isOpen));
const inProgress = computed(() => open.value.filter((i) => i.status === "doing" || i.status === "review").sort(byPriority));
const next = computed(() => {
  const m = activeMilestone.value?.id;
  return open.value
    .filter((i) => i.status === "todo")
    .sort((a, b) => Number(b.milestone === m) - Number(a.milestone === m) || byPriority(a, b))
    .slice(0, 6);
});
const bugs = computed(() => open.value.filter((i) => i.kind === "bug" && i.status !== "inbox"));
const urgentBugs = computed(() => bugs.value.filter((i) => i.priority === "urgent").length);
const inbox = computed(() => open.value.filter((i) => i.status === "inbox").length);
const todoCount = computed(() => open.value.filter((i) => i.status === "todo").length);

const waiting = computed(() => projectSessions.value.filter((s) => s.status === "waiting"));
const live = computed(() => sessionsByUrgency(projectSessions.value.filter((s) => s.status !== "offline" && s.status !== "waiting")));
const openPrs = computed(() => (store.prs ?? []).filter((p) => p.state === "open" || p.state === "draft"));

const milestone = computed(() => activeMilestone.value);
const progress = computed(() => (milestone.value ? progressOf(milestone.value.id) : null));
const due = computed(() => {
  const t = milestone.value?.target;
  if (!t) return "";
  void now.value;
  const d = daysUntil(t);
  return d < 0 ? `en retard de ${-d} j` : d === 0 ? "aujourd'hui" : `encore ${d} j`;
});

const recent = computed(() =>
  [...store.items]
    .sort((a, b) => b.updated.localeCompare(a.updated))
    .slice(0, 8)
    .map((i) => ({
      item: i,
      verb: i.closed && i.updated === i.closed ? (i.status === "done" ? "terminé" : "abandonné") : i.created === i.updated ? "créé" : "modifié",
    })),
);

/** A fresh project: offer to let Claude read the repo and fill it. */
const blank = computed(() => !store.items.length && !store.milestones.length);

const firstLine = computed(() => project.value?.vision.split("\n").find((l) => l.trim()) ?? "");
</script>

<template>
  <div class="view">
    <header class="hero" data-tauri-drag-region>
      <ProjectGlyph v-if="project" :project="project" :size="46" />
      <div class="hero-text">
        <h1>{{ project?.name }}</h1>
        <Markdown v-if="firstLine" :source="firstLine" class="pitch" />
      </div>
      <span class="spacer" />
      <button class="btn btn-quiet" @click="store.launch = {}"><Play />Lancer un agent</button>
      <button class="btn btn-solid" @click="store.capture = { kind: 'task' }"><Plus />Nouvelle tâche</button>
    </header>

    <div class="view-body">
      <section v-if="blank" class="welcome">
        <Sparkles :size="22" class="spark" />
        <div class="welcome-text">
          <h2>{{ project?.name }} est encore vide</h2>
          <p>
            Claude peut lire le dépôt (plans, design, historique, PR, assets), te poser ses questions, puis préparer
            la roadmap, les tâches et les idées dans le bon état. Rien n'est écrit sans ton accord.
          </p>
        </div>
        <button class="btn btn-solid" :disabled="!project?.present" @click="setupWithClaude()"><Sparkles />Configurer avec Claude</button>
      </section>
      <div class="tiles">
        <button class="tile milestone" @click="go({ name: 'roadmap' })">
          <span class="tile-label">Jalon en cours</span>
          <template v-if="milestone && progress">
            <span class="ms-title">{{ milestone.title }}</span>
            <span class="meter" role="meter" :aria-valuenow="progress.done" :aria-valuemax="progress.total">
              <span class="meter-fill" :style="{ width: `${progress.ratio * 100}%` }" />
            </span>
            <span class="tile-foot">
              <strong>{{ Math.round(progress.ratio * 100) }} %</strong>
              {{ progress.done }} sur {{ progress.total }}<template v-if="milestone.target">, échéance {{ dateLabel(milestone.target) }} ({{ due }})</template>
            </span>
          </template>
          <span v-else class="tile-foot">Aucun jalon prévu : pose le prochain dans la roadmap.</span>
        </button>
        <button class="tile" @click="go({ name: 'tasks' })">
          <span class="tile-label"><SquareCheckBig :size="13" />En cours</span>
          <span class="big">{{ inProgress.length }}</span>
          <span class="tile-foot">{{ todoCount }} à faire ensuite</span>
        </button>
        <button class="tile" @click="go({ name: 'tasks' })">
          <span class="tile-label"><Bug :size="13" />Bugs ouverts</span>
          <span class="big">{{ bugs.length }}</span>
          <span class="tile-foot">{{ urgentBugs ? `dont ${urgentBugs} urgent${urgentBugs > 1 ? "s" : ""}` : "aucun urgent" }}</span>
        </button>
        <button class="tile" @click="go({ name: 'backlog' })">
          <span class="tile-label"><Lightbulb :size="13" />À trier</span>
          <span class="big">{{ inbox }}</span>
          <span class="tile-foot">{{ inbox ? "idées et bugs à trier" : "boîte vide" }}</span>
        </button>
      </div>

      <div class="columns">
        <div class="col">
          <section v-if="waiting.length" class="block">
            <h2 class="section-title">Ils t'attendent<span class="count">{{ waiting.length }}</span></h2>
            <button v-for="s in waiting" :key="s.id" class="ask" @click="go({ name: 'agents', session: s.id })">
              <StatusDot status="waiting" />
              <span class="ask-body">
                <span class="ask-title">{{ s.item ? `${s.item} ${s.title}` : s.title }}</span>
                <span v-if="s.message" class="ask-msg">{{ s.message }}</span>
              </span>
              <span class="when">{{ ago(s.statusAt, now) }}</span>
            </button>
          </section>

          <section class="block">
            <h2 class="section-title">En cours<span class="count">{{ inProgress.length }}</span></h2>
            <ItemRow v-for="i in inProgress" :key="i.id" :item="i" />
            <p v-if="!inProgress.length" class="calm">Rien en cours. Prends la suivante ci-dessous.</p>
          </section>

          <section class="block">
            <h2 class="section-title">Ensuite</h2>
            <ItemRow v-for="i in next" :key="i.id" :item="i" show-milestone />
            <p v-if="!next.length" class="calm">Rien de prévu : passe par le backlog.</p>
          </section>
        </div>

        <div class="col side">
          <section class="block">
            <h2 class="section-title">
              Agents<span class="count">{{ live.length + waiting.length }}</span>
            </h2>
            <button v-for="s in live" :key="s.id" class="line" @click="go({ name: 'agents', session: s.id })">
              <StatusDot :status="s.status" />
              <AgentIcon :agent="s.agent" :size="13" />
              <span class="line-text">{{ s.item ? `${s.item} ${s.title}` : s.title }}</span>
              <span class="when">{{ ago(s.statusAt, now) }}</span>
            </button>
            <p v-if="!live.length && !waiting.length" class="calm"><Bot :size="14" />Aucun agent ne tourne.</p>
            <p v-if="store.worktrees" class="foot-note">
              {{ store.worktrees.length }} worktree{{ store.worktrees.length > 1 ? "s" : "" }},
              {{ store.worktrees.filter((w) => w.dirty).length }} avec des changements non commités
            </p>
          </section>

          <section class="block">
            <h2 class="section-title">Pull requests<span class="count">{{ openPrs.length }}</span></h2>
            <button v-for="p in openPrs.slice(0, 6)" :key="p.number" class="line" @click="openExternal(p.url)">
              <GitPullRequest :size="14" class="pr" :class="[p.state, p.checks]" />
              <span class="line-text">#{{ p.number }} {{ p.title }}</span>
              <span class="ci" :class="p.checks">{{ { pass: "CI ok", fail: "CI ✕", pending: "CI…", none: "" }[p.checks] }}</span>
            </button>
            <p v-if="store.prs && !openPrs.length" class="calm"><GitMerge :size="14" />Aucune PR ouverte.</p>
            <p v-if="store.prsError" class="calm">{{ store.prsError }}</p>
          </section>

          <section class="block">
            <h2 class="section-title">Récemment</h2>
            <button v-for="r in recent" :key="r.item.id" class="line" @click="store.drawer = r.item.id">
              <CircleCheck v-if="r.verb === 'terminé'" :size="14" class="done" />
              <span v-else class="bullet" />
              <span class="line-text"><span class="code muted">{{ r.item.id }}</span> {{ r.item.title }}</span>
              <span class="when">{{ r.verb }}, {{ ago(r.item.updated, now) }}</span>
            </button>
          </section>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hero {
  display: flex;
  align-items: center;
  gap: 16px;
  flex: none;
  padding: 26px 28px 18px;
  background: radial-gradient(80% 160% at 0% 0%, var(--accent-soft), transparent 70%);
}
.hero h1 {
  font-size: var(--text-2xl);
}
.hero-text {
  min-width: 0;
}
.pitch {
  max-width: 640px;
  margin-top: 4px;
  color: var(--subtext);
  font-size: var(--text-sm);
}
.spacer {
  flex: 1;
}
.view-body {
  padding-top: 8px;
}

.welcome {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 18px;
  padding: 18px 20px;
  border-radius: var(--radius-lg);
  background:
    radial-gradient(120% 160% at 0% 0%, color-mix(in oklab, #e8a68a 12%, transparent), transparent 60%),
    var(--raised);
  border: 1px solid var(--line-strong);
}
.welcome-text {
  flex: 1;
}
.welcome h2 {
  font-size: var(--text-lg);
}
.welcome p {
  max-width: 640px;
  margin-top: 4px;
  color: var(--subtext);
  font-size: var(--text-sm);
  line-height: 1.5;
}
.spark {
  flex: none;
  color: #e8a68a;
}
.tiles {
  display: grid;
  grid-template-columns: 2fr repeat(3, 1fr);
  gap: 12px;
}
.tile {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  min-height: 116px;
  padding: 14px 16px;
  border-radius: var(--radius-lg);
  background: var(--raised);
  border: 1px solid var(--line);
  text-align: left;
  transition:
    border-color 140ms,
    transform 140ms var(--ease-out);
}
.tile:hover {
  border-color: var(--line-strong);
  transform: translateY(-1px);
}
.tile-label {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--subtext);
  font-size: var(--text-xs);
  font-weight: 600;
}
.big {
  color: var(--bright);
  font-size: 32px;
  font-weight: 650;
  font-stretch: 125%;
  line-height: 1;
}
.tile-foot {
  margin-top: auto;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.tile-foot strong {
  margin-right: 4px;
  color: var(--bright);
  font-weight: 650;
}
.ms-title {
  color: var(--bright);
  font-size: var(--text-lg);
  font-weight: 650;
  font-stretch: 112%;
}
.meter {
  width: 100%;
  height: 8px;
  margin-top: 6px;
  border-radius: 4px;
  background: var(--surface-0);
  overflow: hidden;
}
.meter-fill {
  display: block;
  height: 100%;
  border-radius: 4px;
  background: var(--accent);
  transition: width 600ms var(--ease-out);
}

.columns {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 360px;
  gap: 32px;
  margin-top: 30px;
}
.block + .block {
  margin-top: 28px;
}
.ask {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 12px 14px;
  border-radius: var(--radius-md);
  background: color-mix(in oklab, var(--attention) 9%, var(--raised));
  border: 1px solid color-mix(in oklab, var(--attention) 28%, transparent);
  text-align: left;
}
.ask + .ask {
  margin-top: 6px;
}
.ask:hover {
  background: color-mix(in oklab, var(--attention) 14%, var(--raised));
}
.ask-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}
.ask-title {
  color: var(--bright);
  font-weight: 600;
}
.ask-msg {
  overflow: hidden;
  color: var(--subtext);
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.calm {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  color: var(--overlay);
  font-size: var(--text-sm);
}
.line {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 32px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  text-align: left;
  font-size: var(--text-sm);
}
.line:hover {
  background: rgb(255 255 255 / 0.03);
}
.line-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.when {
  flex: none;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.foot-note {
  padding: 6px 10px 0;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.pr {
  color: var(--subtext);
}
.pr.draft {
  color: var(--overlay);
}
.ci {
  color: var(--overlay);
  font-size: var(--text-xs);
}
.ci.fail {
  color: var(--fail);
}
.ci.pass {
  color: var(--ok);
}
.done {
  color: var(--st-done);
}
.bullet {
  width: 14px;
  height: 14px;
  display: grid;
  place-items: center;
}
.bullet::after {
  content: "";
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--surface-1);
}
</style>
