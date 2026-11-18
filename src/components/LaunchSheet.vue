<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { FolderGit2, GitBranch, GitBranchPlus, Play } from "lucide-vue-next";
import Modal from "./Modal.vue";
import PickMenu from "./PickMenu.vue";
import AgentIcon from "./AgentIcon.vue";
import StatusIcon from "./StatusIcon.vue";
import { AGENTS, type AgentKind, type AgentOptions, type Effort, type StartRequest } from "../domain/types";
import { byPriority, isOpen, itemsById, project, refreshWorktrees, startSession, store } from "../state/store";
import { api, errorText } from "../lib/api";
import { slugify, tilde } from "../lib/format";

/** Starts an agent (or a plain terminal) on an item, in the main checkout, a new worktree or an existing one. */
const NONE = "__none__";
/** Missions a free session can take instead of an item, in the main checkout, in auto mode. */
const PRESETS = [
  { id: "groom", label: "Trier et organiser" },
  { id: "brainstorm", label: "Brainstormer" },
] as const;
type Preset = (typeof PRESETS)[number]["id"];
const itemId = ref(store.launch?.item ?? NONE);
const agent = ref<AgentKind>((localStorage.getItem("launch.agent") as AgentKind) ?? "claude");
type Place = StartRequest["place"];
const place = ref<Place>(store.launch?.path ? "path" : ((localStorage.getItem(`launch.place.${store.projectId}`) as Place) ?? "main"));
const savedPreset = localStorage.getItem("launch.preset") as Preset | null;
const preset = ref<Preset | null>(savedPreset && PRESETS.some((p) => p.id === savedPreset) ? savedPreset : null);
const branch = ref("");
const path = ref(store.launch?.path ?? "");
const prompt = ref("");
const busy = ref(false);
/** What the agent can run, as its CLI reports it; null keeps the agent's own model and effort. */
const options = ref<AgentOptions | null>(null);
const optionsError = ref("");
const model = ref<string | null>(null);
const effort = ref<Effort | null>(null);
const modelId = computed({
  get: () => model.value ?? NONE,
  set: (id: string) => (model.value = id === NONE ? null : id),
});
const modelOptions = computed(() => [
  { id: NONE, label: "Par défaut" },
  ...(options.value?.models ?? []).map((m) => ({ id: m.id, label: m.label })),
]);
const modelLabel = computed(() => options.value?.models.find((m) => m.id === model.value)?.label ?? "Par défaut");
/** The efforts the chosen model takes, or those of the agent's default model. */
const efforts = computed<Effort[]>(() => {
  const o = options.value;
  if (!o) return [];
  return o.models.find((m) => m.id === model.value)?.efforts ?? o.efforts;
});

function saved(key: string, allowed: string[]) {
  const value = localStorage.getItem(key);
  return value && allowed.includes(value) ? value : null;
}

async function loadOptions(a: AgentKind) {
  options.value = null;
  optionsError.value = "";
  model.value = null;
  effort.value = null;
  if (a === "shell" || a === "game") return;
  try {
    const o = await api.agentOptions(a);
    if (agent.value !== a) return;
    options.value = o;
    model.value = saved(`launch.model.${a}`, o.models.map((m) => m.id));
    effort.value = saved(`launch.effort.${a}`, efforts.value);
  } catch (e) {
    if (agent.value === a) optionsError.value = errorText(e);
  }
}
watch(agent, loadOptions, { immediate: true });
watch(model, () => {
  if (effort.value && !efforts.value.includes(effort.value)) effort.value = null;
});

const item = computed(() => (itemId.value === NONE ? undefined : itemsById.value.get(itemId.value)));
const itemOptions = computed(() => [
  { id: NONE, label: "Sans élément" },
  ...store.items.filter(isOpen).sort(byPriority).map((i) => ({ id: i.id, label: `${i.id} ${i.title}` })),
]);

const others = computed(() => (store.worktrees ?? []).filter((w) => !w.main));
const pathOptions = computed(() => others.value.map((w) => ({ id: w.path, label: w.branch ?? tilde(w.path) })));
const mainPath = computed(() => store.worktrees?.find((w) => w.main)?.path ?? project.value?.path ?? "");
const busyMain = computed(() =>
  store.sessions.filter((s) => s.project === store.projectId && s.status !== "offline" && s.agent !== "shell" && s.agent !== "game" && s.cwd === mainPath.value),
);

