import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AgentKind,
  AgentOptions,
  Asset,
  Item,
  ItemDraft,
  ItemPatch,
  Milestone,
  MilestoneMeta,
  OrchestrateRequest,
  Project,
  ProjectMeta,
  PullRequest,
  Session,
  StartRequest,
  SyncStatus,
  UpdateStatus,
  Worktree,
} from "../domain/types";
import { demoApi } from "./demo";

/** False in `pnpm dev` in a plain browser, where the UI runs on demo data. */
export const inTauri = "__TAURI_INTERNALS__" in window;

const tauriApi = {
  syncStatus: () => invoke<SyncStatus>("sync_status"),
  syncNow: () => invoke<SyncStatus>("sync_now"),

  projects: () => invoke<Project[]>("projects_list"),
  projectSave: (id: string, meta: ProjectMeta, vision: string) => invoke<Project>("project_save", { id, meta, vision }),
  /** Adds a game from its repo; its data comes back if the repo already has a `studio` branch. */
  projectAttach: (meta: ProjectMeta) => invoke<Project>("project_attach", { meta }),

  milestones: (project: string) => invoke<Milestone[]>("milestones_list", { project }),
  milestoneSave: (project: string, id: string | null, meta: MilestoneMeta, goal: string) =>
    invoke<Milestone>("milestone_save", { project, id, meta, goal }),
  milestoneDelete: (project: string, id: string) => invoke<void>("milestone_delete", { project, id }),

  items: (project: string) => invoke<Item[]>("items_list", { project }),
  itemCreate: (project: string, draft: ItemDraft) => invoke<Item>("item_create", { project, draft }),
  itemUpdate: (id: string, patch: ItemPatch) => invoke<Item>("item_update", { id, patch }),
  itemDelete: (id: string) => invoke<void>("item_delete", { id }),

  sessions: () => invoke<Session[]>("sessions_list"),
  sessionStart: (req: StartRequest) => invoke<Session>("session_start", { req }),
  /** The models an agent can run and their efforts, as its own CLI reports them. */
  agentOptions: (agent: AgentKind) => invoke<AgentOptions>("agent_options", { agent }),
  sessionSetup: (project: string) => invoke<Session>("session_setup", { project }),
  /** Claude Opus delegating the roadmap's parallel work to agents and merging it on an integration branch. */
  sessionOrchestrate: (req: OrchestrateRequest) => invoke<Session>("session_orchestrate", { req }),
  /** Runs the game from the main checkout or a worktree; again where it runs, restarts it. */
  sessionPlay: (project: string, path?: string) => invoke<Session>("session_play", { project, path }),
  sessionResume: (id: string) => invoke<Session>("session_resume", { id }),
  sessionStop: (id: string) => invoke<void>("session_stop", { id }),
  /** A Claude agent that reviews (`review`) or tests (`qa`) a session's work without changing it. */
  sessionInspect: (id: string, kind: "review" | "qa") => invoke<Session>("session_inspect", { id, kind }),
  /** Sends a review or QA agent's report to the agent whose work it looked at; returns that agent's id. */
  sessionHandOver: (id: string) => invoke<string>("session_hand_over", { id }),

  worktrees: (project: string) => invoke<Worktree[]>("worktrees_list", { project }),
  worktreeRemove: (project: string, path: string) => invoke<void>("worktree_remove", { project, path }),
  /** Removes the worktree and its local branch, once its work is in the base or a merged PR. */
  worktreeArchive: (project: string, path: string) => invoke<void>("worktree_archive", { project, path }),

  prs: (project: string) => invoke<PullRequest[]>("prs_list", { project }),
  assets: (project: string) => invoke<Asset[]>("assets_list", { project }),
  assetUrl: (path: string) => convertFileSrc(path),
  openPath: (path: string, reveal = false) => invoke<void>("path_open", { path, reveal }),
  openInVscode: (path: string) => invoke<void>("path_vscode", { path }),

  updateStatus: () => invoke<UpdateStatus>("update_status"),
  updateCheck: () => invoke<UpdateStatus>("update_check"),
  restart: () => invoke<void>("app_restart"),

  ptyAttach(session: string, cols: number, rows: number, onData: (bytes: Uint8Array) => void) {
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (buf) => onData(new Uint8Array(buf));
    return invoke<number>("pty_attach", { session, cols, rows, onData: channel });
  },
  ptyWrite: (id: number, data: string) => invoke<void>("pty_write", { id, data }),
  ptyResize: (id: number, cols: number, rows: number) => invoke<void>("pty_resize", { id, cols, rows }),
  ptyDetach: (id: number) => invoke<void>("pty_detach", { id }),
};

export type Api = typeof tauriApi;

export const api: Api = inTauri ? tauriApi : demoApi;

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
