<script setup lang="ts">
import { computed, nextTick, reactive, ref } from "vue";
import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, Map, Pencil, Plus, Trash2 } from "lucide-vue-next";
import ItemRow from "../components/ItemRow.vue";
import Markdown from "../components/Markdown.vue";
import { MILESTONE_STATUSES, label, type Milestone, type MilestoneStatus } from "../domain/types";
import { byPriority, confirmAsk, deleteMilestone, now, progressOf, project, saveMilestone, saveProject, store } from "../state/store";
import { dateLabel, daysUntil } from "../lib/format";

const open = reactive(new Set<string>());
const editing = ref<string | null>(null);
const draft = reactive({ title: "", status: "planned" as MilestoneStatus, target: "", goal: "" });

function edit(m: Milestone | null) {
  editing.value = m?.id ?? "new";
  Object.assign(draft, { title: m?.title ?? "", status: m?.status ?? "planned", target: m?.target ?? "", goal: m?.goal ?? "" });
  nextTick(() => document.querySelector<HTMLInputElement>(".ms-form input")?.focus());
}

async function save(m: Milestone | null) {
  if (!draft.title.trim()) return;
  const order = m?.order ?? Math.max(-1, ...store.milestones.map((x) => x.order)) + 1;
  const saved = await saveMilestone(m?.id ?? null, {
    title: draft.title.trim(),
    status: draft.status,
    target: draft.target || undefined,
    order,
    goal: draft.goal,
  });
  if (saved) editing.value = null;
}

async function remove(m: Milestone) {
  const { total } = progressOf(m.id);
  const detail = total ? ` Ses ${total} éléments restent dans le tracker, sans jalon.` : "";
  if (!(await confirmAsk(`Supprimer le jalon « ${m.title} » ?${detail}`, "Supprimer"))) return;
  await deleteMilestone(m.id);
  editing.value = null;
}

/** Swaps orders with the neighbour; orders are rewritten densely so ties never stick. */
async function shift(m: Milestone, dir: -1 | 1) {
  const list = [...store.milestones];
  const i = list.findIndex((x) => x.id === m.id);
  const j = i + dir;
  if (j < 0 || j >= list.length) return;
  [list[i], list[j]] = [list[j], list[i]];
  for (const [order, x] of list.entries()) {
    if (x.order !== order) await saveMilestone(x.id, { ...x, order });
  }
}

async function setStatus(m: Milestone, status: MilestoneStatus) {
  await saveMilestone(m.id, { ...m, status });
}

function itemsOf(id: string) {
  return store.items
    .filter((i) => i.milestone === id && i.status !== "dropped")
    .sort((a, b) => Number(a.status === "done") - Number(b.status === "done") || byPriority(a, b));
}

const unplanned = computed(() => store.items.filter((i) => !i.milestone && ["todo", "doing", "review"].includes(i.status)).length);

function due(m: Milestone) {
  if (!m.target || m.status === "done") return "";
  void now.value;
  const d = daysUntil(m.target);
  if (d < 0) return `en retard de ${-d} j`;
  if (d === 0) return "aujourd'hui";
  return `dans ${d} j`;
}

