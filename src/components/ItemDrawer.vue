<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Copy, ExternalLink, GitBranch, GitMerge, GitPullRequest, Pencil, Play, Plus, Trash2, X } from "lucide-vue-next";
import FieldPick from "./FieldPick.vue";
import Markdown from "./Markdown.vue";
import StatusDot from "./StatusDot.vue";
import AgentIcon from "./AgentIcon.vue";
import { confirmAsk, deleteItem, go, itemsById, notify, now, project, store, updateItem } from "../state/store";
import { prMentions, sessionMentions, worktreeMentions } from "../lib/links";
import { dateLabel, since, tilde } from "../lib/format";
import { copy, openExternal } from "../lib/native";

const item = computed(() => (store.drawer ? itemsById.value.get(store.drawer) : undefined));
const key = computed(() => project.value?.key ?? "");

const sessions = computed(() => (item.value ? store.sessions.filter((s) => sessionMentions(s, item.value!.id, key.value)) : []));
const worktrees = computed(() =>
  item.value ? (store.worktrees ?? []).filter((w) => !w.main && worktreeMentions(w, item.value!.id, key.value)) : [],
);
const prs = computed(() => (item.value ? (store.prs ?? []).filter((p) => prMentions(p, item.value!.id, key.value)) : []));

// Title: saved on Enter or when leaving the field.
const title = ref("");
const titleEl = ref<HTMLTextAreaElement>();
/** Grows the title field with its text. */
function fitTitle() {
  const el = titleEl.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${el.scrollHeight}px`;
}
watch(
  item,
  (i) => {
    if (i && document.activeElement?.id !== "drawer-title") title.value = i.title;
    nextTick(fitTitle);
  },
  { immediate: true },
);
function saveTitle() {
  const t = title.value.trim();
  if (item.value && t && t !== item.value.title) updateItem(item.value.id, { title: t });
  else if (item.value) title.value = item.value.title;
}

// Labels.
const labelInput = ref("");
function addLabel() {
  const l = labelInput.value.trim();
  if (!item.value || !l || item.value.labels.includes(l)) return (labelInput.value = "");
  updateItem(item.value.id, { labels: [...item.value.labels, l] });
  labelInput.value = "";
}
function removeLabel(l: string) {
  if (item.value) updateItem(item.value.id, { labels: item.value.labels.filter((x) => x !== l) });
}

// Description: edited from the version it was opened at, so an agent's note added meanwhile is never overwritten.
const editing = ref(false);
const body = ref("");
const base = ref("");
async function editBody() {
  if (!item.value) return;
  body.value = item.value.body;
  base.value = item.value.updated;
  editing.value = true;
  await nextTick();
  document.querySelector<HTMLTextAreaElement>(".body-field")?.focus();
}
async function saveBody() {
  if (!item.value) return;
  const saved = await updateItem(item.value.id, { body: body.value, baseUpdated: base.value });
  if (saved) editing.value = false;
}
watch(
  () => store.drawer,
  () => (editing.value = false),
);

async function remove() {
  const i = item.value;
  if (i && (await confirmAsk(`Supprimer ${i.id} « ${i.title} » ? Le fichier reste dans l'historique git des données.`, "Supprimer")))
    deleteItem(i.id);
}

function copyId() {
  if (!item.value) return;
  copy(item.value.id);
  notify(`${item.value.id} copié`);
}

function close() {
  if (editing.value && body.value !== item.value?.body) {
    saveBody();
    return;
  }
  store.drawer = null;
}
</script>

