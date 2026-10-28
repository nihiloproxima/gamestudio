/** Browser mode (`pnpm dev` without Tauri): an in-memory studio, so the UI can be designed and checked anywhere. */
import type {
  Asset,
  Item,
  ItemDraft,
  ItemPatch,
  Milestone,
  Project,
  PullRequest,
  Session,
  SyncStatus,
  UpdateStatus,
  Worktree,
} from "../domain/types";
import type { Api } from "./api";

const ago = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();

const projects: Project[] = [
  {
    id: "frostholm",
    name: "Frostholm",
    key: "FRO",
    color: "#8bd5ca",
    path: "~/code/frostholm",
    github: "ada/frostholm",
    base: "origin/main",
    assets: ["assets", "content", "ui"],
    setup: "",
    vision:
      "Survie coop en 2D (Godot). Un îlot, des saisons rudes, des maîtrises qui se gagnent en jouant.\n\n- Jouable à 4 en ligne\n- Une boucle survie → progression → donjon",
    present: true,
  },
  {
    id: "ember",
    name: "Ember",
    key: "EMB",
    color: "#ef9f76",
    path: "~/code/ember",
    github: "ada/ember",
    base: "origin/main",
    assets: ["assets"],
    setup: "",
    vision: "Jeu idle contemplatif : le lac à l'aube, la forêt l'après-midi, le feu de camp la nuit.",
    present: true,
  },
  {
    id: "neon-city",
    name: "Neon City",
    key: "NEO",
    color: "#ff79c6",
    path: "~/code/neon-city",
    github: "ada/neon-city",
    base: "origin/next",
    assets: ["packages/assets/src"],
    setup: "pnpm install --frozen-lockfile",
    vision: "City-builder géolocalisé : acheter des lieux réels, gagner Coins et Réputation, saisons classées.",
    present: true,
  },
];

const milestones: Record<string, Milestone[]> = {
  frostholm: [
    { id: "jalon-0-technique", title: "Jalon 0 : technique", status: "done", order: 0, target: "2026-08-15", goal: "Réseau, sauvegarde, base de contenu." },
    { id: "jalon-1-combat", title: "Jalon 1 : combat", status: "done", order: 1, target: "2026-09-10", goal: "On se bat à plusieurs contre des ennemis lisibles." },
    { id: "jalon-2-survie", title: "Jalon 2 : survie", status: "done", order: 2, target: "2026-10-01", goal: "Faim, froid, abri : on survit une semaine de jeu." },
    { id: "jalon-3-progression", title: "Jalon 3 : progression", status: "active", order: 3, target: "2026-11-15", goal: "**Fait quand** les maîtrises, arbres et armures donnent envie de rejouer une heure de plus." },
    { id: "jalon-4-v0-1", title: "Jalon 4 : v0.1", status: "planned", order: 4, target: "2026-12-20", goal: "Une version jouable à montrer à 10 amis." },
  ],
  ember: [
    { id: "lots-30", title: "Lots 30 à 40 : la veillée", status: "active", order: 0, target: "2026-11-05", goal: "Les récits du soir et le camp qui s'étoffe." },
    { id: "steam", title: "Page Steam", status: "planned", order: 1, goal: "Capsule, trailer, wishlist." },
  ],
  "neon-city": [{ id: "saison-12", title: "Saison 12", status: "active", order: 0, target: "2026-11-30", goal: "Gangs v2, économie revue." }],
};

const item = (project: string, n: number, title: string, kind: Item["kind"], status: Item["status"], extra: Partial<Item> = {}): Item => ({
  id: `${projects.find((p) => p.id === project)!.key}-${n}`,
  project,
  title,
  kind,
  status,
  priority: "normal",
  labels: [],
  created: ago(60 * 24 * (20 - n)),
  updated: ago(n * 37),
  body: "",
  ...extra,
});

