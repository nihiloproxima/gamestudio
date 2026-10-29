import { computed, reactive, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  STATUS_PRIORITY,
  type AgentStatus,
  type Asset,
  type Item,
  type ItemDraft,
  type ItemKind,
  type ItemPatch,
  type Milestone,
  type Priority,
  type Project,
  type ProjectMeta,
  type PullRequest,
  type Session,
  type StartRequest,
  type SyncStatus,
  type UpdateStatus,
  type Worktree,
} from "../domain/types";
import { api, errorText, inTauri } from "../lib/api";

export type RouteName = "overview" | "roadmap" | "tasks" | "backlog" | "agents" | "assets" | "github" | "settings";
export type Route = { name: Exclude<RouteName, "agents"> } | { name: "agents"; session?: string };

export interface Toast {
  id: number;
  text: string;
  tone?: "error";
  action?: { label: string; run: () => void };
}

export const store = reactive({
  booted: false,
  sync: null as SyncStatus | null,
  update: null as UpdateStatus | null,
  projects: [] as Project[],
  projectId: "",
  route: { name: "overview" } as Route,
  items: [] as Item[],
  milestones: [] as Milestone[],
  /** Every project's sessions: a waiting agent elsewhere still shows on the switcher. */
  sessions: [] as Session[],
  /** null until loaded; the matching error explains an empty list. */
  worktrees: null as Worktree[] | null,
  worktreesError: "",
  prs: null as PullRequest[] | null,
  prsError: "",
  assets: null as Asset[] | null,
  assetsError: "",
  /** Item shown in the side drawer. */
  drawer: null as string | null,
  palette: false,
  capture: null as { kind: ItemKind; milestone?: string } | null,
  launch: null as { item?: string; path?: string } | null,
  orchestrate: false,
  toasts: [] as Toast[],
  confirm: null as { text: string; action: string; resolve: (ok: boolean) => void } | null,
});

/** In-app confirmation for destructive actions (the webview has no reliable native confirm). */
export function confirmAsk(text: string, action: string): Promise<boolean> {
  store.confirm?.resolve(false);
  return new Promise((resolve) => (store.confirm = { text, action, resolve }));
}

export const project = computed(() => store.projects.find((p) => p.id === store.projectId));

/** Ticks so relative times stay fresh without each component owning a timer. */
export const now = ref(Date.now());
setInterval(() => (now.value = Date.now()), 30_000);

export function go(route: Route) {
  store.route = route;
}

let toastSeq = 0;
export function notify(text: string, opts: Omit<Toast, "id" | "text"> = {}) {
  const id = ++toastSeq;
  store.toasts = [...store.toasts.slice(-2), { id, text, ...opts }];
  setTimeout(() => (store.toasts = store.toasts.filter((t) => t.id !== id)), opts.tone === "error" ? 7000 : 4200);
}

export function fail(e: unknown, context?: string) {
  notify(context ? `${context} : ${errorText(e)}` : errorText(e), { tone: "error" });
}

// Loading ---------------------------------------------------------------------------------------------------------

export async function boot() {
  try {
    [store.sync, store.update] = await Promise.all([api.syncStatus(), api.updateStatus()]);
    store.projects = await api.projects();
    const saved = localStorage.getItem("project");
    store.projectId = store.projects.find((p) => p.id === saved)?.id ?? store.projects[0]?.id ?? "";
    await Promise.all([loadProject(), refreshSessions()]);
    if (!inTauri) openFromUrl();
  } catch (e) {
    fail(e, "Démarrage");
  } finally {
    store.booted = true;
  }
}

/** Browser mode only: `?view=agents&session=fro-1-claude`, `?item=FRO-1`, `?palette=1`… open the demo on a given
 *  screen, for screenshots (`scripts/screenshots.sh`) and design work. */
function openFromUrl() {
  const q = new URLSearchParams(location.search);
  const view = q.get("view") as RouteName | null;
  if (view) store.route = view === "agents" ? { name: "agents", session: q.get("session") ?? undefined } : ({ name: view } as Route);
  store.drawer = q.get("item");
  store.palette = q.has("palette");
  if (q.has("launch")) store.launch = { item: q.get("launch") || undefined };
  store.orchestrate = q.has("orchestrate");
  if (q.has("capture")) store.capture = { kind: (q.get("capture") || "task") as ItemKind };
}

async function loadProject() {
  const id = store.projectId;
  if (!id) return;
  store.worktrees = null;
  store.prs = null;
  store.assets = null;
  store.worktreesError = store.prsError = store.assetsError = "";
  await refreshData();
  refreshWorktrees();
  refreshPrs();
}

export async function switchProject(id: string) {
  if (id === store.projectId || !store.projects.some((p) => p.id === id)) return;
  store.projectId = id;
  store.drawer = null;
  localStorage.setItem("project", id);
  if (store.route.name === "agents") store.route = { name: "agents" };
  await loadProject();
}

