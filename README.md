# Gamestudio

Un atelier macOS pour développer ses jeux avec des agents de code : roadmap, tâches, bugs, idées à trier, sessions
Claude Code ou Codex en parallèle (terminaux, worktrees, VS Code), assets et pull requests, un jeu à la fois.

- **Un jeu à la fois** : un sélecteur bascule d'un projet à l'autre, et tout est séparé par projet. La couleur du jeu
  éclaire l'interface.
- **Roadmap** : le cap du jeu (pitch, objectifs) et des jalons avec leur critère « Fait quand », leur échéance et leur
  avancement.
- **Tâches, bugs, idées** : une boîte « à trier » où l'on jette une idée en une ligne, un backlog, et un suivi des
  tâches en cours, en revue, faites.
- **Agents** : chaque worktree avec l'état de sa branche, sa PR et ses sessions. Les terminaux sont intégrés et
  survivent à la fermeture de l'app (tmux). Les hooks de Claude Code disent qui travaille et qui attend une réponse.
- **Assets** : les images et les sons du jeu, les plus récents en tête (ceux que viennent de générer des agents).
- **GitHub** : les pull requests du dépôt, leur CI et leurs revues.
- **Le suivi vit avec le jeu** : un fichier Markdown par élément, sur une branche `gamestudio` du dépôt du jeu, sans
  lien avec l'historique du code. Il se synchronise tout seul ; sur un autre Mac, clone le jeu, ajoute-le, et sa
  roadmap revient.
- **Pensé pour les agents** : la commande `studio` leur donne la fiche de leur tâche et leur permet de noter leurs
  avancées ou une idée. « Configurer avec Claude » lit le dépôt d'un jeu, pose ses questions, montre ce qu'il compte
  créer et remplit la roadmap une fois que tu as dit oui.

L'interface est en français.

## Prérequis

macOS, `git`, `tmux`, et [`gh`](https://cli.github.com) connecté à ton compte (pull requests). Tes jeux dans des
dépôts git.
[Claude Code](https://claude.com/claude-code) et/ou Codex pour les agents. Pour construire l'app : Node, `pnpm` et
Rust.

## Installer

```sh
pnpm install
pnpm app:install      # construit /Applications/Gamestudio.app, la commande `studio` et le skill Claude Code
```

Au premier lancement, indique le dépôt d'un jeu (avec un nom et une clé si c'est un nouveau jeu). Les autres
s'ajoutent dans les réglages ou avec `studio project add` depuis leur dépôt.

## Développer

```sh
pnpm tauri dev        # l'app
pnpm dev              # l'interface seule dans un navigateur, sur des données de démonstration
pnpm exec vue-tsc --noEmit && (cd src-tauri && cargo test)
```

L'architecture est décrite dans [CLAUDE.md](CLAUDE.md).

## La commande `studio`

```sh
studio items                       # ce qui est ouvert dans le projet du dossier courant
studio item JEU-12                 # une fiche
studio idea "un corbeau qui vole les graines"
studio note JEU-12 "slots faits, reste le rendu"
studio status JEU-12 review
studio import plan.json --dry-run  # tout un plan (jalons, éléments, réglages), en aperçu
studio guide setup                 # la marche à suivre pour qu'un agent configure un jeu
studio help
```

## Licence

[MIT](LICENSE)