/** `jeu-12-armures-slot-de-torse`: the item id leads, so PRs and worktrees link back to it. */
function suggestBranch() {
  const i = item.value;
  branch.value = i ? `${i.id.toLowerCase()}-${slugify(i.title, 36)}` : "";
}
watch(itemId, suggestBranch, { immediate: true });

/** A mission belongs to a session without an item; it works on the roadmap, in the main checkout. */
watch(itemId, (id) => {
  if (id !== NONE) preset.value = null;
});
watch(preset, (p) => {
  if (p) place.value = "main";
});
watch(agent, (a) => {
  if (a === "shell") preset.value = null;
});

const promptPlaceholder = computed(() =>
  item.value
    ? "Consigne en plus de la fiche (facultatif)"
    : preset.value === "groom"
      ? "En plus de la mission (facultatif) : où commencer, quoi laisser de côté"
      : preset.value === "brainstorm"
        ? "Le sujet du brainstorm (facultatif)"
        : "Ce que l'agent doit faire (facultatif)",
);

onMounted(() => {
  if (store.worktrees === null) refreshWorktrees();
  if (place.value === "path" && !path.value) path.value = others.value[0]?.path ?? "";
});

const ready = computed(() => {
  if (!project.value?.present) return false;
  if (place.value === "worktree") return !!branch.value.trim();
  if (place.value === "path") return !!path.value;
  return true;
});

async function launch() {
  if (!ready.value || busy.value) return;
  busy.value = true;
  localStorage.setItem("launch.agent", agent.value);
  if (options.value) {
    localStorage.setItem(`launch.model.${agent.value}`, model.value ?? "");
    localStorage.setItem(`launch.effort.${agent.value}`, effort.value ?? "");
  }
  localStorage.setItem(`launch.place.${store.projectId}`, place.value === "path" ? "main" : place.value);
  localStorage.setItem("launch.preset", !item.value ? (preset.value ?? "") : "");
  await startSession({
    project: store.projectId,
    item: item.value?.id,
    agent: agent.value,
    place: place.value,
    branch: place.value === "worktree" ? branch.value.trim() : undefined,
    path: place.value === "path" ? path.value : undefined,
    prompt: prompt.value,
    preset: !item.value ? preset.value : null,
    model: model.value ?? undefined,
    effort: effort.value ?? undefined,
  });
  busy.value = false;
}
</script>

