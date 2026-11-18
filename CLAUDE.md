# Studio

A macOS workbench for developing software with coding agents (games first, any git repo since the rename from
Gamestudio): roadmap and objectives, tasks, bugs, ideas to triage, the agent sessions and worktrees working on them,
assets and PRs. One project at a time, picked with the switcher; everything is segregated by project. Meant to
replace an external tracker (Linear…) and PLAN.md-style checklists.

Nothing in the code is tied to one person's projects, paths or accounts: projects are added by their user (first-run form,
Settings, `studio project add`, or Claude through `studio guide setup`); demo data (`src/lib/demo.ts`) is fictional.
Keep it that way.

UI copy is French (sentence case, plain verbs). Code, identifiers and comments are English.

## Commands

- `pnpm tauri dev`: run the app (Vite on :1430 + Rust core)
- `pnpm dev`: UI only, in a browser, on demo data (`src/lib/demo.ts`); the UI must keep working without Tauri.
  Deep links open a given screen there (`?view=agents&session=fro-1-claude`, `?item=FRO-1`, `?palette=1`;
  `state/store.ts` `openFromUrl`); `scripts/screenshots.sh` uses them to regenerate the README screenshots
  (`docs/screenshots/`, headless Chrome at 2x). Keep both READMEs (`README.md` English, `README.fr.md`) in step
- `pnpm exec vue-tsc --noEmit`: typecheck; `cargo test` in `src-tauri/` (`source ~/.cargo/env` if needed)
- `pnpm app:install`: local release build to /Applications, `studio` CLI in ~/.local/bin, skill in ~/.claude/skills
  (`scripts/install.sh` without `--source` installs the latest GitHub release instead)
- `pnpm release X.Y.Z` (from main): bumps package.json, tauri.conf.json, Cargo.toml, tags `vX.Y.Z` and pushes; the
  Release workflow (`.github/workflows/release.yml`) builds the app and publishes `Studio_X.Y.Z_aarch64.app.tar.gz`

## Architecture (decided)

- **Tauri 2 + Vue 3/TS**, no UI framework: tokens in `src/styles/base.css` + scoped CSS. `src/domain/types.ts` is the
  contract with the Rust core (`src-tauri/src`); `src/state/store.ts` holds state and actions; `commands.rs` is the IPC
  surface (blocking work off the main thread).
- **Data = files on a `studio` branch of each game's own repo** (orphan: no history shared with the code), checked
  out as a small separate clone in `~/.studio/data/<project>/`, never in the game's working tree: `project.md`,
  `milestones/<slug>.md`, `items/<KEY>-<n>.md`, Markdown with a TOML front matter (`store.rs`). Re-read on every
  request (no cache), every read/write under `store::lock()`, atomic writes. One file per item keeps merges between
  machines line-level. The repo's path on this machine is in `~/.studio/repos.json`, never synced.
- **Sync** (`sync.rs`): one worker thread, for each game, commits, fetches only the data branch, rebases onto
  `origin/studio` (`-X theirs`: this machine's line wins where both edited the same line; a notice names the
  files), pushes. A few seconds after a write and every 3 minutes. A rebase that still fails is aborted and reported;
  nothing is overwritten. The data clone's `origin` is the game's `origin` (or the game repo itself when it has
  none). **Adding a game** (`sync::attach`: onboarding, Settings, `studio project add`) clones the data branch when
  the repo has one (another machine), else starts it; an unreachable remote is an error, never a second history. A
  new project's GitHub repo and base branch are read from its repo when left blank (`worktrees::fill_from_repo`).
- **Renamed from Gamestudio** (`migrate.rs`): on first start `~/.gamestudio` moves to `~/.studio` (old path kept as a
  link, worktrees repaired, Claude conversation folders linked under the new path for `--resume`); a data clone still
  on the `gamestudio` branch moves to `studio` at its next sync and the old remote branch is deleted once pushed.
- **Machine-local** (`~/.studio/`, not synced): `sessions.json`, `studio.sock`, `worktrees/<project>/<branch>`,
  `tmux.conf`, `claude-hooks.json`, `briefs/`, `bin/studio` (link to the running binary).