export async function refreshData() {
  const id = store.projectId;
  try {
    const [projects, items, milestones] = await Promise.all([api.projects(), api.items(id), api.milestones(id)]);
    if (id !== store.projectId) return;
    store.projects = projects;
    store.items = items;
    store.milestones = milestones;
  } catch (e) {
    fail(e, "Lecture des données");
  }
}

export async function refreshSessions() {
  try {
    store.sessions = await api.sessions();
  } catch (e) {
    console.warn("Sessions :", errorText(e));
  }
}

export async function refreshWorktrees() {
  const id = store.projectId;
  try {
    const list = await api.worktrees(id);
    if (id !== store.projectId) return;
    store.worktrees = list;
    store.worktreesError = "";
  } catch (e) {
    if (id !== store.projectId) return;
    store.worktrees = [];
    store.worktreesError = errorText(e);
  }
}

export async function refreshPrs() {
  const id = store.projectId;
  try {
    const list = await api.prs(id);
    if (id !== store.projectId) return;
    store.prs = list;
    store.prsError = "";
  } catch (e) {
    if (id !== store.projectId) return;
    store.prs = [];
    store.prsError = errorText(e);
  }
}

export async function loadAssets() {
  const id = store.projectId;
  try {
    const list = await api.assets(id);
    if (id !== store.projectId) return;
    store.assets = list;
    store.assetsError = "";
  } catch (e) {
    if (id !== store.projectId) return;
    store.assets = [];
    store.assetsError = errorText(e);
  }
}

// Items and milestones --------------------------------------------------------------------------------------------

function put(item: Item) {
  const i = store.items.findIndex((x) => x.id === item.id);
  if (i >= 0) store.items[i] = item;
  else if (item.project === store.projectId) store.items.push(item);
}

export async function createItem(draft: ItemDraft, open = false): Promise<Item | undefined> {
  try {
    const item = await api.itemCreate(store.projectId, draft);
    put(item);
    if (open) store.drawer = item.id;
    else notify(`${item.id} créé`, { action: { label: "Ouvrir", run: () => (store.drawer = item.id) } });
    return item;
  } catch (e) {
    fail(e);
  }
}

export async function updateItem(id: string, patch: ItemPatch): Promise<Item | undefined> {
  try {
    const item = await api.itemUpdate(id, patch);
    put(item);
    return item;
  } catch (e) {
    fail(e);
  }
}

export async function deleteItem(id: string) {
  try {
    await api.itemDelete(id);
    store.items = store.items.filter((i) => i.id !== id);
    if (store.drawer === id) store.drawer = null;
    notify(`${id} supprimé`);
  } catch (e) {
    fail(e);
  }
}

export async function saveProject(id: string, meta: ProjectMeta, vision: string) {
  try {
    const saved = await api.projectSave(id, meta, vision);
    store.projects = await api.projects();
    return saved;
  } catch (e) {
    fail(e);
  }
}

/** Adds a game from its repo, then shows it. */
export async function attachProject(meta: ProjectMeta) {
  try {
    const added = await api.projectAttach(meta);
    store.projects = await api.projects();
    await switchProject(added.id);
    return added;
  } catch (e) {
    fail(e, "Ajout du jeu");
  }
}

export async function saveMilestone(id: string | null, m: Omit<Milestone, "id">) {
  const { goal, ...meta } = m;
  try {
    const saved = await api.milestoneSave(store.projectId, id, meta, goal);
    store.milestones = await api.milestones(store.projectId);
    return saved;
  } catch (e) {
    fail(e);
  }
}

export async function deleteMilestone(id: string) {
  try {
    await api.milestoneDelete(store.projectId, id);
    await refreshData();
  } catch (e) {
    fail(e);
  }
}

export const itemsById = computed(() => new Map(store.items.map((i) => [i.id, i])));
export const milestonesById = computed(() => new Map(store.milestones.map((m) => [m.id, m])));

const PRIORITY_RANK: Record<Priority, number> = { urgent: 0, high: 1, normal: 2, low: 3 };

/** Most pressing first, then most recently touched. */
export const byPriority = (a: Item, b: Item) =>
  PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] || b.updated.localeCompare(a.updated);

export const isOpen = (i: Item) => i.status !== "done" && i.status !== "dropped";

export const activeMilestone = computed(
  () => store.milestones.find((m) => m.status === "active") ?? store.milestones.find((m) => m.status === "planned"),
);

export function progressOf(milestoneId: string) {
  const list = store.items.filter((i) => i.milestone === milestoneId && i.status !== "dropped");
  const done = list.filter((i) => i.status === "done").length;
  return { done, total: list.length, ratio: list.length ? done / list.length : 0 };
}

// Sessions ------------------------------------------------------------------------------------------------------

export const projectSessions = computed(() => store.sessions.filter((s) => s.project === store.projectId));

export const sessionsByUrgency = (list: Session[]) =>
  [...list].sort(
    (a, b) =>
      STATUS_PRIORITY.indexOf(a.status) - STATUS_PRIORITY.indexOf(b.status) || a.statusAt.localeCompare(b.statusAt),
  );