const items: Item[] = [
  item("frostholm", 1, "Armures : slot de torse et de jambes", "task", "doing", { priority: "high", milestone: "jalon-3-progression", labels: ["progression"], body: "Les armures donnent de la défense et un malus de vitesse.\n\n- [x] Slots dans l'inventaire\n- [ ] Rendu sur le personnage\n- [ ] Réglages dans `content/armors/`" }),
  item("frostholm", 2, "Bâton de feu : la Nova repousse trop loin", "bug", "todo", { priority: "urgent", milestone: "jalon-3-progression", labels: ["combat"] }),
  item("frostholm", 3, "Arbre du bâton de glace", "task", "todo", { milestone: "jalon-3-progression" }),
  item("frostholm", 4, "Talents : afficher le coût en points", "task", "review", { milestone: "jalon-3-progression", priority: "high" }),
  item("frostholm", 5, "Refonte des gestes de combat", "idea", "backlog", { body: "Poses, rythme, contact. Après les systèmes." }),
  item("frostholm", 6, "Un corbeau qui vole les graines", "idea", "inbox"),
  item("frostholm", 7, "Le froid ralentit la régénération d'endurance", "idea", "inbox"),
  item("frostholm", 8, "Crash en rejoignant une partie pendant la nuit", "bug", "inbox", { priority: "high" }),
  item("frostholm", 9, "Maîtrise de l'arc", "task", "done", { milestone: "jalon-3-progression", closed: ago(60 * 30) }),
  item("frostholm", 10, "Bâton de feu : boules en crescendo", "task", "done", { milestone: "jalon-3-progression", closed: ago(60 * 8) }),
  item("frostholm", 11, "Écran titre", "task", "todo", { milestone: "jalon-4-v0-1", priority: "low" }),
  item("frostholm", 12, "Plus de choix dans les arbres de talents", "idea", "backlog"),
  item("ember", 1, "Le récit du troisième talisman", "task", "doing", { milestone: "lots-30" }),
  item("ember", 2, "La flûte joue faux la nuit de pluie", "bug", "todo", { priority: "high" }),
  item("ember", 3, "Un renard qui vient se chauffer au feu", "idea", "inbox"),
  item("neon-city", 1, "Gangs : le QG se pose une fois pour toutes", "task", "review", { milestone: "saison-12", priority: "high" }),
  item("neon-city", 2, "Le Neon Katana ignore la Surveillance", "bug", "done", { closed: ago(300) }),
];

const sessions: Session[] = [
  { id: "fro-1-claude", project: "frostholm", item: "FRO-1", title: "Armures : slot de torse et de jambes", agent: "claude", cwd: "/Users/ada/.gamestudio/worktrees/frostholm/fro-1-armures", branch: "fro-1-armures", created: ago(95), claudeSession: null, status: "waiting", statusAt: ago(4), message: "Claude a besoin de ta permission pour lancer godot --headless" },
  { id: "fro-4-codex", project: "frostholm", item: "FRO-4", title: "Talents : afficher le coût en points", agent: "codex", cwd: "/Users/ada/code/frostholm", branch: null, created: ago(240), claudeSession: null, status: "working", statusAt: ago(1), message: null },
  { id: "frostholm-shell", project: "frostholm", item: null, title: "Terminal", agent: "shell", cwd: "/Users/ada/code/frostholm", branch: null, created: ago(30), claudeSession: null, status: "idle", statusAt: ago(30), message: null },
  { id: "sfx-forge", project: "frostholm", item: null, title: "Sons de la forge (ElevenLabs)", agent: "claude", cwd: "/Users/ada/code/frostholm", branch: null, created: ago(20), claudeSession: null, status: "working", statusAt: ago(2), message: null },
  { id: "emb-1-claude", project: "ember", item: "EMB-1", title: "Le récit du troisième talisman", agent: "claude", cwd: "/Users/ada/code/ember", branch: null, created: ago(50), claudeSession: null, status: "waiting", statusAt: ago(12), message: "J'ai fini le premier jet du récit, tu veux le relire ?" },
];

const worktrees: Record<string, Worktree[]> = {
  frostholm: [
    { path: "/Users/ada/code/frostholm", branch: "main", head: "437344a", main: true, locked: false, dirty: 3, ahead: 0, behind: 0, lastCommit: "Plan : refonte du combat après les systèmes", lastCommitAt: ago(80) },
    { path: "/Users/ada/.gamestudio/worktrees/frostholm/fro-1-armures", branch: "fro-1-armures", head: "9f1c2e0", main: false, locked: false, dirty: 7, ahead: 4, behind: 1, lastCommit: "3.9 Armures : slots de torse et jambes", lastCommitAt: ago(15) },
    { path: "/Users/ada/.gamestudio/worktrees/frostholm/fro-4-talents-cout", branch: "fro-4-talents-cout", head: "a02b77d", main: false, locked: false, dirty: 0, ahead: 2, behind: 0, lastCommit: "Talents : le coût en points sous chaque nœud", lastCommitAt: ago(200) },
  ],
};

