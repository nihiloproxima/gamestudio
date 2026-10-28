<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { Cloud, CloudOff, Plus, RefreshCw, Sparkles, TriangleAlert } from "lucide-vue-next";
import ProjectGlyph from "../components/ProjectGlyph.vue";
import type { ProjectMeta } from "../domain/types";
import { attachProject, notify, now, project, saveProject, setupWithClaude, store, syncNow } from "../state/store";
import { since } from "../lib/format";

/** The current game's settings (synced through its data branch, except its path on this Mac), a new game, and
 *  where its data lives. */
const SWATCHES = ["#ff79c6", "#8bd5ca", "#ef9f76", "#89b4fa", "#cba6f7", "#a6e3a1", "#f9e2af", "#f38ba8", "#94e2d5", "#b4befe"];

const creating = ref(false);
const form = reactive({ name: "", key: "", color: "", path: "", github: "", base: "", assets: "", setup: "" });

function load() {
  const p = creating.value ? undefined : project.value;
  Object.assign(form, {
    name: p?.name ?? "",
    key: p?.key ?? "",
    color: p?.color ?? SWATCHES[3],
    path: p?.path ?? "~/code/",
    github: p?.github ?? "",
    base: p?.base ?? "",
    assets: p?.assets.join("\n") ?? "assets",
    setup: p?.setup ?? "",
  });
}
watch([() => project.value?.id, creating], load, { immediate: true });

const dirty = computed(() => {
  const p = project.value;
  if (creating.value || !p) return true;
  return (
    form.name !== p.name ||
    form.key !== p.key ||
    form.color !== p.color ||
    form.path !== p.path ||
    form.github !== p.github ||
    form.base !== p.base ||
    form.assets !== p.assets.join("\n") ||
    form.setup !== p.setup
  );
});