<template>
  <Modal :width="620" @close="store.launch = null">
    <form class="sheet" @submit.prevent="launch" @keydown.meta.enter.prevent="launch">
      <header>
        <h2>Lancer un agent</h2>
        <span class="muted">{{ project?.name }}</span>
      </header>

      <div class="body">
        <div class="row">
          <span class="label">Sur</span>
          <PickMenu v-model="itemId" :options="itemOptions" filterable>
            <template #default="{ toggle }">
              <button type="button" class="pick-item" @click="toggle">
                <template v-if="item">
                  <StatusIcon :status="item.status" />
                  <span class="code id">{{ item.id }}</span>
                  <span class="t">{{ item.title }}</span>
                </template>
                <span v-else class="muted">Sans élément : une session libre</span>
              </button>
            </template>
          </PickMenu>
        </div>

        <div v-if="!item && agent !== 'shell'" class="row">
          <span class="label">Mission</span>
          <div class="segmented">
            <button type="button" :class="{ on: !preset }" @click="preset = null">Libre</button>
            <button v-for="p in PRESETS" :key="p.id" type="button" :class="{ on: preset === p.id }" @click="preset = p.id">
              {{ p.label }}
            </button>
          </div>
        </div>

        <div class="row">
          <span class="label">Avec</span>
          <div class="segmented">
            <button v-for="a in AGENTS" :key="a.id" type="button" :class="{ on: agent === a.id }" @click="agent = a.id">
              <AgentIcon :agent="a.id" :size="13" />{{ a.label }}
            </button>
          </div>
        </div>

        <div v-if="options?.models.length" class="row">
          <span class="label">Modèle</span>
          <PickMenu v-model="modelId" :options="modelOptions" filterable>
            <template #default="{ toggle }">
              <button type="button" class="btn btn-quiet btn-sm code" @click="toggle">{{ modelLabel }}</button>
            </template>
          </PickMenu>
        </div>

        <div v-if="efforts.length" class="row">
          <span class="label">Effort</span>
          <div class="segmented">
            <button type="button" :class="{ on: !effort }" @click="effort = null">Par défaut</button>
            <button v-for="e in efforts" :key="e" type="button" :class="{ on: effort === e }" @click="effort = e">{{ e }}</button>
          </div>
        </div>

        <div class="places">
          <label class="place" :class="{ on: place === 'main' }">
            <input v-model="place" type="radio" value="main" />
            <FolderGit2 :size="16" />
            <span class="what">
              <strong>Dépôt principal</strong>
              <span class="muted code">{{ tilde(mainPath) }}</span>
              <span v-if="busyMain.length" class="warn">{{ busyMain.length }} agent(s) y travaillent déjà</span>
            </span>
          </label>
          <label class="place" :class="{ on: place === 'worktree' }">
            <input v-model="place" type="radio" value="worktree" />
            <GitBranchPlus :size="16" />
            <span class="what">
              <strong>Nouveau worktree</strong>
              <input
                v-model="branch"
                class="field code branch"
                placeholder="nom-de-branche"
                spellcheck="false"
                @focus="place = 'worktree'"
              />
              <span class="muted small">Depuis {{ project?.base }}{{ project?.setup ? `, puis ${project.setup}` : "" }}</span>
            </span>
          </label>
          <label v-if="others.length" class="place" :class="{ on: place === 'path' }">
            <input v-model="place" type="radio" value="path" />
            <GitBranch :size="16" />
            <span class="what">
              <strong>Worktree existant</strong>
              <PickMenu v-model="path" :options="pathOptions">
                <template #default="{ toggle }">
                  <button type="button" class="btn btn-quiet btn-sm code" @click="(place = 'path'), toggle()">
                    {{ pathOptions.find((o) => o.id === path)?.label ?? "Choisir…" }}
                  </button>
                </template>
              </PickMenu>
            </span>
          </label>
        </div>

        <textarea
          v-if="agent !== 'shell'"
          v-model="prompt"
          class="field"
          rows="3"
          :placeholder="promptPlaceholder"
        />
        <p v-if="optionsError" class="error">{{ optionsError }}</p>
        <p v-if="!project?.present" class="error">{{ project?.name }} n'est pas cloné dans {{ project?.path }} sur ce Mac : règle le chemin dans les réglages.</p>
      </div>

      <footer>
        <span class="muted small"><kbd>⌘</kbd><kbd>↵</kbd> lance</span>
        <button type="button" class="btn btn-ghost" @click="store.launch = null">Annuler</button>
        <button type="submit" class="btn btn-solid" :disabled="!ready || busy"><Play />{{ busy ? "Lancement…" : "Lancer" }}</button>
      </footer>
    </form>
  </Modal>
</template>

<style scoped>
.sheet {
  display: flex;
  flex-direction: column;
}
header {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 18px 22px 4px;
}
header h2 {
  font-size: var(--text-lg);
}
.body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px 22px 18px;
}
.row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.row .label {
  width: 56px;
  margin: 0;
}
.pick-item {
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 500px;
  height: 32px;
  padding: 0 12px;
  border-radius: var(--radius-sm);
  background: var(--surface-0);
}
.pick-item:hover {
  background: var(--surface-1);
}
.id {
  color: var(--overlay);
  font-size: var(--text-xs);
}
.t {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.places {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.place {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--radius-md);
  border: 1px solid var(--line-strong);
  cursor: pointer;
  transition:
    border-color 120ms,
    background 120ms;
}
.place:hover {
  border-color: var(--surface-1);
}
.place.on {
  border-color: var(--accent-line);
  background: var(--accent-soft);
}
.place > svg {
  margin-top: 1px;
  color: var(--subtext);
}
.place.on > svg {
  color: var(--accent);
}
.place input[type="radio"] {
  display: none;
}
.what {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  flex: 1;
  min-width: 0;
}
.what strong {
  font-weight: 600;
}
.branch {
  height: 30px;
}
.small {
  font-size: var(--text-xs);
}
.warn {
  color: var(--pr-high);
  font-size: var(--text-xs);
}
.error {
  color: var(--fail);
  font-size: var(--text-sm);
}
footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px 12px 22px;
  border-top: 1px solid var(--line);
}
footer .muted {
  flex: 1;
}
</style>
