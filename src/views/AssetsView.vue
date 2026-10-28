<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Box, Copy, ExternalLink, FileImage, FolderOpen, Grid3x3, Images, Music, Pause, Play, RefreshCw, Search, Type } from "lucide-vue-next";
import PickMenu from "../components/PickMenu.vue";
import type { Asset } from "../domain/types";
import { fail, loadAssets, notify, now, project, store } from "../state/store";
import { api } from "../lib/api";
import { bytes, since } from "../lib/format";
import { copy } from "../lib/native";

/** What the game looks and sounds like, newest first: generated art and sounds show up here as they land. */
onMounted(() => store.assets === null && loadAssets());
watch(() => store.projectId, () => store.assets === null && loadAssets());

type Filter = "all" | "image" | "audio" | "other";
const filter = ref<Filter>("all");
const query = ref("");
const ALL = "__all__";
const folder = ref(ALL);
const sort = ref<"recent" | "name">("recent");
const pixel = ref(localStorage.getItem("assets.pixel") !== "0");
watch(pixel, (v) => localStorage.setItem("assets.pixel", v ? "1" : "0"));
const limit = ref(300);
const selected = ref<Asset | null>(null);

const DAY = 86_400_000;
const isNew = (a: Asset) => now.value - a.modified < DAY;

/** First two path segments: `content/items`, `assets/sounds`. */
const folderOf = (a: Asset) => a.rel.split("/").slice(0, -1).slice(0, 2).join("/") || ".";

const folders = computed(() => {
  const counts = new Map<string, number>();
  for (const a of store.assets ?? []) counts.set(folderOf(a), (counts.get(folderOf(a)) ?? 0) + 1);
  return [
    { id: ALL, label: "Tous les dossiers" },
    ...[...counts.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([id, n]) => ({ id, label: `${id} (${n})` })),
  ];
});

const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  const list = (store.assets ?? []).filter(
    (a) =>
      (filter.value === "all" ||
        (filter.value === "other" ? a.kind !== "image" && a.kind !== "audio" : a.kind === filter.value)) &&
      (folder.value === ALL || folderOf(a) === folder.value) &&
      (!q || a.rel.toLowerCase().includes(q)),
  );
  return sort.value === "recent" ? list.sort((a, b) => b.modified - a.modified) : list.sort((a, b) => a.rel.localeCompare(b.rel));
});
watch([filter, folder, query, sort], () => (limit.value = 300));

const images = computed(() => shown.value.filter((a) => a.kind === "image").slice(0, limit.value));
const others = computed(() => shown.value.filter((a) => a.kind !== "image").slice(0, limit.value));
const freshCount = computed(() => (store.assets ?? []).filter(isNew).length);

const name = (a: Asset) => a.rel.split("/").pop() ?? a.rel;
const KIND_ICON = { image: FileImage, audio: Music, model: Box, source: Grid3x3, font: Type };

// One player for every sound, so starting one stops the last.
const audio = new Audio();
const playing = ref<string | null>(null);
audio.addEventListener("ended", () => (playing.value = null));
function play(a: Asset) {
  if (playing.value === a.path) {
    audio.pause();
    playing.value = null;
    return;
  }
  audio.src = api.assetUrl(a.path);
  audio.play().then(
    () => (playing.value = a.path),
    (e) => fail(e, "Lecture"),
  );
}

async function open(a: Asset, reveal = false) {
  try {
    await api.openPath(a.path, reveal);
  } catch (e) {
    fail(e);
  }
}