export function waitingIn(projectId: string) {
  return store.sessions.filter((s) => s.project === projectId && s.status === "waiting").length;
}

export function statusOf(list: Session[]): AgentStatus {
  return STATUS_PRIORITY.find((s) => list.some((x) => x.status === s)) ?? "offline";
}

export async function startSession(req: StartRequest) {
  try {
    const session = await api.sessionStart(req);
    await refreshSessions();
    refreshWorktrees();
    if (session.item) refreshData();
    store.launch = null;
    go({ name: "agents", session: session.id });
    return session;
  } catch (e) {
    fail(e, "Lancement");
  }
}

/** Claude scans the repo, asks the user, then fills the project once they agree (`studio guide setup`). */
export async function setupWithClaude(projectId = store.projectId) {
  try {
    const session = await api.sessionSetup(projectId);
    await refreshSessions();
    go({ name: "agents", session: session.id });
  } catch (e) {
    fail(e, "Configuration");
  }
}

/** The current game's live orchestrator: there is one at most. */
export const orchestrator = computed(() =>
  projectSessions.value.find((s) => s.role === "orchestrator" && s.status !== "offline"),
);

/** Shows the running orchestrator, or asks how to start one. */
export function openOrchestration() {
  if (orchestrator.value) go({ name: "agents", session: orchestrator.value.id });
  else store.orchestrate = true;
}

export async function orchestrate(parallel: number, prompt: string) {
  try {
    const session = await api.sessionOrchestrate(store.projectId, parallel, prompt);
    await refreshSessions();
    refreshWorktrees();
    store.orchestrate = false;
    go({ name: "agents", session: session.id });
  } catch (e) {
    fail(e, "Orchestration");
  }
}

export async function resumeSession(id: string) {
  try {
    await api.sessionResume(id);
    await refreshSessions();
  } catch (e) {
    fail(e, "Reprise");
  }
}

export async function stopSession(id: string) {
  try {
    await api.sessionStop(id);
    await refreshSessions();
    if (store.route.name === "agents" && store.route.session === id) go({ name: "agents" });
  } catch (e) {
    fail(e, "Arrêt");
  }
}

// Updates -------------------------------------------------------------------------------------------------------

export async function checkForUpdate() {
  try {
    store.update = await api.updateCheck();
    if (store.update.state === "idle") notify(`Gamestudio ${store.update.current} est à jour`);
    if (store.update.state === "error") fail(store.update.error, "Mise à jour");
  } catch (e) {
    fail(e, "Mise à jour");
  }
}

/** Back on the version just installed; agents keep running in tmux. */
export function restartApp() {
  api.restart().catch((e) => fail(e, "Redémarrage"));
}

// Sync ----------------------------------------------------------------------------------------------------------

export async function syncNow() {
  try {
    store.sync = await api.syncNow();
    if (store.sync.error) fail(store.sync.error, "Synchro");
    else await refreshData();
  } catch (e) {
    fail(e, "Synchro");
  }
}

// Live updates ----------------------------------------------------------------------------------------------------

/** The game's color lights the whole app. */
watch(
  () => project.value?.color,
  (color) => color && document.documentElement.style.setProperty("--accent", color),
);

let dataTimer: ReturnType<typeof setTimeout> | undefined;
function refreshDataSoon() {
  clearTimeout(dataTimer);
  dataTimer = setTimeout(refreshData, 150);
}

if (inTauri) {
  // The CLI, the agents and the sync change data behind the UI's back; the core says so.
  listen("data-changed", refreshDataSoon);
  listen("sessions-changed", refreshSessions);
  // `studio start` from a terminal: show the new session.
  listen<string>("navigate-session", async (e) => {
    await refreshSessions();
    const s = store.sessions.find((x) => x.id === e.payload);
    if (!s) return;
    await switchProject(s.project);
    go({ name: "agents", session: s.id });
  });
  listen<SyncStatus>("sync-changed", (e) => {
    const notice = e.payload.notice;
    if (notice && notice !== store.sync?.notice) notify(notice);
    store.sync = e.payload;
  });
  listen<UpdateStatus>("update-changed", (e) => {
    const ready = e.payload.state === "ready" && store.update?.state !== "ready";
    store.update = e.payload;
    if (ready) notify(`Gamestudio ${e.payload.latest?.replace(/^v/, "")} est installée`, { action: { label: "Redémarrer", run: restartApp } });
  });
  listen<{ project: string; id: string }>("open-item", async (e) => {
    await switchProject(e.payload.project);
    store.drawer = e.payload.id;
  });
}

window.addEventListener("focus", () => {
  if (!store.projectId) return;
  refreshDataSoon();
  refreshWorktrees();
});

// tmux liveness has no event; worktrees and PRs change outside the app.
setInterval(() => store.projectId && refreshSessions(), 4000);
setInterval(() => {
  if (store.projectId && ["overview", "agents"].includes(store.route.name)) refreshWorktrees();
}, 15_000);
setInterval(() => store.projectId && refreshPrs(), 120_000);