const prs: Record<string, PullRequest[]> = {
  frostholm: [
    { number: 42, title: "FRO-4 Talents : afficher le coût en points", state: "open", branch: "fro-4-talents-cout", base: "main", url: "https://github.com/ada/frostholm/pull/42", author: "ada", updatedAt: ago(190), mergedAt: null, additions: 184, deletions: 22, review: "none", checks: "pass" },
    { number: 41, title: "FRO-1 Armures (brouillon)", state: "draft", branch: "fro-1-armures", base: "main", url: "https://github.com/ada/frostholm/pull/41", author: "ada", updatedAt: ago(20), mergedAt: null, additions: 512, deletions: 40, review: "none", checks: "pending" },
    { number: 39, title: "Maîtrise de l'arc", state: "merged", branch: "fro-9-arc", base: "main", url: "https://github.com/ada/frostholm/pull/39", author: "ada", updatedAt: ago(1800), mergedAt: ago(1800), additions: 930, deletions: 120, review: "approved", checks: "pass" },
  ],
  "neon-city": [
    { number: 812, title: "feat(gangs): say where the HQ goes (NEO-1)", state: "open", branch: "feature/neo-1", base: "next", url: "https://github.com/ada/neon-city/pull/812", author: "ada", updatedAt: ago(60), mergedAt: null, additions: 240, deletions: 31, review: "changes", checks: "fail" },
  ],
};

