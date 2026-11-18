---
name: studio
description: Read and update the Studio project tracker with the `studio` CLI — tasks, bugs, ideas, milestones, project settings, agent sessions. Use when working in a repo tracked by Studio (a game, a web app, any project) and the task touches what to do next, an item id like JEU-12, logging progress, capturing an idea or a bug for later, or when asked to set a project up in Studio (then run `studio guide setup` first) or to orchestrate agents on its roadmap (`studio guide orchestrate`).
---

# Studio (`studio`)

Studio tracks a project's roadmap (milestones), tasks, bugs and ideas, the agent sessions working on it, its assets
and PRs. The `studio` CLI talks to the running app; the project is inferred from the current directory (the project's
repo or one of its worktrees), else pass `--project <name|key>`. Sessions launched from the app already have
`STUDIO_PROJECT`, `STUDIO_ITEM` and `studio` on their PATH.

## While working on an item

```sh
studio item JEU-12                    # the full record: status, milestone, description, notes
studio note JEU-12 "Décision : …"     # dated note, for decisions and checkpoints worth keeping
studio status JEU-12 review           # ready to check; done once the user verified it
studio idea "…" --body "…"            # an idea outside the current scope: capture it, don't pursue it
studio bug "…" --body "…"             # a bug noticed on the way
```

Write titles and notes in the project's language. Never mark an item `done` that the user has not verified when it
needs trying by hand (in game, in the app).

## Finding what to do

```sh
studio items                          # open items (todo, doing, review, inbox, backlog)
studio items --status todo
studio milestones
studio sessions                       # who else is working, and where
```

## Changing things

```sh
studio add "Titre" --kind task|bug|idea --status todo --priority high --milestone "Jalon 3" --labels a,b --body -
studio set JEU-12 --title … --priority … --milestone … --labels …     # --milestone "" detaches it
studio milestone add "Démo Steam" --status planned --target 2026-12-01 --goal "Fait quand …"
studio milestone set <id> --status active
studio project set --assets assets,content --base origin/main --setup "pnpm install" --vision -
studio start JEU-12 --branch jeu-12-armures   # starts a Claude session on it, in a new worktree
studio start JEU-12 --branch jeu-12-armures --effort high   # same, at a given effort (low, medium, high, xhigh, max)
studio start JEU-12 --agent codex --model gpt-6.1-sol --effort high   # a Codex session; models and efforts as the agent's CLI lists them
studio start JEU-12 --agent opencode --model opencode-go/minimax-m3.1-flash --effort high   # an OpenCode session (effort = the model's variant)
studio start --preset groom                # a free session, no item, no worktree: it grooms the roadmap, auto mode
```

## Orchestrating

When launched as a project's orchestrator, or asked to orchestrate agents on its roadmap, run `studio guide orchestrate`
and follow it. The agents you start with `studio start` are yours to drive, and only yours:

```sh
studio wait                           # until one of your agents ends its turn, waits or stops (9 min at most)
studio peek fro-12-claude             # the end of its screen
studio send fro-12-claude "…"         # typed into it and submitted; refused while it waits on the user
studio stop fro-12-claude
studio title "Combat : armures et Nova (FRO-1, FRO-2)"   # what you work on, shown above your agents in the app
```

## Review and QA

A second Claude reads (review) or tests (QA) a session's work in its directory, and cannot change it: no file edits,
no git or `gh pr` writes. Its last message is its report.

```sh
studio review                         # a review of your own work (from inside your session)
studio review fro-12-claude --qa --context "Surtout le menu pause"   # QA of one of your agents' work
studio wait fro-12-claude-2           # until it has reported
studio report fro-12-claude-2         # its report
```

Running one again replaces the last one of that kind. Fix what the report gets right, and say what you set aside and
why.

## Setting a project up

When asked to set up, import or (re)configure a project in Studio, run `studio guide setup` and follow it: read the
repo, ask the user the questions that change the plan, show `studio import plan.json --dry-run`, and import only
after their explicit go.

`studio help` lists every command; `--json` gives raw output.