- **Sessions** (`sessions.rs`): tmux on a private server (`tmux -L studio`), shown through xterm.js, so agents outlive
  the app. Claude runs with `--settings ~/.studio/claude-hooks.json` (hooks call `studio hook …` → status
  working/waiting/idle, Claude session id for `--resume`), Codex with `-c notify=[…]`. OpenCode gets a plugin
  (`opencode_plugin.js`, written to `~/.studio/opencode/plugins`) that reports `studio hook opencode` from its
  session events; it is loaded through `OPENCODE_CONFIG_DIR`, with `--standalone` so the server running it sees the
  session's environment. Models and efforts come from each agent's CLI (`models.rs`: `codex debug models`,
  `opencode models` with its cached catalog, cached 5 min; Claude's aliases and efforts are fixed), and a session's
  `model` and `effort` are checked against them (`sessions::choice`). Sessions get `STUDIO_SESSION`,
  `STUDIO_PROJECT`, `STUDIO_ITEM`, `STUDIO_AGENT` and `~/.studio/bin` first on PATH. tmux takes a new session's
  PATH from the creating client, so `tmux::new_session` (and `respawn`) set it on the client too. They also get
  `CLAUDE_CODE_SCROLL_SPEED=3`: Claude Code makes up for xterm.js's damped wheel only when it recognizes xterm.js,
  which tmux hides from it.
- **Playing** (`sessions::play`, `game.rs`): "Lancer le projet" runs the project's `run` command, or one read from the
  repo (Godot: `project.godot` at the root or one folder down, `godot` on PATH or the app in Applications, the .NET
  build for a C# project), in a `game` session in the main checkout or a worktree, so its output shows in a terminal.
  Playing again there restarts it in place (`tmux respawn-pane`): attached views stay attached.
- **`studio` CLI** (`cli.rs` → socket → `control.rs`): the app binary run with arguments. Same code paths as the UI,
  emits `data-changed` / `sessions-changed` so the UI follows. `studio guide setup` (`guide_setup.md`, no app needed)
  is the procedure an agent follows to set a game up; `studio import plan.json [--dry-run]` (`plan.rs`) writes a
  whole plan after validating all of it. "Configurer avec Claude" launches a session told to follow that guide.
- **Orchestration** (`sessions::orchestrate`, `guide_orchestrate.md`): "Orchestrer" starts Claude (`--model opus`,
  `--effort xhigh` unless the user picks another) in its own worktree on a new `orchestration-<date>` branch (`-2`…
  when taken; several can run per game, an item goes to the first whose agent `studio start`s on it), told to
  follow `studio guide orchestrate`: pick parallel work from the roadmap, `studio start` an agent per item, `studio
  wait` / `peek` / `send`, review and merge each branch, open one PR, never push the base. A session started by an
  agent records it as `parent`; started by an orchestrator, it is a `worker`: its worktree branches from the
  orchestrator's branch, its brief ends with the unattended rules (`WORKER_RULES`), and Claude's `idle_prompt`
  notification leaves it idle. Workers run at the effort the user set for the orchestration (the orchestrator's
  `worker_effort`, which wins), else at the one the orchestrator picks per item (`studio start --effort`). A
  session's `effort` is kept so `--resume` gets it again. Orchestrator and workers run in `--permission-mode auto`.
  An agent sends to or stops only its own children, and never types into a session waiting on the user. The Stop
  hook's `last_assistant_message` becomes the session's `message`; SessionStart only records the Claude session id
  (a mid-turn compaction no longer looks idle).
- **Links** between items and work are by naming: a branch or PR title containing `JEU-12` belongs to JEU-12
  (`src/lib/links.ts`). Default branch names start with the lowercase id.
- **Worktrees** (`worktrees.rs`): read from `git worktree list`; created under `~/.studio/worktrees`; removal
  refused while dirty or with a live session (branch kept). **Archive** (`worktrees::archive`, offered on a lane
  once nothing is ahead of the base or its PR merged) also deletes the local branch, after checking again: every
  commit in the freshly fetched base, or a merged PR (`github::merged_pr`) whose head holds the whole branch.
- **Updates** (`update.rs`): an installed app (running from a `.app`) asks `gh release view` on the repo in
  Cargo.toml's `repository` 20 s after start and every 6 h; a newer `vX.Y.Z` is downloaded with `gh` (works on a
  private repo, no token stored), its Info.plist version checked, and swapped in place of the running bundle (copy
  beside it, two renames, rollback on failure), then a notification asks for a restart (`app_restart`). The bundle is
  ad-hoc signed as a whole (`bundle.macOS.signingIdentity: "-"`). The `studio` link and the Claude skill point inside
  the bundle (the skill is a bundle resource), so updates carry them along.
- **Waiting on you**: a session turning `waiting` (hook) gets a system notification with a sound while the window is
  in the background (`sessions::notify_waiting`), a toast with "Voir" while it is in front (`store.ts`
  `announceWaiting`), and the Dock badge counts them.
- **GitHub**: `gh pr list` per project (`github.rs`), polled every 2 min. **Assets**: scan of the project's declared
  folders (`assets.rs`), shown through Tauri's asset protocol.

## Domain

Item kinds: task, bug, idea. Statuses: inbox (à trier) → backlog → todo → doing → review → done, or dropped. A task
starts in todo, a bug or an idea in inbox. Starting an agent on an item moves it to doing. Milestones: planned,
active, done, ordered, with a goal ("Fait quand") and an optional target date. The project file's body is the
game's vision and objectives ("Le cap").

## Design system

- Deep Catppuccin Mocha, so the chrome melts into the Mocha terminals. `--accent` is the current game's color: the
  whole app takes the color of the game you are in.
- **Amber pulsing (`--attention`) means "an agent is waiting on you" and nothing else.**
- Type: Mona Sans variable, wide headings (`font-stretch: 125%`); monospace only for ids, branches, paths, terminals.
- Rows, not card grids (except the asset gallery). No all-caps eyebrows, no middle-dot meta strings.
- Keyboard first: ⌘K palette, ⌘1–7 views, ⌥1–9 games, C/B/I capture, A launch, O orchestrate, P play, j/k + Enter
  in lists, / filter.
  Shortcuts use `KeyboardEvent.code` so they work on AZERTY.

## Next

1. Reboot restore (resume every offline Claude session at once).
2. PR actions (ready for review, merge after checks) and CI logs; asset generation jobs with their outputs linked.
3. Before a public release: an English UI (copy is French only); Developer ID signing and notarization if
   downloads from a browser must open without a Gatekeeper warning; an Intel or universal build.
