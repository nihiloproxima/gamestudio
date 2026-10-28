# Gamestudio

A macOS workbench for developing games with coding agents: roadmap and objectives, tasks, bugs, ideas to triage, the
agent sessions and worktrees working on them, assets and PRs. One game at a time, picked with the switcher;
everything is segregated by project. Meant to replace an external tracker (Linear…) and PLAN.md-style checklists.

Nothing in the code is tied to one person's games, paths or accounts: games are added by their user (first-run form,
Settings, `studio project add`, or Claude through `studio guide setup`); demo data (`src/lib/demo.ts`) is fictional.
Keep it that way.

UI copy is French (sentence case, plain verbs). Code, identifiers and comments are English.

## Commands

- `pnpm tauri dev`: run the app (Vite on :1430 + Rust core)
- `pnpm dev`: UI only, in a browser, on demo data (`src/lib/demo.ts`); the UI must keep working without Tauri
- `pnpm exec vue-tsc --noEmit`: typecheck; `cargo test` in `src-tauri/` (`source ~/.cargo/env` if needed)
- `pnpm app:install`: local release build to /Applications, `studio` CLI in ~/.local/bin, skill in ~/.claude/skills
  (`scripts/install.sh` without `--source` installs the latest GitHub release instead)
- `pnpm release X.Y.Z` (from main): bumps package.json, tauri.conf.json, Cargo.toml, tags `vX.Y.Z` and pushes; the
  Release workflow (`.github/workflows/release.yml`) builds the app and publishes `Gamestudio_X.Y.Z_aarch64.app.tar.gz`

## Architecture (decided)

- **Tauri 2 + Vue 3/TS**, no UI framework: tokens in `src/styles/base.css` + scoped CSS. `src/domain/types.ts` is the
  contract with the Rust core (`src-tauri/src`); `src/state/store.ts` holds state and actions; `commands.rs` is the IPC
  surface (blocking work off the main thread).
- **Data = files on a `gamestudio` branch of each game's own repo** (orphan: no history shared with the code), checked
  out as a small separate clone in `~/.gamestudio/data/<project>/`, never in the game's working tree: `project.md`,
  `milestones/<slug>.md`, `items/<KEY>-<n>.md`, Markdown with a TOML front matter (`store.rs`). Re-read on every
  request (no cache), every read/write under `store::lock()`, atomic writes. One file per item keeps merges between
  machines line-level. The repo's path on this machine is in `~/.gamestudio/repos.json`, never synced.
- **Sync** (`sync.rs`): one worker thread, for each game, commits, fetches only the data branch, rebases onto
  `origin/gamestudio` (`-X theirs`: this machine's line wins where both edited the same line; a notice names the
  files), pushes. A few seconds after a write and every 3 minutes. A rebase that still fails is aborted and reported;
  nothing is overwritten. The data clone's `origin` is the game's `origin` (or the game repo itself when it has
  none). **Adding a game** (`sync::attach`: onboarding, Settings, `studio project add`) clones the data branch when
  the repo has one (another machine), else starts it; an unreachable remote is an error, never a second history. A
  new project's GitHub repo and base branch are read from its repo when left blank (`worktrees::fill_from_repo`).
- **Machine-local** (`~/.gamestudio/`, not synced): `sessions.json`, `studio.sock`, `worktrees/<project>/<branch>`,
  `tmux.conf`, `claude-hooks.json`, `briefs/`, `bin/studio` (link to the running binary).
- **Sessions** (`sessions.rs`): tmux on a private server (`tmux -L studio`), shown through xterm.js, so agents outlive
  the app. Claude runs with `--settings ~/.gamestudio/claude-hooks.json` (hooks call `studio hook …` → status
  working/waiting/idle, Claude session id for `--resume`), Codex with `-c notify=[…]`. Sessions get `STUDIO_SESSION`,
  `STUDIO_PROJECT`, `STUDIO_ITEM`, `STUDIO_AGENT` and `~/.gamestudio/bin` first on PATH. tmux takes a new session's
  PATH from the creating client, so `tmux::new_session` sets it on the client too.
- **`studio` CLI** (`cli.rs` → socket → `control.rs`): the app binary run with arguments. Same code paths as the UI,
  emits `data-changed` / `sessions-changed` so the UI follows. `studio guide setup` (`guide_setup.md`, no app needed)
  is the procedure an agent follows to set a game up; `studio import plan.json [--dry-run]` (`plan.rs`) writes a
  whole plan after validating all of it. "Configurer avec Claude" launches a session told to follow that guide.
- **Links** between items and work are by naming: a branch or PR title containing `JEU-12` belongs to JEU-12
  (`src/lib/links.ts`). Default branch names start with the lowercase id.
- **Worktrees** (`worktrees.rs`): read from `git worktree list`; created under `~/.gamestudio/worktrees`; removal
  refused while dirty or with a live session (branch kept).
- **Updates** (`update.rs`): an installed app (running from a `.app`) asks `gh release view` on the repo in
  Cargo.toml's `repository` 20 s after start and every 6 h; a newer `vX.Y.Z` is downloaded with `gh` (works on a
  private repo, no token stored), its Info.plist version checked, and swapped in place of the running bundle (copy
  beside it, two renames, rollback on failure), then a notification asks for a restart (`app_restart`). The bundle is
  ad-hoc signed as a whole (`bundle.macOS.signingIdentity: "-"`). The `studio` link and the Claude skill point inside
  the bundle (the skill is a bundle resource), so updates carry them along.
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
- Keyboard first: ⌘K palette, ⌘1–7 views, ⌥1–9 games, C/B/I capture, A launch, j/k + Enter in lists, / filter.
  Shortcuts use `KeyboardEvent.code` so they work on AZERTY.

## Next

1. Notifications when an agent starts waiting (the notification plugin is in, used by updates); reboot restore
   (resume every offline Claude session at once).
2. PR actions (ready for review, merge after checks) and CI logs; asset generation jobs with their outputs linked.
3. Before a public release: an English UI (copy is French only); Developer ID signing and notarization if
   downloads from a browser must open without a Gatekeeper warning; an Intel or universal build.
