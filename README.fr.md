# Gamestudio

[English](README.md) | **Français**

Un atelier macOS pour faire ses jeux avec des agents de code. La roadmap du jeu, ses tâches, bugs et idées, les
sessions Claude Code ou Codex qui y travaillent avec leurs worktrees et leurs pull requests, et les images et sons du
jeu : un jeu à la fois, dans une seule fenêtre.

![Vue d'ensemble d'un jeu : jalon en cours, agents qui t'attendent, tâches en cours, pull requests](docs/screenshots/overview.png)

> Gamestudio tourne sur les Mac Apple Silicon.

## Ce qu'il fait

- **Un jeu à la fois.** Un sélecteur (⌥1–9) passe d'un jeu à l'autre ; tout le reste appartient au jeu courant, et
  la fenêtre prend sa couleur.
- **Roadmap.** Le pitch et les objectifs du jeu, puis des jalons avec leur « Fait quand », leur échéance et leur
  avancement.
- **Tâches, bugs, idées.** Une idée ou un bug se note en une ligne, se trie plus tard vers le backlog ou le plan, et
  on suit ce qui est en cours, en revue, fait.
- **Agents.** Claude Code, Codex ou un simple terminal se lancent sur une tâche, dans le dépôt principal ou un
  worktree neuf. Chaque worktree montre sa branche, ses changements non commités et sa pull request ; chaque agent
  montre s'il travaille, t'attend ou se repose. Les terminaux vivent dans tmux : les agents continuent quand l'app
  se ferme.
- **Orchestration.** Claude Opus prend dans la roadmap ce qui peut avancer en parallèle, le confie à des agents
  chacun dans son worktree, relit et fusionne leur travail sur une branche d'intégration, et ouvre une seule pull
  request.
- **Assets.** Les images et les sons du jeu, les plus récents en tête : ce qu'un agent vient de générer apparaît.
- **GitHub.** Les pull requests du dépôt, leur CI et leurs revues, reliées aux tâches qu'elles traitent.
- **Le suivi voyage avec le jeu.** Il vit sur une branche `gamestudio` du dépôt du jeu, un fichier Markdown par
  élément. On clone le jeu sur un autre Mac, on l'ajoute, et sa roadmap revient.
- **Pensé pour les agents.** La commande `studio` donne à un agent sa tâche et lui permet de noter ses avancées ou
  de garder une idée pour plus tard ; « Configurer avec Claude » laisse Claude lire le dépôt d'un jeu et remplir la
  roadmap une fois que tu as dit oui.

## En images

| | |
| --- | --- |
| ![Roadmap avec les jalons et leur avancement](docs/screenshots/roadmap.png) **Roadmap.** Le cap, puis les jalons sur un fil : atteints, en cours, à venir. | ![Liste des tâches avec une tâche ouverte sur le côté](docs/screenshots/tasks.png) **Tâches.** Statut, priorité et jalon se changent sur place ; une tâche s'ouvre sur le côté avec sa description et le travail qui s'y rattache. |
| ![Vue Agents : worktrees, sessions et terminal](docs/screenshots/agents.png) **Agents.** Les worktrees avec leur état et leurs sessions ; le terminal de l'agent choisi, et ce qu'il attend. | ![Idées et backlog avec la saisie en une ligne](docs/screenshots/backlog.png) **Idées et backlog.** Une ligne pour noter, puis le tri : backlog, plan ou abandon. |
| ![Galerie d'assets avec les fichiers récents signalés](docs/screenshots/assets.png) **Assets.** Les images sur un damier, nettes pour le pixel art ; les sons s'écoutent sur place. | ![Palette de commandes](docs/screenshots/palette.png) **⌘K.** N'importe quelle tâche, vue, jeu ou action, au clavier. |

## Comment ça marche

### Le suivi vit dans le dépôt du jeu

Les données d'un jeu sont sur une branche orpheline `gamestudio` de son dépôt : elle ne partage aucun historique avec
le code et ne touche jamais ta copie de travail ni `main`. Gamestudio en garde une petite copie dans
`~/.gamestudio/data/<jeu>` :

```
project.md            nom, clé des éléments, couleur, dossiers d'assets, pitch et objectifs du jeu
milestones/demo.md    un jalon : statut, échéance, « Fait quand »
items/FRO-12.md       une tâche, un bug ou une idée
```

Un élément est du Markdown simple avec un en-tête TOML, lisible et modifiable à la main :

```markdown
+++
title = "Armures : slot de torse et de jambes"
kind = "task"
status = "doing"
priority = "high"
milestone = "jalon-3-progression"
+++

Les armures donnent de la défense et un malus de vitesse.

- [x] Slots dans l'inventaire
- [ ] Rendu sur le personnage
```

Une modification est commitée et poussée quelques secondes après, et celles des autres machines arrivent toutes les
trois minutes. Quand deux Mac modifient le même élément, git fusionne ligne par ligne ; là où ils divergent sur une
ligne, la machine qui synchronise l'emporte et l'autre version reste dans l'historique. L'emplacement du dépôt sur
chaque Mac est gardé localement (`~/.gamestudio/repos.json`).

### Les agents tournent dans tmux

Chaque session tourne dans le serveur tmux de Gamestudio, affiché dans l'app par xterm.js. Fermer ou mettre à jour
l'app laisse les agents tourner ; `tmux -L studio attach` les retrouve depuis n'importe quel terminal. Un agent
lancé sur une tâche la reçoit comme consigne, et une session dans un nouveau worktree a une branche au nom de la
tâche (`fro-12-armures`), pour que sa pull request s'y rattache. Claude Code est lancé avec des hooks (via
`--settings`, tes réglages globaux ne sont pas touchés) qui disent à l'app quand il travaille, t'attend ou a fini :
un agent qui attend pulse en ambre et se compte sur l'icône du Dock. Après un redémarrage, un clic reprend la
conversation Claude d'une session.

Le jeu tourne de la même façon (<kbd>P</kbd>, ou depuis n'importe quel worktree pour essayer la branche d'un agent) :
sa sortie reste dans un terminal, et le relancer le redémarre sur le code du moment. Gamestudio reconnaît un projet
Godot ; pour un autre moteur, indique la commande dans les réglages.

### La commande `studio`

Le binaire de l'app sert aussi de ligne de commande, pour toi et pour les agents. Les sessions lancées par l'app
l'ont dans leur PATH :

```sh
studio items                        # ce qui est ouvert dans le jeu du dossier courant
studio item FRO-12                  # une tâche, en entier
studio note FRO-12 "slot de torse fait, reste les jambes"
studio status FRO-12 review
studio idea "un corbeau qui vole les graines"
studio import plan.json --dry-run   # tout un plan (jalons, éléments, réglages), en aperçu
studio wait                         # pour un agent : jusqu'à ce qu'un des agents qu'il a lancés finisse ou demande
studio help                         # tout le reste
```

### Laisser Claude configurer un jeu

« Configurer avec Claude » lance une session Claude dans le dépôt du jeu, qui suit `studio guide setup`. Claude lit
le dépôt (plans, documents de design, historique git, pull requests, TODO, dossiers d'assets), te pose les questions
qui changent le plan, montre un aperçu avec `studio import --dry-run`, et n'écrit rien avant ton accord.

### Laisser Claude orchestrer des agents

« Orchestrer » (<kbd>O</kbd>, ou dans la vue des agents) lance Claude Opus en effort `xhigh` dans un worktree à lui,
sur une nouvelle branche `orchestration-…`, qui suit `studio guide orchestrate`. Il lit la roadmap et le code,
prévoit des vagues de tâches et de bugs prévus qui touchent des parties distinctes du jeu, et lance un agent Claude
sur chacun avec `studio start`, chacun dans un worktree parti de sa branche d'intégration. Il les attend avec
`studio wait`, lit leur écran avec `studio peek`, leur répond avec `studio send`, relit et fusionne chaque branche
dès qu'elle est prête, résout les conflits, lance les vérifications du projet après chaque fusion, puis publie sa
branche et ouvre une pull request dont le titre cite chaque tâche. Il ne pousse jamais sur la branche de base.

Tout tourne sans surveillance, dans le mode auto de Claude. Les agents de l'orchestrateur reçoivent des règles en
plus de leur tâche (trancher et le noter, commiter sans pousser, finir par une note et `review`) et ne te dérangent
pas : celui qui finit son tour attend l'orchestrateur, pas toi. Seule une permission que Claude refuse de s'accorder
fait passer un agent en ambre, et toi seul peux y répondre : un agent ne pilote que ceux qu'il a lancés, et ne répond
jamais à une permission. Ce qui demande ta décision (une idée, une tâche floue, un choix de design) est laissé de
côté avec une note. Les tâches fusionnées restent en revue jusqu'à ce que tu les aies essayées.

### Mises à jour

Les releases sont construites par GitHub Actions. L'app installée en cherche une nouvelle au démarrage puis toutes
les six heures (via `gh`), l'installe à sa place et t'envoie une notification pour redémarrer ; les agents
continuent de tourner pendant ce temps.

## Prérequis

- Un Mac Apple Silicon
- `git`, `tmux`, et la [CLI GitHub](https://cli.github.com) connectée (`gh auth login`)
- [Claude Code](https://claude.com/claude-code) et/ou Codex pour les agents
- Tes jeux dans des dépôts git

## Installer

```sh
curl -fsSL https://raw.githubusercontent.com/nihiloproxima/gamestudio/main/scripts/install.sh | bash
```

Le script installe la dernière release dans `/Applications`, la commande `studio` dans `~/.local/bin` et le skill
Claude Code dans `~/.claude/skills`. Les deux pointent dans l'app, donc les mises à jour les emportent. Depuis un
clone, `pnpm app:install` construit et installe à partir des sources.

## Premiers pas

1. **Ajoute un jeu.** Indique son dépôt, avec un nom et une clé pour ses éléments (`FRO` donne `FRO-1`, `FRO-2`…).
   Si le dépôt a déjà une branche `gamestudio`, ses données reviennent telles quelles.
2. **Remplis-le.** Laisse Claude le faire à partir du dépôt (« Configurer avec Claude »), ou crée les jalons dans la
   roadmap, les tâches avec <kbd>C</kbd>, les bugs avec <kbd>B</kbd> et les idées avec <kbd>I</kbd>.
3. **Lance un agent** sur une tâche avec <kbd>A</kbd> : Claude, Codex ou un terminal, dans le dépôt principal ou un
   nouveau worktree. Quand il a besoin de toi, son point passe à l'ambre.
4. **Ajoute tes autres jeux** dans les Réglages, « Nouveau jeu », ou avec `studio project add` depuis leur dépôt.

| Raccourci | |
| --- | --- |
| <kbd>⌘K</kbd> | recherche et actions |
| <kbd>⌘1</kbd>–<kbd>⌘7</kbd> | vue d'ensemble, roadmap, tâches, idées, agents, assets, GitHub |
| <kbd>⌥1</kbd>–<kbd>⌥9</kbd> | changer de jeu |
| <kbd>C</kbd> <kbd>B</kbd> <kbd>I</kbd> | nouvelle tâche, nouveau bug, nouvelle idée |
| <kbd>A</kbd> | lancer un agent |
| <kbd>O</kbd> | orchestrer des agents |
| <kbd>P</kbd> | lancer le jeu |
| <kbd>J</kbd> <kbd>K</kbd> <kbd>Entrée</kbd> | parcourir une liste, ouvrir |
| <kbd>/</kbd> | filtrer |

## Développer

```sh
pnpm install
pnpm tauri dev                 # l'app
pnpm dev                       # l'interface seule dans un navigateur, sur un studio de démonstration fictif
pnpm exec vue-tsc --noEmit && (cd src-tauri && cargo test)
scripts/screenshots.sh         # régénère ces captures à partir de la démo
pnpm release 0.2.0             # depuis main : version, tag et push ; la CI publie la release
```

Tauri 2, avec un cœur en Rust et une interface Vue 3. L'architecture est décrite dans [CLAUDE.md](CLAUDE.md).

## Licence

[MIT](LICENSE)