function copyPath(a: Asset) {
  copy(a.rel);
  notify("Chemin copié");
}
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>Assets</h1>
      <div class="segmented">
        <button :class="{ on: filter === 'all' }" @click="filter = 'all'">Tout</button>
        <button :class="{ on: filter === 'image' }" @click="filter = 'image'"><Images />Images</button>
        <button :class="{ on: filter === 'audio' }" @click="filter = 'audio'"><Music />Sons</button>
        <button :class="{ on: filter === 'other' }" @click="filter = 'other'"><Box />Autres</button>
      </div>
      <PickMenu v-model="folder" :options="folders" filterable>
        <template #default="{ toggle }">
          <button class="btn btn-ghost btn-sm" @click="toggle"><FolderOpen />{{ folder === ALL ? "Tous les dossiers" : folder }}</button>
        </template>
      </PickMenu>
      <span class="spacer" />
      <label class="toggle"><input v-model="pixel" type="checkbox" />Pixel art</label>
      <div class="segmented">
        <button :class="{ on: sort === 'recent' }" @click="sort = 'recent'">Récents</button>
        <button :class="{ on: sort === 'name' }" @click="sort = 'name'">Nom</button>
      </div>
      <label class="find">
        <Search :size="14" />
        <input v-model="query" placeholder="Filtrer" @keydown.esc="query = ''" />
      </label>
      <button class="btn btn-ghost btn-icon" title="Relire les dossiers" @click="loadAssets"><RefreshCw /></button>
    </header>

    <div class="split" :class="{ detail: selected }">
      <div class="view-body">
        <p v-if="store.assetsError" class="empty">{{ store.assetsError }}</p>
        <div v-else-if="store.assets && !store.assets.length" class="empty">
          <Images />
          <p>
            Aucun asset dans {{ project?.assets.join(", ") || "les dossiers déclarés" }}.
            Les dossiers scannés se règlent dans les réglages du projet.
          </p>
        </div>
        <template v-else>
          <p v-if="freshCount && sort === 'recent'" class="fresh">{{ freshCount }} fichier{{ freshCount > 1 ? "s" : "" }} ajouté{{ freshCount > 1 ? "s" : "" }} ou modifié{{ freshCount > 1 ? "s" : "" }} depuis hier</p>

          <div v-if="images.length" class="grid" :class="{ pixel }">
            <button
              v-for="a in images"
              :key="a.path"
              class="tile"
              :class="{ on: selected?.path === a.path }"
              :title="a.rel"
              @click="selected = a"
              @dblclick="open(a)"
            >
              <span class="thumb"><img :src="api.assetUrl(a.path)" loading="lazy" decoding="async" :alt="name(a)" /></span>
              <span class="name">{{ name(a) }}</span>
              <span v-if="isNew(a)" class="new">nouveau</span>
            </button>
          </div>

          <div v-if="others.length" class="files">
            <div
              v-for="a in others"
              :key="a.path"
              class="file"
              :class="{ on: selected?.path === a.path }"
              @click="selected = a"
              @dblclick="open(a)"
            >
              <button v-if="a.kind === 'audio'" class="play" :title="playing === a.path ? 'Pause' : 'Écouter'" @click.stop="play(a)">
                <Pause v-if="playing === a.path" :size="13" />
                <Play v-else :size="13" />
              </button>
              <component :is="KIND_ICON[a.kind]" v-else :size="15" class="kind" />
              <span class="file-name">{{ name(a) }}</span>
              <span v-if="isNew(a)" class="new inline">nouveau</span>
              <span class="dir code">{{ folderOf(a) }}</span>
              <span class="size">{{ bytes(a.size) }}</span>
              <span class="when">{{ since(a.modified, now) }}</span>
            </div>
          </div>

          <button v-if="shown.length > limit" class="btn btn-quiet more" @click="limit += 300">
            Afficher plus ({{ shown.length - limit }} restants)
          </button>
        </template>
      </div>

      <aside v-if="selected" class="inspector">
        <div class="preview" :class="{ pixel }">
          <img v-if="selected.kind === 'image'" :src="api.assetUrl(selected.path)" :alt="name(selected)" />
          <button v-else-if="selected.kind === 'audio'" class="big-play" @click="play(selected)">
            <Pause v-if="playing === selected.path" :size="28" />
            <Play v-else :size="28" />
          </button>
          <component :is="KIND_ICON[selected.kind]" v-else :size="40" class="kind" />
        </div>
        <h3 class="selectable">{{ name(selected) }}</h3>
        <p class="path code selectable">{{ selected.rel }}</p>
        <dl>
          <dt>Taille</dt>
          <dd>{{ bytes(selected.size) }}</dd>
          <dt>Modifié</dt>
          <dd>{{ since(selected.modified, now) }}</dd>
        </dl>
        <div class="actions">
          <button class="btn btn-quiet" @click="open(selected)"><ExternalLink />Ouvrir</button>
          <button class="btn btn-ghost" @click="open(selected, true)"><FolderOpen />Finder</button>
          <button class="btn btn-ghost btn-icon" title="Copier le chemin" @click="copyPath(selected)"><Copy /></button>
        </div>
        <button class="btn btn-ghost btn-sm close" @click="selected = null">Fermer</button>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.split {
  display: grid;
  grid-template-columns: 1fr;
  flex: 1;
  min-height: 0;
}
.split.detail {
  grid-template-columns: 1fr 320px;
}
.toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--subtext);
  font-size: var(--text-xs);
  cursor: pointer;
}
.toggle input {
  accent-color: var(--accent);
}
.fresh {
  margin-bottom: 14px;
  color: var(--accent);
  font-size: var(--text-sm);
  font-weight: 550;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
  gap: 10px;
}
.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px;
  border-radius: var(--radius-md);
  border: 1px solid transparent;
  text-align: left;
}
.tile:hover {
  background: var(--raised);
}
.tile.on {
  border-color: var(--accent-line);
  background: var(--accent-soft);
}
/* A checkerboard shows where the art is transparent. */
.thumb,
.preview {
  display: grid;
  place-items: center;
  aspect-ratio: 1;
  border-radius: var(--radius-sm);
  background-color: #1c1c2a;
  background-image:
    linear-gradient(45deg, #242436 25%, transparent 25%),
    linear-gradient(-45deg, #242436 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, #242436 75%),
    linear-gradient(-45deg, transparent 75%, #242436 75%);
  background-size: 16px 16px;
  background-position:
    0 0,
    0 8px,
    8px -8px,
    -8px 0;
  overflow: hidden;
}
.thumb img,
.preview img {
  max-width: 82%;
  max-height: 82%;
  min-width: 40%;
  object-fit: contain;
}
.pixel img {
  image-rendering: pixelated;
}
.name {
  overflow: hidden;
  color: var(--subtext);
  font-size: var(--text-2xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.new {
  position: absolute;
  top: 10px;
  right: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--accent);
  color: var(--crust);
  font-size: 9.5px;
  font-weight: 700;
}
.new.inline {
  position: static;
}
.files {
  display: flex;
  flex-direction: column;
  gap: 1px;
  margin-top: 22px;
}
.file {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 36px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
}
.file:hover {
  background: rgb(255 255 255 / 0.03);
}
.file.on {
  background: var(--accent-soft);
}
.play {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--surface-0);
  color: var(--accent);
}
.play:hover {
  background: var(--surface-1);
}
.kind {
  color: var(--overlay);
}
.file-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dir,
.size,
.when {
  color: var(--overlay);
  font-size: var(--text-xs);
}
.when {
  width: 90px;
  text-align: right;
}
.more {
  margin-top: 18px;
}
.inspector {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 20px;
  border-left: 1px solid var(--line);
  overflow-y: auto;
}
.preview {
  width: 100%;
}
.big-play {
  display: grid;
  place-items: center;
  width: 72px;
  height: 72px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--crust);
}
.inspector h3 {
  font-size: var(--text-md);
  word-break: break-all;
}
.path {
  color: var(--overlay);
  font-size: var(--text-xs);
  word-break: break-all;
}
dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 14px;
  margin: 4px 0;
  font-size: var(--text-sm);
}
dt {
  color: var(--overlay);
}
dd {
  margin: 0;
}
.actions {
  display: flex;
  gap: 6px;
}
.close {
  align-self: flex-start;
}
</style>
