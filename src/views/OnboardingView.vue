<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Download, Gamepad2, Sparkles } from "lucide-vue-next";
import { boot, fail, saveProject, store } from "../state/store";
import { api } from "../lib/api";

/** First launch: start the data on this machine or bring it from GitHub, then add a first game. */
const url = ref("");
const busy = ref(false);

async function start(cloneUrl?: string) {
  busy.value = true;
  try {
    store.sync = await api.dataInit(cloneUrl);
    await boot();
  } catch (e) {
    fail(e, cloneUrl ? "Récupération" : "Création");
  } finally {
    busy.value = false;
  }
}

// First game: a name, a key for its items, where its repo is. GitHub and the base branch come from the repo.
const name = ref("");
const key = ref("");
const path = ref("~/code/");
const keyTouched = ref(false);
watch(name, (n) => {
  if (!keyTouched.value) key.value = n.normalize("NFD").replace(/[^A-Za-z]/g, "").slice(0, 3).toUpperCase();
});
const ready = computed(() => name.value.trim() && /^[A-Z]{2,5}$/.test(key.value) && path.value.trim().length > 7);

async function addGame() {
  if (!ready.value) return;
  busy.value = true;
  const saved = await saveProject(
    null,
    { name: name.value.trim(), key: key.value, color: "#89b4fa", path: path.value.trim(), github: "", base: "", assets: ["assets"], setup: "" },
    "",
  );
  if (saved) await boot();
  busy.value = false;
}
</script>

<template>
  <div class="onboarding" data-tauri-drag-region>
    <div class="card">
      <div class="gem">
        <span style="--c: #ff79c6" /><span style="--c: #8bd5ca" /><span style="--c: #ef9f76" />
      </div>
      <h1>Gamestudio</h1>

      <template v-if="!store.sync?.initialized">
        <p class="lead">Roadmaps, tâches, idées, agents et assets de tes jeux, au même endroit, synchronisés par GitHub.</p>

        <button class="choice" :disabled="busy" @click="start()">
          <Sparkles :size="18" />
          <span>
            <strong>Commencer sur ce Mac</strong>
            <span>Les données vivent dans ~/.gamestudio/data. La synchro GitHub se branche ensuite dans les réglages.</span>
          </span>
        </button>

        <form class="choice" @submit.prevent="url.trim() && start(url.trim())">
          <Download :size="18" />
          <span class="grow">
            <strong>Récupérer depuis GitHub</strong>
            <span>Les données d'un autre Mac, depuis leur dépôt privé.</span>
            <span class="inline">
              <input v-model="url" class="field code" placeholder="git@github.com:compte/gamestudio-data.git" spellcheck="false" />
              <button type="submit" class="btn btn-solid" :disabled="busy || !url.trim()">Récupérer</button>
            </span>
          </span>
        </form>
      </template>

      <form v-else class="first" @submit.prevent="addGame">
        <p class="lead">Ton premier jeu. Les autres s'ajoutent ensuite dans les réglages, et Claude peut remplir la roadmap à partir du dépôt.</p>
        <div class="two">
          <label><span class="label">Nom</span><input v-model="name" class="field" placeholder="Mon jeu" autofocus /></label>
          <label>
            <span class="label">Clé des éléments</span>
            <input v-model="key" class="field code" maxlength="5" placeholder="JEU" @input="(keyTouched = true), (key = key.toUpperCase())" />
          </label>
        </div>
        <label>
          <span class="label">Dépôt sur ce Mac</span>
          <input v-model="path" class="field code" placeholder="~/code/mon-jeu" spellcheck="false" />
        </label>
        <p class="hint">
          <Gamepad2 :size="14" />Les éléments seront numérotés {{ key || "JEU" }}-1, {{ key || "JEU" }}-2… Le dépôt GitHub et la
          branche de base sont lus dans le dépôt.
        </p>
        <button type="submit" class="btn btn-solid" :disabled="busy || !ready">Ajouter le jeu</button>
      </form>
    </div>
  </div>
</template>

<style scoped>
.onboarding {
  display: grid;
  place-items: center;
  height: 100%;
  background:
    radial-gradient(60% 50% at 30% 20%, rgb(255 121 198 / 0.08), transparent),
    radial-gradient(60% 50% at 70% 80%, rgb(139 213 202 / 0.08), transparent),
    var(--mantle);
}
.card {
  width: 560px;
  padding: 36px;
  border-radius: 20px;
  background: var(--raised);
  box-shadow: var(--shadow-pop);
}
.gem {
  display: flex;
  gap: 6px;
  margin-bottom: 18px;
}
.gem span {
  width: 14px;
  height: 14px;
  border-radius: 4px;
  background: var(--c);
  transform: rotate(45deg);
}
h1 {
  font-size: var(--text-2xl);
}
.lead {
  margin: 8px 0 26px;
  color: var(--subtext);
  line-height: 1.55;
}
.choice {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  width: 100%;
  padding: 16px;
  border-radius: var(--radius-md);
  border: 1px solid var(--line-strong);
  text-align: left;
  transition: border-color 120ms;
}
.choice + .choice {
  margin-top: 10px;
}
button.choice:hover:not(:disabled) {
  border-color: var(--accent-line);
}
.choice > svg {
  margin-top: 2px;
  color: var(--accent);
}
.choice > span {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.choice span span {
  color: var(--subtext);
  font-size: var(--text-sm);
}
.grow {
  flex: 1;
}
.inline {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}
.first {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.first .lead {
  margin-bottom: 6px;
}
.two {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 12px;
}
.hint {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.first .btn {
  align-self: flex-end;
}
</style>
