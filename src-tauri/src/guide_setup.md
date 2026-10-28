# Configurer un jeu dans Gamestudio

Tu prépares le suivi d'un jeu dans Gamestudio : réglages du projet, cap (vision et objectifs), jalons de la
roadmap, tâches, bugs et idées, chacun dans le bon état. Tu travailles depuis le dépôt du jeu (dossier courant) avec la
commande `studio`. Tu ne modifies **jamais** le dépôt du jeu : tu le lis, tu écris seulement dans Gamestudio.

Règle d'or : **tu n'écris rien dans Gamestudio avant que l'utilisateur ait validé l'aperçu.** Inventaire, questions, aperçu,
puis import.

## 1. Ce qui existe déjà

```sh
studio project          # réglages actuels (si le projet n'existe pas : voir « Nouveau jeu » plus bas)
studio milestones
studio items --all
```

S'il y a déjà des jalons ou des éléments, tu complètes : l'import ignore ce qui porte déjà le même titre.

## 2. Inventaire du dépôt (lecture seule)

Lis, sans tout dérouler, ce qui dit où en est le jeu :

- `README*`, `CLAUDE.md`, `AGENTS.md` : nature du jeu, moteur, conventions, où sont les plans.
- Les documents de plan et de design : `PLAN.md`, `GDD.md`, `ROADMAP*`, `doc/`, `docs/`, `TODO*`, `CHANGELOG*`.
  Repère les étapes, lots, jalons, cases cochées (`- [x]`) ou non, « Fait quand », « État actuel », les idées « pour
  plus tard », les bugs connus.
- L'historique : `git log --oneline -40`, les branches (`git branch -a`), les worktrees (`git worktree list`).
- Les PR : `gh pr list --state open` ; les identifiants de tickets cités (RET-123…) qui trahissent un autre tracker.
- Les `TODO`/`FIXME` du code : compte-les par dossier (`git grep -c -E "TODO|FIXME"`), sans en faire une tâche chacun.
- Les dossiers d'assets (images, sons, modèles) : là où il y en a vraiment, relatifs au dépôt.
- Ce qu'il faut pour travailler dans un worktree neuf (`pnpm install`, rien pour Godot…), et la branche d'intégration
  (main, next…).

## 3. Les questions à l'utilisateur

Avant de proposer quoi que ce soit, pose les questions dont la réponse change le plan, **groupées** (outil
AskUserQuestion s'il est disponible, sinon dans la conversation), avec ta recommandation en premier. Typiquement :

- Quelle source fait foi pour la roadmap (tel document, tel tracker) et faut-il reprendre ce qui est déjà fait (en
  « fait », pour l'historique) ou seulement ce qui reste ?
- Quel jalon est en cours, et ses échéances s'il y en a ?
- La granularité : une tâche par étape du plan, ou un regroupement ?
- Les idées « pour plus tard » : à trier (inbox) ou directement au backlog ?
- Les réglages incertains : branche de base des worktrees, commande d'installation, dossiers d'assets.
- Un tracker externe (Linear…) à reprendre ? Si oui et qu'un outil y donne accès, propose de l'importer de la même
  façon ; sinon demande un export.

Ne pose pas une question dont la réponse est dans le dépôt.

## 4. Le plan

Écris le plan dans un fichier JSON hors du dépôt (`/tmp` ou ton dossier de travail), au format :

```json
{
  "project": {
    "vision": "Le pitch en deux lignes, puis les objectifs en liste Markdown.",
    "assets": ["assets", "content"],
    "base": "origin/main",
    "setup": "",
    "github": "proprietaire/depot"
  },
  "milestones": [
    { "title": "Jalon 3 : progression", "status": "active", "target": "2026-11-15",
      "goal": "**Fait quand** les maîtrises et les armures donnent envie de rejouer." }
  ],
  "items": [
    { "title": "Armures : slot de torse", "kind": "task", "status": "todo", "priority": "high",
      "milestone": "Jalon 3 : progression", "labels": ["progression"],
      "body": "Contexte, critère « Fait quand », fichiers concernés." }
  ]
}
```

Valeurs possibles :

- `kind` : `task` (par défaut), `bug`, `idea`.
- `status` : `inbox` (à trier), `backlog` (trié, pas prévu), `todo` (prévu), `doing`, `review`, `done`, `dropped`.
  Sans statut, une tâche va en `todo`, un bug ou une idée en `inbox`.
- `priority` : `urgent`, `high`, `normal` (par défaut), `low`.
- Jalon `status` : `planned`, `active` (un seul en principe), `done` ; `target` au format AAAA-MM-JJ.
- `milestone` d'un élément : le titre (ou l'id) d'un jalon du plan ou du projet.
- `project` : seulement les réglages à changer (`name`, `color`, `path`, `github`, `base`, `assets`, `setup`,
  `vision`).

Pour bien remplir :

- Titres courts, dans la langue du projet, à l'infinitif ou en nom (« Arbre du bâton de glace »), sans préfixe de numéro.
- Le `body` porte le contexte utile à un agent : critère « Fait quand », fichiers, décisions déjà prises, liens.
- L'état vient du dépôt : case cochée ou étape finie dans `git log` → `done` ; étape en cours → `doing` ; idée « pour
  plus tard » → `idea`. N'invente pas de travail que ni le dépôt ni l'utilisateur n'évoquent.
- Les jalons dans l'ordre chronologique : ils s'ajoutent après ceux qui existent.

## 5. Aperçu, validation, import

```sh
studio import /chemin/plan.json --dry-run
```

Montre le résumé à l'utilisateur (jalons, nombre d'éléments par statut, quelques exemples) et ajuste le plan selon ses
remarques. **Seulement après son accord explicite :**

```sh
studio import /chemin/plan.json
```

L'import vérifie tout avant d'écrire : en cas d'erreur, rien n'est écrit et la liste des problèmes s'affiche. Le
relancer après correction est sans risque (ce qui existe déjà est ignoré). Pour une retouche ponctuelle ensuite :
`studio set <ID> …`, `studio milestone set <id> …`, `studio project set …`.

## 6. Pour finir

Termine par un court bilan : ce qui a été créé, ce qui reste à décider, et ce que l'utilisateur devrait vérifier dans l'app.
Si les docs du dépôt (CLAUDE.md, PLAN.md) font double emploi avec Gamestudio, propose-lui de les faire pointer vers
`studio` (« les tâches vivent dans Gamestudio : `studio items` »), sans le faire toi-même.

## Nouveau jeu

Si `studio project` répond que le dossier n'appartient à aucun projet, demande à l'utilisateur le nom du jeu et une
clé de 2 à 5 majuscules pour numéroter ses éléments (JEU, RPG…), puis, avec son accord, depuis le dépôt du jeu :

```sh
studio project add --name "Nom" --key CLE
```

Le suivi vivra sur une branche `gamestudio` du dépôt, sans lien avec l'historique du code. Si cette branche existe
déjà (un autre Mac), ses données reviennent telles quelles : reprends alors à l'étape 1 pour compléter, pas pour
refaire.