async function save() {
  const meta: ProjectMeta = {
    name: form.name.trim(),
    key: form.key.trim().toUpperCase(),
    color: form.color,
    path: form.path.trim(),
    github: form.github.trim().replace(/^https:\/\/github\.com\//, "").replace(/\.git$/, ""),
    base: form.base.trim(),
    assets: form.assets.split(/[\n,]/).map((a) => a.trim()).filter(Boolean),
    setup: form.setup.trim(),
  };
  if (creating.value) {
    const added = await attachProject(meta);
    if (!added) return;
    creating.value = false;
    notify(`${added.name} ajouté`, { action: { label: "Configurer avec Claude", run: () => setupWithClaude(added.id) } });
    return;
  }
  const id = project.value?.id;
  if (id && (await saveProject(id, meta, project.value?.vision ?? ""))) notify("Réglages enregistrés");
}

const remote = computed(() => (project.value ? store.sync?.remotes[project.value.id] : undefined));
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Réglages</h1>
      <span class="spacer" />
      <button v-if="!creating" class="btn btn-ghost" @click="creating = true"><Plus />Nouveau jeu</button>
    </header>

    <div class="view-body">
      <div class="page">
        <section class="card">
          <header class="card-head">
            <ProjectGlyph :project="{ name: form.name || '?', color: form.color }" :size="34" />
            <div>
              <h2>{{ creating ? "Nouveau jeu" : form.name }}</h2>
              <p class="muted">
                {{ creating ? "Si son dépôt a déjà une branche gamestudio, ses données reviennent telles quelles." : "Les mêmes réglages sur chaque Mac, sauf le chemin du dépôt." }}
              </p>
            </div>
          </header>
          <form class="form" @submit.prevent="save">
            <div class="two">
              <label><span class="label">Nom</span><input v-model="form.name" class="field" placeholder="Mon jeu" /></label>
              <label>
                <span class="label">Clé des éléments</span>
                <input v-model="form.key" class="field code" maxlength="5" placeholder="JEU" @input="form.key = form.key.toUpperCase()" />
              </label>
            </div>
            <div>
              <span class="label">Couleur</span>
              <div class="swatches">
                <button
                  v-for="c in SWATCHES"
                  :key="c"
                  type="button"
                  class="swatch"
                  :class="{ on: form.color === c }"
                  :style="{ background: c }"
                  :title="c"
                  @click="form.color = c"
                />
                <input v-model="form.color" type="color" class="picker" title="Autre couleur" />
              </div>
            </div>
            <div class="two">
              <label>
                <span class="label">Dépôt sur ce Mac</span>
                <input v-model="form.path" class="field code" placeholder="~/code/mon-jeu" spellcheck="false" />
              </label>
              <label>
                <span class="label">Dépôt GitHub</span>
                <input v-model="form.github" class="field code" placeholder="lu dans le dépôt si vide" spellcheck="false" />
              </label>
            </div>
            <p v-if="!creating && project && !project.present" class="warn">
              <TriangleAlert :size="14" />Pas de dépôt git dans {{ project.path }} sur ce Mac.
              <template v-if="project.github">Clone-le : <code class="selectable">git clone git@github.com:{{ project.github }}.git {{ project.path }}</code></template>
            </p>
            <div class="two">
              <label>
                <span class="label">Base des nouveaux worktrees</span>
                <input v-model="form.base" class="field code" placeholder="lue dans le dépôt si vide" spellcheck="false" />
              </label>
              <label>
                <span class="label">Installation d'un nouveau worktree</span>
                <input v-model="form.setup" class="field code" placeholder="pnpm install (facultatif)" spellcheck="false" />
              </label>
            </div>
            <label>
              <span class="label">Dossiers d'assets, un par ligne, relatifs au dépôt</span>
              <textarea v-model="form.assets" class="field code" rows="3" spellcheck="false" />
            </label>
            <div class="form-actions">
              <button v-if="creating" type="button" class="btn btn-ghost" @click="creating = false">Annuler</button>
              <button type="submit" class="btn btn-solid" :disabled="!dirty">{{ creating ? "Ajouter le jeu" : "Enregistrer" }}</button>
            </div>
          </form>
        </section>

        <section v-if="!creating && project" class="card">
          <header class="card-head">
            <component :is="remote ? Cloud : CloudOff" :size="22" class="muted" />
            <div>
              <h2>Données et synchro</h2>
              <p class="muted">
                Le suivi de {{ project.name }} vit sur la branche <code>gamestudio</code> de son dépôt, sans lien avec
                l'historique du jeu : un fichier Markdown par élément, copié dans
                <code class="selectable">~/.gamestudio/data/{{ project.id }}</code>.
              </p>
            </div>
          </header>
          <p class="state">
            <template v-if="remote">Synchronisé avec <code class="selectable">{{ remote }}</code></template>
            <template v-else>Pas de dépôt distant : les données restent sur ce Mac</template>
            <template v-if="store.sync?.lastSync">, dernière synchro {{ since(store.sync.lastSync, now) }}</template>. Un
            changement part quelques secondes après, et l'app récupère ceux des autres Mac toutes les 3 minutes. Sur un
            autre Mac, clone le jeu et ajoute-le : son suivi revient.
          </p>
          <p v-if="store.sync?.error" class="warn selectable"><TriangleAlert :size="14" />{{ store.sync.error }}</p>
          <button class="btn btn-quiet" :disabled="store.sync?.syncing" @click="syncNow"><RefreshCw />Synchroniser maintenant</button>
        </section>

        <section v-if="!creating" class="card claude">
          <header class="card-head">
            <Sparkles :size="22" class="spark" />
            <div>
              <h2>Configurer avec Claude</h2>
              <p class="muted">
                Claude lit le dépôt (plans, design, historique, PR, assets), te pose ses questions, te montre ce qu'il
                compte créer, puis remplit la roadmap, les tâches, les idées et ces réglages une fois que tu as dit oui.
              </p>
            </div>
          </header>
          <button class="btn btn-solid" :disabled="!project?.present" @click="setupWithClaude()"><Sparkles />Lancer la configuration</button>
          <p v-if="project && !project.present" class="muted small">Le dépôt doit être cloné sur ce Mac.</p>
        </section>

        <section v-if="!creating" class="card">
          <header class="card-head">
            <div>
              <h2>Pour les agents</h2>
              <p class="muted">Les sessions lancées ici connaissent la commande <code>studio</code> : elles lisent leur fiche, notent leurs avancées et proposent des idées.</p>
            </div>
          </header>
          <pre class="cli selectable">studio items                       ce qui est ouvert dans le projet
studio item JEU-12                 une fiche
studio idea "un corbeau…"          une idée à trier
studio note JEU-12 "…"             une note datée sur la fiche
studio status JEU-12 review        le statut
studio import plan.json --dry-run  un plan entier, en aperçu
studio guide setup                 la marche à suivre pour configurer un jeu
studio help                        tout le reste</pre>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: 18px;
  max-width: 760px;
}
.card {
  padding: 20px 22px;
  border-radius: var(--radius-lg);
  background: var(--raised);
  border: 1px solid var(--line);
}
.card-head {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 18px;
}
.card-head h2 {
  font-size: var(--text-lg);
}
.card-head p {
  margin-top: 3px;
  font-size: var(--text-sm);
}
.form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
}
.swatches {
  display: flex;
  align-items: center;
  gap: 8px;
}
.swatch {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.15);
  transition: transform 120ms var(--ease-out);
}
.swatch:hover {
  transform: scale(1.1);
}
.swatch.on {
  box-shadow:
    0 0 0 2px var(--raised),
    0 0 0 4px var(--bright);
}
.picker {
  width: 28px;
  height: 28px;
  padding: 0;
  border: 0;
  background: none;
  cursor: pointer;
}
.warn {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  color: var(--pr-high);
  font-size: var(--text-sm);
}
.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.state {
  margin-bottom: 12px;
  color: var(--subtext);
  font-size: var(--text-sm);
  line-height: 1.55;
}
.small {
  margin: 4px 0 8px;
  font-size: var(--text-xs);
}
.claude {
  background:
    radial-gradient(120% 140% at 100% 0%, color-mix(in oklab, #e8a68a 10%, transparent), transparent 60%),
    var(--raised);
}
.spark {
  color: #e8a68a;
}
.cli {
  margin: 0;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  background: var(--crust);
  color: var(--subtext);
  font-family: var(--font-code);
  font-size: var(--text-xs);
  line-height: 1.7;
}
</style>
