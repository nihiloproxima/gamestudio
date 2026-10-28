<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { GitBranch } from "lucide-vue-next";
import { attachProject, boot } from "../state/store";

/** First launch: point at a game's repo. If it already holds Gamestudio data, it comes back as it is. */
const name = ref("");
const key = ref("");
const path = ref("~/code/");
const keyTouched = ref(false);
const busy = ref(false);

watch(name, (n) => {
  if (!keyTouched.value) key.value = n.normalize("NFD").replace(/[^A-Za-z]/g, "").slice(0, 3).toUpperCase();
});

const ready = computed(() => path.value.trim().length > 7);

async function add() {
  if (!ready.value || busy.value) return;
  busy.value = true;
  const added = await attachProject({
    name: name.value.trim(),
    key: key.value,
    color: "#89b4fa",
    path: path.value.trim(),
    github: "",
    base: "",
    assets: ["assets"],
    setup: "",
  });
  if (added) await boot();
  busy.value = false;
}
</script>

<template>
  <div class="onboarding" data-tauri-drag-region>
    <form class="card" @submit.prevent="add">
      <div class="gem">
        <span style="--c: #ff79c6" /><span style="--c: #8bd5ca" /><span style="--c: #ef9f76" />
      </div>
      <h1>Gamestudio</h1>
      <p class="lead">
        Roadmap, tâches, idées, agents et assets de tes jeux, au même endroit. Commence par le dépôt git d'un jeu :
        son suivi vivra sur une branche <code>gamestudio</code> de ce dépôt, à côté du code sans s'y mêler.
      </p>

      <label>
        <span class="label">Dépôt du jeu sur ce Mac</span>
        <input v-model="path" class="field code" placeholder="~/code/mon-jeu" spellcheck="false" autofocus />
      </label>
      <div class="two">
        <label><span class="label">Nom</span><input v-model="name" class="field" placeholder="Mon jeu" /></label>
        <label>
          <span class="label">Clé des éléments</span>
          <input v-model="key" class="field code" maxlength="5" placeholder="JEU" @input="(keyTouched = true), (key = key.toUpperCase())" />
        </label>
      </div>
      <p class="hint">
        <GitBranch :size="14" />
        <span>
          Le dépôt a déjà une branche <code>gamestudio</code> (un autre Mac) ? Ses données reviennent telles quelles, le nom
          et la clé ne servent qu'à un nouveau jeu.
        </span>
      </p>
      <button type="submit" class="btn btn-solid" :disabled="busy || !ready">{{ busy ? "Ajout…" : "Ajouter le jeu" }}</button>
    </form>
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
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 560px;
  padding: 36px;
  border-radius: 20px;
  background: var(--raised);
  box-shadow: var(--shadow-pop);
}
.gem {
  display: flex;
  gap: 6px;
  margin-bottom: 4px;
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
  margin-bottom: 8px;
  color: var(--subtext);
  line-height: 1.55;
}
.two {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 12px;
}
.hint {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  color: var(--overlay);
  font-size: var(--text-xs);
  line-height: 1.5;
}
.hint svg {
  flex: none;
  margin-top: 1px;
}
.btn {
  align-self: flex-end;
}
</style>
