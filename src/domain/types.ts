/** The contract with the Rust core (`src-tauri/src/*.rs`); field names follow its camelCase serialization. */

export type ItemKind = "task" | "bug" | "idea";
export type ItemStatus = "inbox" | "backlog" | "todo" | "doing" | "review" | "done" | "dropped";
export type Priority = "urgent" | "high" | "normal" | "low";
export type MilestoneStatus = "planned" | "active" | "done";

export interface ProjectMeta {
  name: string;
  key: string;
  color: string;
  path: string;
  github: string;
  base: string;
  assets: string[];
  setup: string;
}

export interface Project extends ProjectMeta {
  id: string;
  vision: string;
  /** The repo is checked out at `path` on this machine. */
  present: boolean;
}

export interface MilestoneMeta {
  title: string;
  status: MilestoneStatus;
  target?: string;
  order: number;
}

export interface Milestone extends MilestoneMeta {
  id: string;
  goal: string;
}

export interface Item {
  id: string;
  project: string;
  title: string;
  kind: ItemKind;
  status: ItemStatus;
  priority: Priority;
  milestone?: string;
  labels: string[];
  created: string;
  updated: string;
  closed?: string;
  body: string;
}

export interface ItemDraft {
  title: string;
  kind: ItemKind;
  status?: ItemStatus;
  priority?: Priority;
  milestone?: string;
  labels?: string[];
  body?: string;
}

export interface ItemPatch {
  title?: string;
  kind?: ItemKind;
  status?: ItemStatus;
  priority?: Priority;
  /** "" detaches the item from its milestone. */
  milestone?: string;
  labels?: string[];
  body?: string;
  baseUpdated?: string;
}

/** Ordered from most to least urgent: the first one found wins. */
export type AgentStatus = "waiting" | "working" | "idle" | "offline";
export const STATUS_PRIORITY: AgentStatus[] = ["waiting", "working", "idle", "offline"];

export type AgentKind = "claude" | "codex" | "shell";

export interface Session {
  /** Also its tmux session name. */
  id: string;
  project: string;
  item: string | null;
  title: string;
  agent: AgentKind;
  cwd: string;
  branch: string | null;
  created: string;
  claudeSession: string | null;
  status: AgentStatus;
  statusAt: string;
  message: string | null;
}

export interface StartRequest {
  project: string;
  item?: string;
  agent: AgentKind;
  place: "main" | "worktree" | "path";
  branch?: string;
  path?: string;
  prompt?: string;
}

export interface Worktree {
  path: string;
  branch: string | null;
  head: string;
  main: boolean;
  locked: boolean;
  dirty: number;
  ahead: number;
  behind: number;
  lastCommit: string | null;
  lastCommitAt: string | null;
}

export type CheckState = "pass" | "fail" | "pending" | "none";

export interface PullRequest {
  number: number;
  title: string;
  state: "draft" | "open" | "merged" | "closed";
  branch: string;
  base: string;
  url: string;
  author: string;
  updatedAt: string;
  mergedAt: string | null;
  additions: number;
  deletions: number;
  review: "approved" | "changes" | "required" | "none";
  checks: CheckState;
}

export interface Asset {
  path: string;
  rel: string;
  kind: "image" | "audio" | "model" | "source" | "font";
  size: number;
  /** Milliseconds since the epoch. */
  modified: number;
}

export interface SyncStatus {
  syncing: boolean;
  lastSync: string | null;
  /** One line per game in error, prefixed by its name. */
  error: string | null;
  pending: number;
  /** Where each game's data is pushed (its repo's `gamestudio` branch), by project id. */
  remotes: Record<string, string>;
  /** What the last sync merged that is worth knowing. */
  notice: string | null;
}

export interface UpdateStatus {
  current: string;
  /** `disabled` when not running from an installed app (dev build); `ready` once installed, until a restart. */
  state: "disabled" | "idle" | "checking" | "installing" | "ready" | "error";
  latest: string | null;
  error: string | null;
  checkedAt: string | null;
}

export const STATUSES: { id: ItemStatus; label: string }[] = [
  { id: "inbox", label: "À trier" },
  { id: "backlog", label: "Backlog" },
  { id: "todo", label: "À faire" },
  { id: "doing", label: "En cours" },
  { id: "review", label: "En revue" },
  { id: "done", label: "Fait" },
  { id: "dropped", label: "Abandonné" },
];

export const KINDS: { id: ItemKind; label: string }[] = [
  { id: "task", label: "Tâche" },
  { id: "bug", label: "Bug" },
  { id: "idea", label: "Idée" },
];

export const PRIORITIES: { id: Priority; label: string }[] = [
  { id: "urgent", label: "Urgent" },
  { id: "high", label: "Haute" },
  { id: "normal", label: "Normale" },
  { id: "low", label: "Basse" },
];

export const MILESTONE_STATUSES: { id: MilestoneStatus; label: string }[] = [
  { id: "planned", label: "À venir" },
  { id: "active", label: "En cours" },
  { id: "done", label: "Atteint" },
];

export const AGENTS: { id: AgentKind; label: string }[] = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "shell", label: "Terminal" },
];

export const label = <T extends string>(list: { id: T; label: string }[], id: T) =>
  list.find((x) => x.id === id)?.label ?? id;