const swatch = (hue: number, shape: number) =>
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" shape-rendering="crispEdges"><rect width="16" height="16" fill="none"/>${
      shape % 3 === 0
        ? `<rect x="4" y="2" width="8" height="12" fill="hsl(${hue} 60% 55%)"/><rect x="5" y="3" width="3" height="3" fill="hsl(${hue} 70% 80%)"/>`
        : shape % 3 === 1
          ? `<circle cx="8" cy="8" r="5" fill="hsl(${hue} 65% 60%)"/><rect x="6" y="5" width="2" height="2" fill="#fff8"/>`
          : `<path d="M8 1 L15 8 L8 15 L1 8 Z" fill="hsl(${hue} 55% 50%)"/>`
    }</svg>`,
  );

const assets: Asset[] = [
  ...["armor_chest_iron", "armor_legs_leather", "fire_staff", "ice_staff", "bow_wood", "raven", "seed_wheat", "torch", "chest", "campfire", "ore_iron", "potion_red"].map(
    (name, i): Asset => ({ path: swatch(i * 31, i), rel: `content/items/${name}.png`, kind: "image", size: 1200 + i * 90, modified: Date.now() - i * 3_600_000 * (i > 3 ? 30 : 1) }),
  ),
  ...["forge_hammer_1", "forge_hammer_2", "fire_whoosh", "raven_caw"].map(
    (name, i): Asset => ({ path: "", rel: `assets/sounds/${name}.wav`, kind: "audio", size: 88_000 + i * 4000, modified: Date.now() - i * 600_000 }),
  ),
  { path: "", rel: "assets/models/house.blend", kind: "model", size: 2_400_000, modified: Date.now() - 86_400_000 * 6 },
  { path: "", rel: "assets/sprites/player.aseprite", kind: "source", size: 54_000, modified: Date.now() - 86_400_000 * 2 },
];

let sync: SyncStatus = {
  syncing: false,
  lastSync: ago(2),
  error: null,
  pending: 0,
  notice: null,
  remotes: Object.fromEntries(projects.map((p) => [p.id, `git@github.com:${p.github}.git`])),
};

let update: UpdateStatus = { current: "0.1.0", state: "idle", latest: "v0.1.0", error: null, checkedAt: ago(30) };

const wait = <T>(value: T, ms = 120) => new Promise<T>((resolve) => setTimeout(() => resolve(structuredClone(value)), ms));
const fail = (text: string) => Promise.reject(text);

function findItem(id: string) {
  const it = items.find((i) => i.id === id.toUpperCase());
  if (!it) throw `Élément inconnu : ${id}`;
  return it;
}

export const demoApi: Api = {
  syncStatus: () => wait(sync),
  syncNow: () => wait((sync = { ...sync, lastSync: new Date().toISOString() }), 600),

  projects: () => wait(projects),
  projectSave(id, meta, vision) {
    const existing = projects.find((p) => p.id === id);
    if (!existing) return fail(`Projet inconnu : ${id}`);
    Object.assign(existing, meta, { vision });
    return wait(existing);
  },
  projectAttach(meta) {
    const project: Project = { ...meta, base: meta.base || "origin/main", vision: "", id: meta.name.toLowerCase(), present: true };
    projects.push(project);
    return wait(project, 500);
  },

  milestones: (project) => wait(milestones[project] ?? []),
  milestoneSave(project, id, meta, goal) {
    const list = (milestones[project] ??= []);
    const existing = list.find((m) => m.id === id);
    if (existing) Object.assign(existing, meta, { goal });
    else list.push({ ...meta, goal, id: meta.title.toLowerCase().replace(/\W+/g, "-") });
    list.sort((a, b) => a.order - b.order);
    return wait(existing ?? list[list.length - 1]);
  },
  milestoneDelete(project, id) {
    milestones[project] = (milestones[project] ?? []).filter((m) => m.id !== id);
    items.filter((i) => i.milestone === id).forEach((i) => delete i.milestone);
    return wait(undefined);
  },

  items: (project) => wait(items.filter((i) => i.project === project)),
  itemCreate(project, draft: ItemDraft) {
    const p = projects.find((x) => x.id === project)!;
    const n = Math.max(0, ...items.filter((i) => i.project === project).map((i) => Number(i.id.split("-")[1]))) + 1;
    const now = new Date().toISOString();
    const created: Item = {
      id: `${p.key}-${n}`,
      project,
      title: draft.title,
      kind: draft.kind,
      status: draft.status ?? (draft.kind === "task" ? "todo" : "inbox"),
      priority: draft.priority ?? "normal",
      milestone: draft.milestone || undefined,
      labels: draft.labels ?? [],
      created: now,
      updated: now,
      body: draft.body ?? "",
    };
    items.push(created);
    return wait(created);
  },
  itemUpdate(id, patch: ItemPatch) {
    const it = findItem(id);
    if (patch.body !== undefined && patch.baseUpdated && patch.baseUpdated !== it.updated) return fail(`${id} a changé entre-temps`);
    const { baseUpdated: _, milestone, ...rest } = patch;
    Object.assign(it, rest);
    if (milestone !== undefined) it.milestone = milestone || undefined;
    if (patch.status) it.closed = ["done", "dropped"].includes(patch.status) ? new Date().toISOString() : undefined;
    it.updated = new Date().toISOString();
    return wait(it);
  },
  itemDelete(id) {
    items.splice(items.indexOf(findItem(id)), 1);
    return wait(undefined);
  },

  sessions: () => wait(sessions, 40),
  sessionStart(req) {
    const it = req.item ? findItem(req.item) : undefined;
    const s: Session = {
      id: `${(req.item ?? req.project).toLowerCase()}-${req.agent}-${sessions.length}`,
      project: req.project,
      item: req.item ?? null,
      title: it?.title ?? (req.agent === "shell" ? "Terminal" : `${req.agent} libre`),
      agent: req.agent,
      cwd: req.path ?? "/Users/ada/code/frostholm",
      branch: req.branch ?? null,
      created: new Date().toISOString(),
      claudeSession: null,
      status: req.agent === "shell" ? "idle" : "working",
      statusAt: new Date().toISOString(),
      message: null,
    };
    sessions.push(s);
    if (it && ["inbox", "backlog", "todo"].includes(it.status)) it.status = "doing";
    return wait(s, 500);
  },
  sessionSetup(project) {
    return demoApi.sessionStart({ project, agent: "claude", place: "main" }).then((s) => {
      const live = sessions.find((x) => x.id === s.id)!;
      live.title = "Configuration de Gamestudio";
      return { ...s, title: live.title };
    });
  },
  sessionResume(id) {
    const s = sessions.find((x) => x.id === id)!;
    s.status = "working";
    return wait(s);
  },
  sessionStop(id) {
    sessions.splice(
      sessions.findIndex((s) => s.id === id),
      1,
    );
    return wait(undefined);
  },

  worktrees: (project) => wait(worktrees[project] ?? [{ path: projects.find((p) => p.id === project)!.path, branch: "main", head: "0000000", main: true, locked: false, dirty: 0, ahead: 0, behind: 0, lastCommit: "Premier commit", lastCommitAt: ago(600) }]),
  worktreeRemove(project, path) {
    worktrees[project] = (worktrees[project] ?? []).filter((w) => w.path !== path);
    return wait(undefined);
  },

  prs: (project) => wait(prs[project] ?? [], 400),
  assets: (project) => wait(project === "frostholm" ? assets : [], 200),
  assetUrl: (path) => path,
  openPath: () => wait(undefined),
  openInVscode: () => wait(undefined),

  updateStatus: () => wait(update),
  updateCheck: () => wait((update = { ...update, state: "ready", latest: "v0.2.0", checkedAt: new Date().toISOString() }), 900),
  restart() {
    update = { ...update, current: "0.2.0", state: "idle" };
    return wait(undefined);
  },

  async ptyAttach(session, _cols, _rows, onData) {
    const text = `\x1b[38;2;137;180;250m${session}\x1b[0m : terminal de démonstration.\r\nLance l'app avec \x1b[1mpnpm tauri dev\x1b[0m pour de vrais terminaux.\r\n`;
    setTimeout(() => onData(new TextEncoder().encode(text)), 50);
    return 1;
  },
  ptyWrite: () => wait(undefined, 0),
  ptyResize: () => wait(undefined, 0),
  ptyDetach: () => wait(undefined, 0),
};
