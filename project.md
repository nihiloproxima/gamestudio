+++
name = "Studio"
key = "STU"
color = "#f5c2e7"
github = "nihiloproxima/gamestudio"
base = "origin/main"
assets = []
setup = "pnpm install"
run = "pnpm tauri dev"
+++

Un établi macOS pour développer avec des agents de code, jeux d'abord et tout dépôt git ensuite : la roadmap, les tâches, les bugs et les idées, les sessions d'agents et leurs worktrees, les assets et les PR, projet par projet. Il remplace un tracker externe (Linear…) et les checklists PLAN.md.

- Le suivi vit dans le dépôt du projet (branche `studio`), synchronisé entre machines sans service tiers.
- Les agents (Claude, Codex, OpenCode) tournent dans tmux, survivent à l'app et signalent quand ils attendent l'utilisateur.
- Claude configure un projet depuis son dépôt et orchestre des agents en parallèle sur la roadmap.
- Clavier d'abord, une interface sobre qui prend la couleur du projet.
- Prêt pour une sortie publique : UI anglaise, app signée et notarisée, build Intel ou universel.