// The game's pitch and objectives, edited in place.
const visionEditing = ref(false);
const visionDraft = ref("");
function editVision() {
  visionDraft.value = project.value?.vision ?? "";
  visionEditing.value = true;
}
async function saveVision() {
  const p = project.value;
  if (!p) return;
  const { id, vision: _, present: __, ...meta } = p;
  if (await saveProject(id, meta, visionDraft.value)) visionEditing.value = false;
}
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Roadmap</h1>
      <span class="spacer" />
      <button class="btn btn-solid" @click="edit(null)"><Plus />Nouveau jalon</button>
    </header>

    <div class="view-body">
      <section v-if="project" class="vision">
        <header>
          <h2>Le cap</h2>
          <button v-if="!visionEditing" class="btn btn-ghost btn-sm" @click="editVision"><Pencil />Modifier</button>
        </header>
        <template v-if="visionEditing">
          <textarea
            v-model="visionDraft"
            class="field"
            rows="6"
            placeholder="Le pitch du jeu, ses objectifs, ce qu'on vise pour la prochaine version…"
            @keydown.meta.enter="saveVision"
            @keydown.esc="visionEditing = false"
          />
          <div class="form-actions">
            <span class="muted">Markdown. <kbd>⌘</kbd><kbd>↵</kbd> enregistre.</span>
            <button class="btn btn-ghost" @click="visionEditing = false">Annuler</button>
            <button class="btn btn-solid" @click="saveVision">Enregistrer</button>
          </div>
        </template>
        <Markdown v-else-if="project.vision" :source="project.vision" />
        <p v-else class="muted">Écris le pitch et les objectifs du jeu : c'est ce que tout le reste sert.</p>
      </section>

      <div v-if="!store.milestones.length && editing !== 'new'" class="empty">
        <Map />
        <p>Pas encore de jalon. Découpe le chemin jusqu'à la prochaine version en étapes qu'on peut atteindre.</p>
        <button class="btn btn-quiet" @click="edit(null)"><Plus />Premier jalon</button>
      </div>

      <ol class="track">
        <li v-for="(m, index) in store.milestones" :key="m.id" class="ms" :class="m.status">
          <span class="node" />
          <form v-if="editing === m.id" class="ms-form" @submit.prevent="save(m)">
            <input v-model="draft.title" class="field title-field" placeholder="Nom du jalon" />
            <div class="form-row">
              <div class="segmented">
                <button v-for="s in MILESTONE_STATUSES" :key="s.id" type="button" :class="{ on: draft.status === s.id }" @click="draft.status = s.id">
                  {{ s.label }}
                </button>
              </div>
              <label class="date">Échéance <input v-model="draft.target" type="date" class="field" /></label>
            </div>
            <textarea v-model="draft.goal" class="field" rows="4" placeholder="Fait quand… (Markdown)" @keydown.meta.enter="save(m)" />
            <div class="form-actions">
              <button type="button" class="btn btn-danger" @click="remove(m)"><Trash2 />Supprimer</button>
              <span class="grow" />
              <button type="button" class="btn btn-ghost" @click="editing = null">Annuler</button>
              <button type="submit" class="btn btn-solid">Enregistrer</button>
            </div>
          </form>
          <div v-else class="ms-body">
            <header class="ms-head">
              <h3>{{ m.title }}</h3>
              <span class="state" :class="m.status">{{ label(MILESTONE_STATUSES, m.status) }}</span>
              <span v-if="m.target" class="target">{{ dateLabel(m.target) }}<em v-if="due(m)" :class="{ late: due(m).startsWith('en retard') }">{{ due(m) }}</em></span>
              <span class="grow" />
              <div class="tools">
                <button v-if="m.status === 'planned'" class="btn btn-ghost btn-sm" @click="setStatus(m, 'active')">Démarrer</button>
                <button v-if="m.status === 'active'" class="btn btn-ghost btn-sm" @click="setStatus(m, 'done')">Marquer atteint</button>
                <button class="btn btn-ghost btn-sm btn-icon" title="Monter" :disabled="index === 0" @click="shift(m, -1)"><ArrowUp /></button>
                <button class="btn btn-ghost btn-sm btn-icon" title="Descendre" :disabled="index === store.milestones.length - 1" @click="shift(m, 1)"><ArrowDown /></button>
                <button class="btn btn-ghost btn-sm btn-icon" title="Modifier" @click="edit(m)"><Pencil /></button>
              </div>
            </header>
            <Markdown v-if="m.goal" :source="m.goal" class="goal" />
            <button v-if="progressOf(m.id).total" class="progress" @click="open.has(m.id) ? open.delete(m.id) : open.add(m.id)">
              <span class="bar"><span class="fill" :style="{ width: `${progressOf(m.id).ratio * 100}%` }" /></span>
              <span class="nums">{{ progressOf(m.id).done }} / {{ progressOf(m.id).total }}</span>
              <component :is="open.has(m.id) ? ChevronDown : ChevronRight" :size="14" class="chev" />
            </button>
            <button v-else class="btn btn-ghost btn-sm add-first" @click="store.capture = { kind: 'task', milestone: m.id }">
              <Plus />Ajouter une tâche
            </button>
            <div v-if="open.has(m.id)" class="items">
              <ItemRow v-for="i in itemsOf(m.id)" :key="i.id" :item="i" />
              <button class="btn btn-ghost btn-sm add-here" @click="store.capture = { kind: 'task', milestone: m.id }">
                <Plus />Ajouter une tâche à ce jalon
              </button>
            </div>
          </div>
        </li>

        <li v-if="editing === 'new'" class="ms planned">
          <span class="node" />
          <form class="ms-form" @submit.prevent="save(null)">
            <input v-model="draft.title" class="field title-field" placeholder="Nom du jalon, Démo Steam par exemple" />
            <div class="form-row">
              <div class="segmented">
                <button v-for="s in MILESTONE_STATUSES" :key="s.id" type="button" :class="{ on: draft.status === s.id }" @click="draft.status = s.id">
                  {{ s.label }}
                </button>
              </div>
              <label class="date">Échéance <input v-model="draft.target" type="date" class="field" /></label>
            </div>
            <textarea v-model="draft.goal" class="field" rows="4" placeholder="Fait quand… (Markdown)" @keydown.meta.enter="save(null)" />
            <div class="form-actions">
              <span class="grow" />
              <button type="button" class="btn btn-ghost" @click="editing = null">Annuler</button>
              <button type="submit" class="btn btn-solid">Créer le jalon</button>
            </div>
          </form>
        </li>
      </ol>

      <p v-if="unplanned" class="unplanned">
        {{ unplanned }} tâche{{ unplanned > 1 ? "s" : "" }} prévue{{ unplanned > 1 ? "s" : "" }} sans jalon.
      </p>
    </div>
  </div>
