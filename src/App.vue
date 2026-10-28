<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import Sidebar from "./components/Sidebar.vue";
import ToastHost from "./components/ToastHost.vue";
import ItemDrawer from "./components/ItemDrawer.vue";
import QuickCapture from "./components/QuickCapture.vue";
import CommandPalette from "./components/CommandPalette.vue";
import LaunchSheet from "./components/LaunchSheet.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import OverviewView from "./views/OverviewView.vue";
import RoadmapView from "./views/RoadmapView.vue";
import TasksView from "./views/TasksView.vue";
import BacklogView from "./views/BacklogView.vue";
import AgentsView from "./views/AgentsView.vue";
import AssetsView from "./views/AssetsView.vue";
import GithubView from "./views/GithubView.vue";
import SettingsView from "./views/SettingsView.vue";
import OnboardingView from "./views/OnboardingView.vue";
import { boot, go, store, switchProject, type RouteName } from "./state/store";
import { isTyping } from "./lib/native";

const VIEWS: RouteName[] = ["overview", "roadmap", "tasks", "backlog", "agents", "assets", "github"];
const modalOpen = () => store.palette || !!store.capture || !!store.launch || !!store.confirm;

function onKey(e: KeyboardEvent) {
  // Physical keys: ⌘1 is the same key on AZERTY and QWERTY.
  const digit = /^Digit([1-9])$/.exec(e.code)?.[1];
  if (e.metaKey && e.code === "KeyK") {
    store.palette = !store.palette;
  } else if (e.metaKey && !e.shiftKey && digit && Number(digit) <= VIEWS.length && !modalOpen()) {
    const name = VIEWS[Number(digit) - 1];
    go(name === "agents" ? { name } : ({ name } as never));
  } else if (e.altKey && !e.metaKey && digit && !modalOpen()) {
    const p = store.projects[Number(digit) - 1];
    if (!p) return;
    switchProject(p.id);
  } else if (e.metaKey && e.code === "Comma") {
    go({ name: "settings" });
  } else if (e.metaKey && e.code === "KeyN" && !modalOpen()) {
    store.capture = { kind: "task" };
  } else if (e.key === "Escape" && store.drawer && !modalOpen()) {
    store.drawer = null;
  } else if (!e.metaKey && !e.ctrlKey && !e.altKey && !isTyping(e) && !modalOpen()) {
    if (e.key === "c") store.capture = { kind: "task" };
    else if (e.key === "b") store.capture = { kind: "bug" };
    else if (e.key === "i") store.capture = { kind: "idea" };
    else if (e.key === "a") store.launch = store.drawer ? { item: store.drawer } : {};
    else if (e.key === "/") document.querySelector<HTMLInputElement>(".find input")?.focus();
    else return;
  } else {
    return;
  }
  e.preventDefault();
}

onMounted(() => {
  boot();
  window.addEventListener("keydown", onKey);
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div v-if="!store.booted" class="boot" data-tauri-drag-region />
  <OnboardingView v-else-if="!store.projects.length" />
  <div v-else class="shell">
    <Sidebar />
    <main class="main">
      <Transition name="view" mode="out-in">
        <OverviewView v-if="store.route.name === 'overview'" :key="`o-${store.projectId}`" />
        <RoadmapView v-else-if="store.route.name === 'roadmap'" :key="`r-${store.projectId}`" />
        <TasksView v-else-if="store.route.name === 'tasks'" :key="`t-${store.projectId}`" />
        <BacklogView v-else-if="store.route.name === 'backlog'" :key="`b-${store.projectId}`" />
        <AgentsView v-else-if="store.route.name === 'agents'" :key="`a-${store.projectId}`" />
        <AssetsView v-else-if="store.route.name === 'assets'" :key="`s-${store.projectId}`" />
        <GithubView v-else-if="store.route.name === 'github'" :key="`g-${store.projectId}`" />
        <SettingsView v-else :key="`p-${store.projectId}`" />
      </Transition>
      <ItemDrawer />
    </main>
  </div>
  <QuickCapture v-if="store.capture" />
  <CommandPalette v-if="store.palette" />
  <LaunchSheet v-if="store.launch" />
  <ConfirmDialog />
  <ToastHost />
</template>

<style scoped>
.boot {
  height: 100%;
  background: var(--mantle);
}
.shell {
  display: grid;
  grid-template-columns: var(--sidebar-width) 1fr;
  height: 100%;
}
.main {
  position: relative;
  min-width: 0;
  height: 100%;
  overflow: hidden;
  background: var(--base);
}
.view-enter-active,
.view-leave-active {
  transition:
    opacity 120ms ease,
    transform 160ms var(--ease-out);
}
.view-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.view-leave-to {
  opacity: 0;
}
</style>
