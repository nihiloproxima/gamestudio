<script setup lang="ts">
import { computed, ref } from "vue";
import { Network } from "lucide-vue-next";
import Modal from "./Modal.vue";
import { activeMilestone, isOpen, orchestrate, project, store } from "../state/store";

/** Starts the orchestrator: Claude Opus picks what can move in parallel, delegates it to agents and merges it. */
const CHOICES = [2, 3, 4, 5];
const saved = Number(localStorage.getItem("orchestrate.parallel"));
const parallel = ref(CHOICES.includes(saved) ? saved : 3);
const prompt = ref("");
const busy = ref(false);

/** What it would pick from without a consigne: planned tasks and bugs nobody has started. */
const todo = computed(() => store.items.filter((i) => isOpen(i) && i.status === "todo" && i.kind !== "idea"));
const inMilestone = computed(() => todo.value.filter((i) => i.milestone && i.milestone === activeMilestone.value?.id).length);
const base = computed(() => project.value?.base.replace(/^origin\//, "") ?? "main");

async function launch() {
  if (!project.value?.present || busy.value) return;
  busy.value = true;
  localStorage.setItem("orchestrate.parallel", String(parallel.value));
  await orchestrate(parallel.value, prompt.value);
  busy.value = false;
}
</script>

<template>
  <Modal :width="580" @close="store.orchestrate = false">
    <form class="sheet" @submit.prevent="launch" @keydown.meta.enter.prevent="launch">
      <header>
        <h2>Orchestrer</h2>
        <span class="muted">{{ project?.name }}</span>
      </header>

      <div class="body">
        <p class="intro">
          Claude Opus, en effort xhigh, prend dans la roadmap ce qui peut avancer en parallèle et le confie à des agents,
          chacun dans son worktree. Il relit leur travail, le fusionne sur une branche d'intégration, résout les conflits,
          puis ouvre une PR vers <span class="code">{{ base }}</span>, sans jamais y pousser.
        </p>
        <p class="intro muted">
          Tout tourne en mode auto : les agents ne te demandent rien, sauf ce que Claude refuse de faire seul. Ce qui
          demande ta décision est laissé de côté, avec une note.
        </p>

        <div class="row">
          <span class="label">Agents en parallèle</span>
          <div class="segmented">
            <button v-for="n in CHOICES" :key="n" type="button" :class="{ on: parallel === n }" @click="parallel = n">{{ n }}</button>
          </div>
        </div>

        <p class="pool">
          <Network :size="14" />
          <span v-if="todo.length">
            {{ todo.length }} tâche{{ todo.length > 1 ? "s" : "" }} ou bug{{ todo.length > 1 ? "s" : "" }} à faire<template v-if="activeMilestone">, dont {{ inMilestone }} dans {{ activeMilestone.title }}</template>.
          </span>
          <span v-else>Rien à faire pour l'instant : il piochera dans le backlog du jalon en cours.</span>
        </p>

        <textarea
          v-model="prompt"
          class="field"
          rows="3"
          placeholder="Consigne (facultatif) : un périmètre, des priorités, ce qu'il ne faut pas toucher…"
        />
        <p v-if="!project?.present" class="error">{{ project?.name }} n'est pas cloné dans {{ project?.path }} sur ce Mac : règle le chemin dans les réglages.</p>
      </div>

      <footer>
        <span class="muted small"><kbd>⌘</kbd><kbd>↵</kbd> lance</span>
        <button type="button" class="btn btn-ghost" @click="store.orchestrate = false">Annuler</button>
        <button type="submit" class="btn btn-solid" :disabled="!project?.present || busy"><Network />{{ busy ? "Lancement…" : "Lancer" }}</button>
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
.intro {
  font-size: var(--text-sm);
  line-height: 1.5;
}
.row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.row .label {
  margin: 0;
}
.pool {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--subtext);
  font-size: var(--text-sm);
}
.pool svg {
  flex: none;
  color: var(--accent);
}
.small {
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