</template>

<style scoped>
.vision {
  max-width: 860px;
  margin-bottom: 34px;
  padding: 18px 20px;
  border-radius: var(--radius-lg);
  background:
    radial-gradient(120% 140% at 100% 0%, var(--accent-soft), transparent 60%),
    var(--raised);
  border: 1px solid var(--line);
}
.vision header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}
.vision h2 {
  font-size: var(--text-md);
}
.form-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.form-actions .muted {
  flex: 1;
  font-size: var(--text-xs);
}
.grow {
  flex: 1;
}

.track {
  max-width: 860px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.ms {
  position: relative;
  padding: 0 0 30px 34px;
}
/* The rail: solid up to the active milestone, dashed beyond. */
.ms::before {
  content: "";
  position: absolute;
  top: 18px;
  bottom: -2px;
  left: 7px;
  border-left: 2px solid var(--accent-line);
}
.ms.planned::before {
  border-left: 2px dashed var(--line-strong);
}
.ms:last-child::before {
  display: none;
}
.node {
  position: absolute;
  top: 4px;
  left: 0;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid var(--surface-1);
  background: var(--base);
}
.ms.done .node {
  border-color: var(--accent);
  background: var(--accent);
}
.ms.active .node {
  border-color: var(--accent);
  box-shadow:
    0 0 0 4px var(--accent-soft),
    0 0 18px color-mix(in oklab, var(--accent) 50%, transparent);
}
.ms.active .node::after {
  content: "";
  position: absolute;
  inset: 3px;
  border-radius: 50%;
  background: var(--accent);
}
.ms-head {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 24px;
}
.ms-head h3 {
  font-size: var(--text-lg);
}
.ms.done h3 {
  color: var(--subtext);
}
.state {
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--surface-0);
  color: var(--subtext);
  font-size: var(--text-2xs);
  font-weight: 650;
}
.state.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.target {
  display: inline-flex;
  gap: 6px;
  color: var(--subtext);
  font-size: var(--text-xs);
}
.target em {
  color: var(--overlay);
  font-style: normal;
}
.target em.late {
  color: var(--fail);
}
.tools {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity 120ms;
}
.ms:hover .tools {
  opacity: 1;
}
.goal {
  margin-top: 6px;
  color: var(--subtext);
  font-size: var(--text-sm);
}
.progress {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  max-width: 420px;
  margin-top: 10px;
  padding: 4px 0;
}
.bar {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: var(--surface-0);
  overflow: hidden;
}
.fill {
  display: block;
  height: 100%;
  border-radius: 3px;
  background: linear-gradient(90deg, color-mix(in oklab, var(--accent) 70%, transparent), var(--accent));
  transition: width 500ms var(--ease-out);
}
.nums {
  color: var(--subtext);
  font-size: var(--text-xs);
  font-weight: 600;
}
.chev {
  color: var(--overlay);
}
.items {
  margin-top: 8px;
  margin-left: -6px;
}
.add-first {
  margin: 8px 0 0 -9px;
}
.add-here {
  margin: 4px 0 0 6px;
}
.ms-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px;
  border-radius: var(--radius-md);
  background: var(--raised);
  border: 1px solid var(--line-strong);
}
.title-field {
  height: 36px;
  font-size: var(--text-lg);
  font-weight: 600;
}
.form-row {
  display: flex;
  align-items: center;
  gap: 14px;
}
.date {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--subtext);
  font-size: var(--text-xs);
}
.date .field {
  width: 150px;
  color-scheme: dark;
}
.unplanned {
  margin-left: 34px;
  color: var(--overlay);
  font-size: var(--text-sm);
}
</style>