<template>
  <Transition name="drawer">
    <aside v-if="item" :key="'drawer'" class="drawer" @keydown.esc.stop="close">
      <header class="top">
        <button class="id code" title="Copier l'identifiant" @click="copyId">{{ item.id }}<Copy :size="12" /></button>
        <span class="grow" />
        <button class="btn btn-solid btn-sm" @click="store.launch = { item: item.id }"><Play />Lancer un agent</button>
        <button class="btn btn-ghost btn-sm btn-icon" title="Supprimer" @click="remove"><Trash2 /></button>
        <button class="btn btn-ghost btn-sm btn-icon" title="Fermer (Échap)" @click="close"><X /></button>
      </header>

      <div class="scroll">
        <textarea
          id="drawer-title"
          ref="titleEl"
          v-model="title"
          class="title"
          rows="1"
          spellcheck="false"
          @keydown.enter.prevent="($event.target as HTMLElement).blur()"
          @input="fitTitle"
          @blur="saveTitle"
        />

        <div class="props">
          <FieldPick :item="item" field="status" />
          <FieldPick :item="item" field="priority" />
          <FieldPick :item="item" field="kind" />
          <FieldPick :item="item" field="milestone" />
        </div>
        <div class="labels">
          <span v-for="l in item.labels" :key="l" class="chip">
            {{ l }}<button class="x" title="Retirer" @click="removeLabel(l)"><X :size="10" /></button>
          </span>
          <label class="add-label">
            <Plus :size="12" />
            <input v-model="labelInput" placeholder="label" @keydown.enter.prevent="addLabel" @blur="addLabel" />
          </label>
        </div>

        <section v-if="sessions.length || worktrees.length || prs.length" class="work">
          <h3 class="section-title">Travail en cours</h3>
          <button v-for="s in sessions" :key="s.id" class="link" @click="go({ name: 'agents', session: s.id })">
            <StatusDot :status="s.status" />
            <AgentIcon :agent="s.agent" :size="13" />
            <span class="what">{{ s.agent === "shell" ? "Terminal" : s.agent }} <span class="muted">{{ tilde(s.cwd) }}</span></span>
            <span class="when">{{ since(s.statusAt, now) }}</span>
          </button>
          <div v-for="w in worktrees" :key="w.path" class="link static">
            <GitBranch :size="14" class="muted" />
            <span class="what code">{{ w.branch }}</span>
            <span v-if="w.dirty" class="chip">{{ w.dirty }} modif.</span>
            <span v-if="w.ahead" class="chip">+{{ w.ahead }}</span>
          </div>
          <button v-for="p in prs" :key="p.number" class="link" @click="openExternal(p.url)">
            <GitMerge v-if="p.state === 'merged'" :size="14" class="merged" />
            <GitPullRequest v-else :size="14" :class="p.state" />
            <span class="what">#{{ p.number }} {{ p.title }}</span>
            <span class="chip" :class="p.checks">{{ { pass: "CI ok", fail: "CI en échec", pending: "CI en cours", none: "pas de CI" }[p.checks] }}</span>
            <ExternalLink :size="12" class="muted" />
          </button>
        </section>

        <section class="desc">
          <h3 class="section-title">
            Description
            <button v-if="!editing" class="btn btn-ghost btn-sm" @click="editBody"><Pencil />Modifier</button>
          </h3>
          <template v-if="editing">
            <textarea
              v-model="body"
              class="field body-field"
              rows="14"
              placeholder="Contexte, critère « fait quand », pistes… (Markdown)"
              @keydown.meta.enter="saveBody"
              @keydown.esc.stop="editing = false"
            />
            <div class="form-actions">
              <span class="muted"><kbd>⌘</kbd><kbd>↵</kbd> enregistre, <kbd>Échap</kbd> annule</span>
              <button class="btn btn-ghost" @click="editing = false">Annuler</button>
              <button class="btn btn-solid" @click="saveBody">Enregistrer</button>
            </div>
          </template>
          <Markdown v-else-if="item.body" :source="item.body" @dblclick="editBody" />
          <button v-else class="placeholder" @click="editBody">Ajouter une description…</button>
        </section>
      </div>

      <footer class="foot">
        <span>Créé le {{ dateLabel(item.created) }}</span>
        <span>Modifié {{ since(item.updated, now) }}</span>
        <span v-if="item.closed">Clos le {{ dateLabel(item.closed) }}</span>
      </footer>
    </aside>
  </Transition>
</template>

<style scoped>
.drawer {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 30;
  display: flex;
  flex-direction: column;
  width: var(--drawer-width);
  max-width: 70%;
  background: var(--base);
  border-left: 1px solid var(--line-strong);
  box-shadow: -24px 0 60px -20px rgb(0 0 0 / 0.6);
}
.top {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 52px;
  padding: 0 14px 0 20px;
  border-bottom: 1px solid var(--line);
}
.id {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--subtext);
  font-size: var(--text-sm);
}
.id svg {
  opacity: 0;
  transition: opacity 120ms;
}
.id:hover svg {
  opacity: 0.7;
}
.grow {
  flex: 1;
}
.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 20px 24px 32px;
}
.title {
  width: 100%;
  padding: 2px 0;
  border: 0;
  background: none;
  outline: none;
  resize: none;
  overflow: hidden;
  color: var(--bright);
  font-size: var(--text-xl);
  font-weight: 650;
  font-stretch: 112%;
  line-height: 1.25;
  user-select: text;
}
.props {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 14px;
}
.labels {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
}
.labels .chip {
  padding-right: 3px;
}
.x {
  display: grid;
  place-items: center;
  width: 15px;
  height: 15px;
  border-radius: 50%;
  color: var(--overlay);
}
.x:hover {
  background: var(--surface-1);
  color: var(--text);
}
.add-label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--overlay);
}
.add-label input {
  width: 80px;
  border: 0;
  background: none;
  outline: none;
  font-size: var(--text-xs);
}
.work {
  margin-top: 24px;
}
.link {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  min-height: 34px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  background: var(--raised);
  text-align: left;
  font-size: var(--text-sm);
}
.link + .link {
  margin-top: 4px;
}
button.link:hover {
  background: var(--surface-0);
}
.what {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.when {
  color: var(--overlay);
  font-size: var(--text-xs);
}
.merged {
  color: var(--review);
}
.open {
  color: var(--ok);
}
.draft {
  color: var(--overlay);
}
.chip.fail {
  color: var(--fail);
}
.chip.pass {
  color: var(--ok);
}
.desc {
  margin-top: 26px;
}
.desc .section-title {
  justify-content: space-between;
  min-height: 26px;
}
.body-field {
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
.placeholder {
  width: 100%;
  padding: 14px;
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  color: var(--overlay);
  text-align: left;
}
.placeholder:hover {
  border-color: var(--accent-line);
  color: var(--subtext);
}
.foot {
  display: flex;
  gap: 16px;
  padding: 10px 24px;
  border-top: 1px solid var(--line);
  color: var(--overlay);
  font-size: var(--text-xs);
}
.drawer-enter-active,
.drawer-leave-active {
  transition:
    transform 220ms var(--ease-out),
    opacity 160ms ease;
}
.drawer-enter-from,
.drawer-leave-to {
  transform: translateX(40px);
  opacity: 0;
}
</style>
