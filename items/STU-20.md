+++
title = "Choisir l'agent, le modèle et l'effort des sessions d'orchestration"
kind = "task"
status = "todo"
priority = "normal"
milestone = "0-3-actions-et-reprise"
created = "2026-11-20T09:58:07Z"
updated = "2026-11-20T09:58:07Z"
+++

La feuille « Orchestrer » (`OrchestrateSheet.vue`) règle déjà l'effort de l'orchestrateur et celui des agents (`worker_effort`), mais l'orchestrateur est toujours Claude Opus et les workers toujours Claude.

- Orchestrateur : choisir le modèle (Opus par défaut), et l'effort comme aujourd'hui.
- Agents : choisir l'agent (Claude, Codex, OpenCode) et son modèle, ou « Claude décide », avec un effort tiré des efforts de cet agent (`models.rs`, `sessions::choice`). Aujourd'hui `worker_effort` ne s'applique qu'à Claude (`sessions.rs`, `fixed … agent == "claude"`).
- Garder le choix sur la session de l'orchestrateur (comme `worker_effort`) pour que `studio start` l'applique à ses workers, et le faire connaître au brief (`orchestrate_prompt`).

**Fait quand** on lance une orchestration en choisissant le modèle de l'orchestrateur et l'agent/modèle/effort des workers, et que les workers démarrés par `studio start` les respectent.
