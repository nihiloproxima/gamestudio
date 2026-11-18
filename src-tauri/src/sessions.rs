//! Agent and shell sessions, each a tmux session on Studio's private server so it outlives the app. Records live
//! in `~/.studio/sessions.json`, on this machine only; liveness is read from tmux, status comes from the agents'
//! hooks (`studio hook …`), never from screen scraping.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::{game, models, paths, store, tmux, worktrees};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Also the tmux session name.
    pub id: String,
    pub project: String,
    pub item: Option<String>,
    pub title: String,
    /// `claude`, `codex`, `opencode`, `shell`, or `game` (the game running, `play`).
    pub agent: String,
    pub cwd: String,
    pub branch: Option<String>,
    pub created: String,
    /// Claude's own session id, to `claude --resume` it after a reboot.
    #[serde(default)]
    pub claude_session: Option<String>,
    /// `working`, `waiting` (on the user) or `idle`; `offline` once tmux lost it.
    pub status: String,
    pub status_at: String,
    #[serde(default)]
    pub message: Option<String>,
    /// The session that launched this one with `studio start`, if any: the only one, besides the user, that may
    /// drive it (`studio send`, `studio stop`).
    #[serde(default)]
    pub parent: Option<String>,
    /// `orchestrator` (picks the work, delegates it, merges it) or `worker` (launched by an orchestrator). Both run
    /// unattended, in Claude's auto mode.
    #[serde(default)]
    pub role: Option<String>,
    /// The model the agent runs (`--model`, `-m`); its own default without one.
    #[serde(default)]
    pub model: Option<String>,
    /// The agent's effort (`--effort` for Claude, `model_reasoning_effort` for Codex); its default without one.
    #[serde(default)]
    pub effort: Option<String>,
    /// An orchestrator's: the effort the user set for all its agents. Without one, it picks each agent's.
    #[serde(default)]
    pub worker_effort: Option<String>,
}

static RECORDS: Mutex<Vec<Session>> = Mutex::new(Vec::new());
/// Held while an orchestrator's agent claims its item, so two orchestrators cannot both start one on the same item.
static CLAIM: Mutex<()> = Mutex::new(());

fn file() -> PathBuf {
    paths::studio_dir().join("sessions.json")
}

pub fn load() {
    let records = std::fs::read_to_string(file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    *RECORDS.lock().unwrap() = records;
}

fn save(records: &[Session]) {
    if let Err(e) = paths::write_atomic(&file(), &serde_json::to_string_pretty(records).unwrap_or_default()) {
        eprintln!("{e}");
    }
}

fn changed(app: &AppHandle) {
    let waiting = list().iter().filter(|s| s.status == "waiting").count();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_badge_count((waiting > 0).then_some(waiting as i64));
    }
    let _ = app.emit("sessions-changed", ());
}

pub fn list() -> Vec<Session> {
    let alive = tmux::sessions();
    RECORDS
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .map(|mut s| {
            if !alive.contains(&s.id) {
                s.status = "offline".into();
            }
            s
        })
        .collect()
}

fn find(id: &str) -> Result<Session, String> {
    list().into_iter().find(|s| s.id == id).ok_or_else(|| format!("Session inconnue : {id}"))
}

/// The session `id`, when `caller` may drive it: a person (no caller) drives any session, an agent only those it
/// launched.
pub fn driven(id: &str, caller: Option<&str>) -> Result<Session, String> {
    let session = find(id)?;
    match caller {
        Some(c) if session.parent.as_deref() != Some(c) => Err(format!("{id} n'a pas été lancée par cette session")),
        _ => Ok(session),
    }
}

/// A live session whose directory is `dir` or inside it.
pub fn live_in(dir: &Path) -> Option<String> {
    let dir = paths::canonical(dir);
    list().into_iter().find(|s| s.status != "offline" && Path::new(&s.cwd).starts_with(&dir)).map(|s| s.id)
}

/// Holds a `studio` link to the running binary, first on every session's PATH: agents reach the CLI even before
/// it is installed, and always the version of the app that launched them.
fn bin_dir() -> PathBuf {
    paths::studio_dir().join("bin")
}

/// What agents call back (hooks, notify, plugin): the `studio` link, not the binary's own path. An agent reads these
/// once, when it starts, and keeps running across installs that move or rename the app; the link follows them.
fn studio_link() -> String {
    bin_dir().join("studio").display().to_string()
}

pub fn link_cli() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let link = bin_dir().join("studio");
    if std::fs::read_link(&link).is_ok_and(|target| target == exe) {
        return Ok(());
    }
    std::fs::create_dir_all(bin_dir()).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&exe, &link).map_err(|e| format!("Lien {} impossible : {e}", link.display()))
}

fn hooks_file() -> PathBuf {
    paths::studio_dir().join("claude-hooks.json")
}

/// Claude Code settings layered on the sessions Studio launches (`claude --settings`), so their hooks report to
/// the app without touching the user's global settings. Rewritten at startup with the current binary.
pub fn write_hooks() -> Result<(), String> {
    let exe = paths::shell_quote(&studio_link());
    let hook = |event: &str| {
        serde_json::json!([{ "hooks": [{ "type": "command", "command": format!("{exe} hook {event}"), "timeout": 5 }] }])
    };
    let settings = serde_json::json!({
        "hooks": {
            "SessionStart": hook("start"),
            "UserPromptSubmit": hook("prompt"),
            "PreToolUse": hook("tool"),
            "Notification": hook("notification"),
            "Stop": hook("stop"),
        }
    });
    paths::write_atomic(&hooks_file(), &serde_json::to_string_pretty(&settings).unwrap())
}

fn opencode_config_dir() -> PathBuf {
    paths::studio_dir().join("opencode")
}

/// The OpenCode plugin that reports a session's status to the app (`studio hook opencode`). It sits in the config
/// directory of OpenCode sessions only (`OPENCODE_CONFIG_DIR`), so the user's own OpenCode setup is left alone.
pub fn write_opencode_plugin() -> Result<(), String> {
    let exe = serde_json::to_string(&studio_link()).map_err(|e| e.to_string())?;
    let source = include_str!("opencode_plugin.js").replace("__STUDIO_EXE__", &exe);
    let plugins = opencode_config_dir().join("plugins");
    // Its name under Gamestudio: left there, it would report every status twice.
    let _ = std::fs::remove_file(plugins.join("gamestudio.js"));
    paths::write_atomic(&plugins.join("studio.js"), &source)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub project: String,
    pub item: Option<String>,
    /// `claude`, `codex`, `opencode` or `shell`.
    pub agent: String,
    /// `main` (the repo's checkout), `worktree` (new, for `branch`) or `path` (an existing worktree).
    pub place: String,
    pub branch: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub prompt: String,
    /// A free session's mission, without an item: `groom` (organize the roadmap) or `brainstorm`.
    #[serde(default)]
    pub preset: Option<String>,
    /// Shown instead of the item's title or the branch.
    #[serde(default)]
    pub title: Option<String>,
    /// The session running `studio start`, set by the control socket.
    #[serde(skip)]
    pub parent: Option<String>,
    /// Set by `orchestrate`; a worker's role comes from its parent.
    #[serde(skip)]
    pub role: Option<String>,
    /// The model, one `models::options` lists for the agent; its default without one.
    #[serde(default)]
    pub model: Option<String>,
    /// An effort the chosen model takes (`models::options`); the agent's default without one.
    #[serde(default)]
    pub effort: Option<String>,
    /// Set by `orchestrate`: the effort of every agent the orchestrator launches.
    #[serde(skip)]
    pub worker_effort: Option<String>,
}

/// Claude Code's effort levels, lowest first (`claude --effort`).
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

fn check_effort(effort: Option<&str>) -> Result<(), String> {
    match effort {
        Some(e) if !EFFORTS.contains(&e) => Err(format!("Effort inconnu : {e} (au choix : {})", EFFORTS.join(", "))),
        _ => Ok(()),
    }
}

/// A new session's model and effort, checked against what the agent's CLI reports for them. For a Claude agent of an
/// orchestrator, the effort the user set for all of them wins.
fn choice(
    agent: &str,
    model: Option<&str>,
    asked: Option<&str>,
    lead: Option<&Session>,
) -> Result<(Option<String>, Option<String>), String> {
    let model = model.map(str::trim).filter(|m| !m.is_empty());
    let asked = asked.map(str::trim).filter(|e| !e.is_empty());
    let fixed = lead.and_then(|p| p.worker_effort.as_deref()).filter(|_| agent == "claude");
    let Some(effort) = fixed.or(asked) else {
        if let Some(m) = model {
            models::options(agent)?.models.iter().find(|x| x.id == m).ok_or(format!("Modèle inconnu : {m}"))?;
        }
        return Ok((model.map(str::to_owned), None));
    };
    let options = models::options(agent)?;
    let efforts = match model {
        Some(m) => options.models.iter().find(|x| x.id == m).ok_or(format!("Modèle inconnu : {m}"))?.efforts.clone(),
        None => options.efforts,
    };
    if efforts.is_empty() {
        return Err(format!("{} n'a pas d'effort à régler", model.unwrap_or(agent)));
    }
    if !efforts.iter().any(|e| e == effort) {
        return Err(format!("Effort inconnu : {effort} (au choix : {})", efforts.join(", ")));
    }
    Ok((model.map(str::to_owned), Some(effort.to_owned())))
}

/// What the "Configurer avec Claude" session is told; the procedure itself is `studio guide setup`.
pub const SETUP_PROMPT: &str = "Configure ce projet dans Studio. Lance d'abord `studio guide setup` et suis-le pas à \
pas : inventaire du dépôt, questions groupées, aperçu avec `studio import --dry-run`, et import seulement après mon \
accord explicite.";

/// Told to every agent an orchestrator launches, after its item: nobody answers its questions, the orchestrator
/// merges its branch.
const WORKER_RULES: &str = "---\nTu travailles sans surveillance, pour l'orchestrateur de Studio : personne ne \
répondra à tes questions, et c'est lui qui relira et fusionnera ton travail.\n\
- Reste dans le périmètre de la consigne ; ce qui en sort devient `studio idea \"…\"` ou `studio bug \"…\"`.\n\
- Devant un choix, tranche raisonnablement et consigne-le : `studio note {id} \"Décision : …\"`.\n\
- Vérifie ton travail (tests, build) avant de rendre la main.\n\
- Commite tout sur ta branche. Ne pousse pas, ne fusionne pas, ne touche à aucune autre branche.\n\
- Pour finir : `studio note {id} \"…\"` résume ce qui est fait et comment le vérifier, puis `studio status {id} review`.\n\
- Bloqué pour de bon : explique pourquoi dans une note et arrête-toi sans passer en review.\n";

/// Told to a free session that grooms the roadmap: organize with `studio`, without writing code.
const GROOM_RULES: &str = "---\nTu tries et organises la roadmap de {name} : clarifier, regrouper, prioriser — sans \
écrire de code.\n\
- Pars du tour d'horizon : `studio items --all`, `studio milestones`, `studio sessions`, et la vision dans `studio \
project`.\n\
- L'inbox d'abord : clarifie titre et description de chaque élément à trier, puis `studio set <id>` (kind, priorité, \
jalon, labels) et `studio status <id> todo`, `backlog` ou `dropped`.\n\
- Découpe ce qui est trop gros pour un seul agent, fusionne les doublons, rédige les fiches vagues ; chaque décision \
devient `studio note <id> \"Décision : …\"`.\n\
- Ce qui surgit en route devient `studio idea \"…\"` ou `studio bug \"…\"`, sans t'y attarder.\n\
- Un choix qui revient à l'utilisateur (périmètre, design, arbitrage) : pose-le dans une note et laisse l'élément à \
trier plutôt que de trancher.\n\
- Termine par un bilan : ce qui a changé, ce qui l'attend.\n";

/// Told to a free session that brainstorms: propose and confront, without writing code or reshaping the roadmap.
const BRAINSTORM_RULES: &str = "---\nTu brainstormes sur {name} : proposer, confronter, creuser — sans écrire de code, \
sans réorganiser la roadmap.\n\
- Pars de la vision (`studio project`) et de ce qui existe (`studio items --all`, `studio milestones`).\n\
- Creuse peu de pistes plutôt que d'en effleurer beaucoup : ce qu'elles changent au projet, ce qu'elles coûtent, où elles \
risquent de casser.\n\
- Une piste qui tient devient `studio idea \"…\"` avec le raisonnement en corps de fiche ; les autres disparaissent.\n\
- Les arbitrages reviennent à l'utilisateur : pose-les franchement, propose, ne décide pas à sa place.\n\
- Termine par un bilan : les pistes proposées, celle que tu recommandes et pourquoi.\n";

/// Told to a review agent: look at a branch's work, report, change nothing.
const REVIEW_RULES: &str = "---\nTu relis ce travail sans le modifier : tu ne peux ni éditer de fichier ni écrire dans \
git, et tu n'en as pas besoin.\n\
- Lis le diff : `git diff {base}...HEAD`, plus ce qui n'est pas commité (`git status`, `git diff`). Lis autour ce qu'il \
faut pour juger, et les règles du dépôt (CLAUDE.md, AGENTS.md…).\n\
- Cherche ce qui est faux : bugs, cas oubliés, régressions, ce qui contredit la consigne ou les règles du dépôt, \
tests manquants là où ils comptent. Lance les tests et le build s'ils sont rapides.\n\
- Ne signale que ce que tu as vérifié. Pour chaque point : le fichier et la ligne, ce qui se passe, pourquoi c'est \
faux, la correction que tu proposes. Les détails de style seulement s'ils gênent vraiment.\n\
- Ton dernier message est ton rapport, transmis tel quel à l'agent qui a fait le travail : les points du plus grave \
au moins grave, puis ton verdict (prêt à fusionner, ou pas encore et pourquoi). Rien d'autre après.\n";

/// Told to a QA agent: run the work and check it behaves, change nothing.
const QA_RULES: &str = "---\nTu testes ce travail sans le modifier : tu ne peux ni éditer de fichier ni écrire \
dans git, et tu n'en as pas besoin.\n\
- Comprends ce qui doit marcher : la consigne, le diff (`git diff {base}...HEAD`), les règles du dépôt (CLAUDE.md, \
AGENTS.md…) et ses critères d'acceptation s'il en a.\n\
- Écris le guide de test : les étapes, et ce qu'on doit voir à chacune.\n\
- Exécute tout ce qui s'automatise : tests, build, lancement du projet{run}, requêtes, captures d'écran. Ce que tu \
crées pour tester (données, fichiers) va dans un dossier temporaire, jamais dans le dépôt.\n\
- Ton dernier message est ton rapport, transmis tel quel à l'agent qui a fait le travail : ce qui marche, ce qui \
casse (étapes pour le reproduire, ce que tu attendais, ce que tu as vu), puis ce qui reste à vérifier à la main. \
Rien d'autre après.\n";

/// What a review or QA agent may not do: they look at the work, they never change it.
const READ_ONLY_DENIED: [&str; 19] = [
    "Edit", "Write", "NotebookEdit", "Bash(git add:*)", "Bash(git commit:*)", "Bash(git push:*)", "Bash(git merge:*)",
    "Bash(git rebase:*)", "Bash(git reset:*)", "Bash(git checkout:*)", "Bash(git switch:*)", "Bash(git restore:*)",
    "Bash(git stash:*)", "Bash(git cherry-pick:*)", "Bash(gh pr create:*)", "Bash(gh pr merge:*)", "Bash(gh pr edit:*)",
    "Bash(gh pr review:*)", "Bash(gh pr comment:*)",
];

fn inspects(role: Option<&str>) -> bool {
    matches!(role, Some("review" | "qa"))
}

fn brief(
    project: &store::Project,
    item: Option<&store::Item>,
    prompt: &str,
    worker: bool,
    preset: Option<&str>,
) -> Option<String> {
    let mut text = String::new();
    if let Some(item) = item {
        let kind = match item.meta.kind.as_str() {
            "bug" => "un bug",
            "idea" => "une idée à creuser",
            _ => "une tâche",
        };
        text += &format!("Tu travailles sur {} ({kind} de {}) : {}\n\n", item.id, project.meta.name, item.meta.title);
        if !item.body.trim().is_empty() {
            text += &format!("{}\n\n", item.body.trim());
        }
    }
    if !prompt.trim().is_empty() {
        text += &format!("{}\n\n", prompt.trim());
    }
    match item {
        Some(item) if worker => text += &WORKER_RULES.replace("{id}", &item.id),
        Some(item) => text += &format!(
            "---\nStudio suit ce travail. Avec la commande `studio` :\n\
             - `studio item {id}` relit la fiche ;\n\
             - `studio note {id} \"…\"` consigne une décision ou un point d'étape ;\n\
             - `studio status {id} review` quand c'est prêt à vérifier (`done` une fois vérifié) ;\n\
             - `studio idea \"…\"` note une idée hors sujet sans t'y attarder.\n",
            id = item.id
        ),
        None if preset == Some("groom") => text += &GROOM_RULES.replace("{name}", &project.meta.name),
        None if preset == Some("brainstorm") => text += &BRAINSTORM_RULES.replace("{name}", &project.meta.name),
        None => {}
    }
    (!text.trim().is_empty()).then_some(text)
}

fn unique_id(base: &str) -> String {
    let taken: Vec<String> =
        RECORDS.lock().unwrap().iter().map(|s| s.id.clone()).chain(tmux::sessions()).collect();
    let base = if base.is_empty() { "session".to_string() } else { base.to_string() };
    let mut id = base.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Unattended sessions run in Claude's auto mode: its classifier, not a person, clears each action. The orchestrator
/// thinks with the strongest model.
fn claude_flags(role: Option<&str>, model: Option<&str>, effort: Option<&str>) -> String {
    let mut flags = String::new();
    match (role, model) {
        (Some("orchestrator"), _) => flags += " --model opus",
        (_, Some(m)) => flags += &format!(" --model {}", paths::shell_quote(m)),
        _ => {}
    }
    if let Some(e) = effort {
        flags += &format!(" --effort {e}");
    }
    if role.is_some() {
        flags += " --permission-mode auto";
    }
    if inspects(role) {
        let denied: Vec<String> = READ_ONLY_DENIED.iter().map(|t| paths::shell_quote(t)).collect();
        flags += &format!(" --disallowedTools {}", denied.join(" "));
    }
    flags
}

/// Codex's model and effort, given before its subcommand.
fn codex_flags(model: Option<&str>, effort: Option<&str>) -> String {
    let mut flags = String::new();
    if let Some(m) = model {
        flags += &format!(" -m {}", paths::shell_quote(m));
    }
    if let Some(e) = effort {
        flags += &format!(" -c {}", paths::shell_quote(&format!("model_reasoning_effort=\"{e}\"")));
    }
    flags
}

/// The shell command that runs the agent, then leaves a login shell behind when it exits.
fn agent_command(
    agent: &str,
    role: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    brief_file: Option<&Path>,
    resume: Option<&Session>,
) -> Result<Option<String>, String> {
    let exe = studio_link();
    let brief_arg = brief_file.map(|f| format!(" \"$(cat {})\"", paths::shell_quote(&f.display().to_string()))).unwrap_or_default();
    let run = match agent {
        "claude" => {
            let settings = format!("--settings {}{}", paths::shell_quote(&hooks_file().display().to_string()), claude_flags(role, model, effort));
            match resume {
                Some(Session { claude_session: Some(id), .. }) => format!("claude {settings} --resume {}", paths::shell_quote(id)),
                Some(_) => format!("claude {settings} --continue"),
                None => format!("claude {settings}{brief_arg}"),
            }
        }
        "codex" => {
            let notify = format!("-c {}{}", paths::shell_quote(&format!("notify=[\"{exe}\",\"hook\",\"codex\"]")), codex_flags(model, effort));
            match resume {
                Some(_) => format!("codex {notify} resume --last"),
                None => format!("codex {notify}{brief_arg}"),
            }
        }
        "opencode" => {
            let mut flags = String::new();
            // OpenCode takes the effort as the variant of its model: `provider/model#effort`.
            if let Some(m) = model {
                let variant = effort.map(|e| format!("#{e}")).unwrap_or_default();
                flags += &format!(" --model {}", paths::shell_quote(&format!("{m}{variant}")));
            }
            if role.is_some() {
                flags += " --auto";
            }
            // Its own config directory, which holds only Studio's status plugin: the user's OpenCode setup stays put.
            let config = format!("OPENCODE_CONFIG_DIR={}", paths::shell_quote(&opencode_config_dir().display().to_string()));
            // `--standalone`: the plugin runs in the server this session starts, which is the one that sees its environment.
            match resume {
                Some(_) => format!("{config} opencode --standalone --continue{flags}"),
                None => {
                    let prompt = brief_file
                        .map(|f| format!(" --prompt \"$(cat {})\"", paths::shell_quote(&f.display().to_string())))
                        .unwrap_or_default();
                    format!("{config} opencode --standalone{flags}{prompt}")
                }
            }
        }
        "shell" => return Ok(None),
        other => return Err(format!("Agent inconnu : {other}")),
    };
    Ok(Some(format!("{run}; exec \"$SHELL\" -l")))
}

/// Starts the session in tmux or, when it is live, replaces what runs in it.
fn spawn(session: &Session, command: Option<&str>) -> Result<(), String> {
    let item = session.item.clone().unwrap_or_default();
    let path = format!("{}:{}", bin_dir().display(), std::env::var("PATH").unwrap_or_default());
    let env = [
        ("PATH", path.as_str()),
        ("STUDIO_SESSION", session.id.as_str()),
        ("STUDIO_PROJECT", session.project.as_str()),
        ("STUDIO_ITEM", item.as_str()),
        ("STUDIO_AGENT", session.agent.as_str()),
        // xterm.js reports about a third of the trackpad's wheel motion. Claude Code scrolls 3 lines per report when
        // it recognizes xterm.js, which it cannot behind tmux: tell it.
        ("CLAUDE_CODE_SCROLL_SPEED", "3"),
    ];
    match command {
        Some(c) if tmux::has_session(&session.id) => tmux::respawn(&session.id, Path::new(&session.cwd), &env, c),
        _ => tmux::new_session(&session.id, Path::new(&session.cwd), &env, command),
    }
}

pub fn start(app: &AppHandle, req: StartRequest) -> Result<Session, String> {
    let project = store::project(&req.project)?;
    let item = req.item.as_deref().filter(|i| !i.is_empty()).map(store::item).transpose()?;
    let parent = req.parent.as_deref().and_then(|id| find(id).ok());
    // An agent launched by an orchestrator works for it: unattended, on one item, in a worktree branched from the
    // orchestrator's integration branch, so it builds on what is already merged there.
    let lead = parent.as_ref().filter(|p| p.role.as_deref() == Some("orchestrator"));
    if lead.is_some() && item.is_none() {
        return Err("Un agent de l'orchestrateur travaille sur un élément : précise son id".into());
    }
    if lead.is_some() && req.place != "worktree" {
        return Err("Un agent de l'orchestrateur travaille dans son propre worktree : précise --branch".into());
    }
    // Several orchestrators can run on a project: the first to start an agent on an item gets it.
    let _claim = lead.map(|_| CLAIM.lock().unwrap_or_else(|e| e.into_inner()));
    if let (Some(_), Some(i)) = (lead, &item) {
        if let Some(s) = list().into_iter().find(|s| s.item.as_deref() == Some(i.id.as_str()) && s.status != "offline") {
            return Err(format!("{} est déjà pris par {} : choisis un autre élément", i.id, s.id));
        }
    }
    // A free session can take a mission — grooming the roadmap, brainstorming — instead of an item: it works in the
    // main checkout, auto mode, and touches the tracker, not the code.
    let preset = req.preset.as_deref().map(str::trim).filter(|p| !p.is_empty());
    match preset {
        None | Some("groom") | Some("brainstorm") => {}
        Some(other) => return Err(format!("Mission inconnue : {other} (au choix : groom, brainstorm)")),
    }
    if preset.is_some() && (item.is_some() || req.agent == "shell") {
        return Err("Une mission (groom, brainstorm) se donne à une session libre, sans élément".into());
    }
    if preset.is_some() && req.place == "worktree" {
        return Err("Une session de mission n'a pas besoin de worktree : elle travaille sur la roadmap, pas le code".into());
    }
    let role = req.role
        .clone()
        .or_else(|| lead.map(|_| "worker".to_string()))
        .or_else(|| preset.map(str::to_string));
    let (model, effort) = choice(&req.agent, req.model.as_deref(), req.effort.as_deref(), lead)?;
    // Fail before creating anything the command could not use.
    agent_command(&req.agent, role.as_deref(), model.as_deref(), effort.as_deref(), None, None)?;

    let (cwd, branch) = match req.place.as_str() {
        "main" => (worktrees::repo_path(&project)?, None),
        "path" => {
            let path = PathBuf::from(req.path.as_deref().ok_or("Il manque le chemin du worktree")?);
            if !path.is_dir() {
                return Err(format!("{} n'existe pas", path.display()));
            }
            (path, req.branch.clone())
        }
        "worktree" => {
            let branch = req.branch.as_deref().filter(|b| !b.trim().is_empty()).ok_or("Il manque le nom de la branche")?;
            let from = lead.and_then(|p| p.branch.as_deref());
            (worktrees::create(&project, branch, from)?, Some(branch.trim().to_string()))
        }
        other => return Err(format!("Emplacement inconnu : {other}")),
    };

    let cwd = paths::canonical(&cwd);
    let base = match (&item, &branch) {
        (Some(i), _) => format!("{}-{}", i.id.to_lowercase(), req.agent),
        (None, Some(b)) => store::slugify(&b.replace('/', "-"), 40),
        (None, None) => format!("{}-{}", project.id, req.agent),
    };
    let id = unique_id(&base);
    let title = match (&item, &branch) {
        _ if req.title.is_some() => req.title.clone().unwrap_or_default(),
        (Some(i), _) => i.meta.title.clone(),
        (None, Some(b)) => b.clone(),
        (None, None) if preset == Some("groom") => "Trier et organiser".into(),
        (None, None) if preset == Some("brainstorm") => "Brainstorm".into(),
        (None, None) => if req.agent == "shell" { "Terminal".into() } else { format!("{} libre", req.agent) },
    };

    let brief_file = match brief(&project, item.as_ref(), &req.prompt, role.as_deref() == Some("worker"), preset) {
        Some(text) if req.agent != "shell" => {
            let f = paths::studio_dir().join("briefs").join(format!("{id}.md"));
            paths::write_atomic(&f, &text)?;
            Some(f)
        }
        _ => None,
    };
    let mut command = agent_command(&req.agent, role.as_deref(), model.as_deref(), effort.as_deref(), brief_file.as_deref(), None)?;
    let fresh_worktree = req.place == "worktree" && !project.meta.setup.trim().is_empty();
    if let (true, Some(c)) = (fresh_worktree, &command) {
        command = Some(format!("{} && {c}", project.meta.setup.trim()));
    }

    let now = paths::now_iso();
    let session = Session {
        id,
        project: project.id.clone(),
        item: item.as_ref().map(|i| i.id.clone()),
        title,
        agent: req.agent.clone(),
        cwd: cwd.display().to_string(),
        branch,
        created: now.clone(),
        claude_session: None,
        // An agent with a brief starts on it; otherwise it waits at its prompt.
        status: if brief_file.is_some() { "working" } else { "idle" }.into(),
        status_at: now,
        message: None,
        parent: parent.map(|p| p.id),
        role,
        model,
        effort,
        worker_effort: req.worker_effort.clone(),
    };
    spawn(&session, command.as_deref())?;
    {
        let mut records = RECORDS.lock().unwrap();
        records.push(session.clone());
        save(&records);
    }
    // Starting an agent on an item means it is being worked on.
    if let Some(i) = &item {
        if req.agent != "shell" && matches!(i.meta.status.as_str(), "inbox" | "backlog" | "todo") {
            let patch = store::ItemPatch { status: Some("doing".into()), ..Default::default() };
            if store::update_item(&i.id, patch).is_ok() {
                let _ = app.emit("data-changed", ());
                crate::sync::touch();
            }
        }
    }
    changed(app);
    Ok(session)
}

/// Brings an offline session back in its directory: Claude resumes its conversation, a shell starts fresh.
pub fn resume(app: &AppHandle, id: &str) -> Result<Session, String> {
    let session = find(id)?;
    if session.status != "offline" {
        return Err(format!("{id} tourne déjà"));
    }
    if session.agent == "game" {
        return play(app, &session.project, Some(&session.cwd));
    }
    if !Path::new(&session.cwd).is_dir() {
        return Err(format!("{} n'existe plus", session.cwd));
    }
    let command = agent_command(&session.agent, session.role.as_deref(), session.model.as_deref(), session.effort.as_deref(), None, Some(&session))?;
    spawn(&session, command.as_deref())?;
    // A resumed agent waits at its prompt for what to do next.
    update(id, |s| {
        s.status = "idle".into();
        s.status_at = paths::now_iso();
        s.message = None;
    });
    changed(app);
    find(id).map_err(|_| "Session perdue".into())
}

/// Runs the game from the main checkout or a worktree (`path`), in a session of its own: its output shows in a
/// terminal, and what is left once it quits is a shell. Playing again where it runs restarts it on the current code,
/// in the same terminal.
pub fn play(app: &AppHandle, project: &str, path: Option<&str>) -> Result<Session, String> {
    let project = store::project(project)?;
    let cwd = match path {
        Some(p) => PathBuf::from(p),
        None => worktrees::repo_path(&project)?,
    };
    if !cwd.is_dir() {
        return Err(format!("{} n'existe pas", cwd.display()));
    }
    let cwd = paths::canonical(&cwd);
    let command = format!("{}; exec \"$SHELL\" -l", game::command(&project, &cwd)?);
    let now = paths::now_iso();
    let known = list().into_iter().find(|s| s.agent == "game" && s.project == project.id && Path::new(&s.cwd) == cwd);
    let session = known.unwrap_or_else(|| {
        let branch = paths::run(paths::git(&cwd).args(["branch", "--show-current"]))
            .map(|b| b.trim().to_string())
            .ok()
            .filter(|b| !b.is_empty());
        Session {
            id: unique_id(&format!("{}-run", project.id)),
            project: project.id.clone(),
            item: None,
            title: branch.as_ref().map_or("Lancement".into(), |b| format!("Lancement sur {b}")),
            agent: "game".into(),
            cwd: cwd.display().to_string(),
            branch,
            created: now.clone(),
            claude_session: None,
            status: "idle".into(),
            status_at: now.clone(),
            message: None,
            parent: None,
            role: None,
            model: None,
            effort: None,
            worker_effort: None,
        }
    });
    spawn(&session, Some(&command))?;
    {
        let mut records = RECORDS.lock().unwrap();
        match records.iter_mut().find(|s| s.id == session.id) {
            Some(s) => s.status_at = now,
            None => records.push(session.clone()),
        }
        save(&records);
    }
    changed(app);
    find(&session.id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrateRequest {
    pub project: String,
    pub parallel: u32,
    #[serde(default)]
    pub prompt: String,
    /// The orchestrator's own effort; `xhigh` without one.
    #[serde(default)]
    pub effort: Option<String>,
    /// The effort of every agent it launches; without one, it picks each agent's.
    #[serde(default)]
    pub worker_effort: Option<String>,
}

/// What the orchestrator is told; the procedure itself is `studio guide orchestrate`.
fn orchestrate_prompt(project: &store::Project, branch: &str, parallel: u32, worker_effort: Option<&str>, instructions: &str) -> String {
    let mut text = format!(
        "Tu orchestres le travail sur {} en autonomie. Lance d'abord `studio guide orchestrate` et suis-le jusqu'au \
         bout. Ta branche d'intégration est {branch} ; au plus {parallel} agents en parallèle.",
        project.meta.name
    );
    text += &match worker_effort {
        Some(e) => format!(" Tes agents tournent en effort {e}, réglé par l'utilisateur : `--effort` n'y change rien."),
        None => " Tu choisis l'effort de chaque agent (`studio start --effort`), selon son élément.".into(),
    };
    if !instructions.trim().is_empty() {
        text += &format!("\n\nConsigne de l'utilisateur, qui prime sur le guide :\n{}", instructions.trim());
    }
    text
}

/// Starts a project's orchestrator: Claude in a worktree of its own, on a new integration branch, where it merges
/// the work of the agents it launches. Several can run on a project: each has its own branch, and an item goes to the
/// first one to start an agent on it (`start`).
pub fn orchestrate(app: &AppHandle, req: OrchestrateRequest) -> Result<Session, String> {
    let parallel = req.parallel;
    if !(1..=8).contains(&parallel) {
        return Err(format!("Entre 1 et 8 agents en parallèle, pas {parallel}"));
    }
    let worker_effort = req.worker_effort.filter(|e| !e.trim().is_empty());
    check_effort(worker_effort.as_deref())?;
    let project = store::project(&req.project)?;
    let branch = worktrees::free_branch(&project, &format!("orchestration-{}", paths::local_stamp().replace(' ', "-").replace(':', "")))?;
    let req = StartRequest {
        project: project.id.clone(),
        item: None,
        agent: "claude".into(),
        place: "worktree".into(),
        prompt: orchestrate_prompt(&project, &branch, parallel, worker_effort.as_deref(), &req.prompt),
        branch: Some(branch),
        path: None,
        preset: None,
        title: Some("Orchestration".into()),
        parent: None,
        role: Some("orchestrator".into()),
        model: None,
        effort: Some(req.effort.unwrap_or_else(|| "xhigh".into())),
        worker_effort,
    };
    start(app, req)
}

/// Types `text` into a session and submits it, as if typed in its terminal: how an orchestrator answers its agents.
/// A permission an agent waits on is the user's to grant, never another agent's.
pub fn send(app: &AppHandle, id: &str, text: &str, caller: Option<&str>) -> Result<(), String> {
    let session = driven(id, caller)?;
    match session.status.as_str() {
        "offline" => return Err(format!("{id} est arrêtée")),
        "waiting" if caller.is_some() => return Err(format!("{id} attend l'utilisateur : c'est à lui de répondre")),
        _ => {}
    }
    tmux::paste(id, text)?;
    if matches!(session.agent.as_str(), "claude" | "codex" | "opencode") {
        // Working from now on, so a `studio wait` right after does not see it idle before its hook fires.
        update(id, |s| {
            s.status = "working".into();
            s.status_at = paths::now_iso();
            s.message = None;
        });
        changed(app);
    }
    Ok(())
}

/// Names what a session works on, as an orchestrator does once it has picked its work, and again as it moves on.
pub fn retitle(app: &AppHandle, id: &str, title: &str) -> Result<(), String> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return Err("Le titre est vide".into());
    }
    if !update(id, |s| s.title = title) {
        return Err(format!("Session inconnue : {id}"));
    }
    changed(app);
    Ok(())
}

/// The end of a session's screen: what an agent last said, or what it asks.
pub fn peek(id: &str, lines: usize) -> Result<String, String> {
    if find(id)?.status == "offline" {
        return Err(format!("{id} est arrêtée"));
    }
    tmux::capture(id, lines.clamp(1, 2000))
}

/// Ends a session: its processes get SIGHUP and the record goes.
pub fn stop(app: &AppHandle, id: &str) -> Result<(), String> {
    if tmux::has_session(id) {
        tmux::kill_session(id)?;
    }
    let mut records = RECORDS.lock().unwrap();
    records.retain(|s| s.id != id);
    save(&records);
    drop(records);
    let _ = std::fs::remove_file(report_file(id));
    changed(app);
    Ok(())
}

fn update(id: &str, f: impl FnOnce(&mut Session)) -> bool {
    let mut records = RECORDS.lock().unwrap();
    let Some(s) = records.iter_mut().find(|s| s.id == id) else {
        return false;
    };
    f(s);
    save(&records);
    true
}

/// An agent hook fired inside session `id`.
pub fn hook(app: &AppHandle, id: &str, event: &str, payload: &Value) {
    let text = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_owned);
    let now = paths::now_iso();
    let status = match event {
        "prompt" | "tool" => "working",
        "notification" => "waiting",
        "stop" | "codex" => "idle",
        "opencode" => match text("status").as_deref() {
            Some("working") => "working",
            Some("waiting") => "waiting",
            _ => "idle",
        },
        // Startup, resume, /clear and compaction alike: only the conversation id is news. A compaction in the middle
        // of a turn is no reason to look idle.
        "start" => {
            if let Some(sid) = text("session_id") {
                update(id, |s| s.claude_session = Some(sid));
            }
            return;
        }
        _ => return,
    };
    let mut moved = false;
    let mut now_waiting = None;
    let mut report = None;
    update(id, |s| {
        // A review or QA agent's last word is its report, kept whole for the agent whose work it looked at.
        if inspects(s.role.as_deref()) && event == "stop" {
            report = text("last_assistant_message").filter(|m| !m.trim().is_empty());
        }
        // A worker idle at its prompt waits on its orchestrator, not on the user, and a review or QA agent has
        // handed in its report: only a permission turns them amber.
        let unattended = matches!(s.role.as_deref(), Some("worker" | "review" | "qa"));
        if unattended && text("notification_type").as_deref() == Some("idle_prompt") {
            return;
        }
        // A tool call while waiting means the permission was granted; otherwise only real transitions count.
        if s.status != status {
            moved = true;
            s.status = status.into();
            s.status_at = now;
            // At the end of a turn, what the agent said last: its report, or the question it stopped on.
            s.message = match event {
                "notification" => text("message"),
                "stop" => text("last_assistant_message").map(|m| m.chars().take(400).collect()),
                "codex" => text("last-assistant-message").map(|m| m.chars().take(400).collect()),
                "opencode" => text("message").map(|m| m.chars().take(400).collect()),
                _ => None,
            };
            if status == "waiting" {
                now_waiting = Some(s.clone());
            }
        }
    });
    if moved {
        changed(app);
    }
    if let Some(text) = report {
        if let Err(e) = paths::write_atomic(&report_file(id), &text) {
            eprintln!("{e}");
        }
    }
    if let Some(session) = now_waiting {
        notify_waiting(app, &session);
    }
}

fn report_file(id: &str) -> PathBuf {
    paths::studio_dir().join("reports").join(format!("{id}.md"))
}

/// A review or QA agent's last report.
pub fn report(id: &str) -> Result<String, String> {
    let session = find(id)?;
    if !inspects(session.role.as_deref()) {
        return Err(format!("{id} n'est pas un agent de revue ou de QA"));
    }
    std::fs::read_to_string(report_file(id)).map_err(|_| format!("{id} n'a pas encore rendu de rapport"))
}

/// Starts a Claude agent that reviews (`review`) or tests (`qa`) the work of session `target`, in its directory,
/// without the right to change anything; it replaces the last one of that kind. The person, the agent itself
/// (`caller` = `target`) or the agent that launched it may ask.
pub fn inspect(app: &AppHandle, target: &str, kind: &str, context: &str, effort: Option<&str>, caller: Option<&str>) -> Result<Session, String> {
    if caller != Some(target) {
        driven(target, caller)?;
    }
    let worker = find(target)?;
    if !matches!(worker.agent.as_str(), "claude" | "codex" | "opencode") || inspects(worker.role.as_deref()) {
        return Err(format!("{target} n'est pas un agent dont on relit le travail"));
    }
    let (label, rules) = match kind {
        "review" => ("Revue", REVIEW_RULES),
        "qa" => ("QA", QA_RULES),
        other => return Err(format!("Inconnu : {other} (au choix : review, qa)")),
    };
    let project = store::project(&worker.project)?;
    // An orchestrator's agent builds on the orchestrator's branch: its own work is what it adds there.
    let base = match worker.parent.as_deref().map(find) {
        Some(Ok(p)) if p.role.as_deref() == Some("orchestrator") => p.branch.unwrap_or_else(|| project.meta.base.clone()),
        _ => project.meta.base.clone(),
    };
    for old in list().into_iter().filter(|s| s.parent.as_deref() == Some(target) && s.role.as_deref() == Some(kind)) {
        stop(app, &old.id)?;
    }

    let what = match &worker.item {
        Some(i) => format!("{i} : {}", worker.title),
        None => worker.title.clone(),
    };
    let mut prompt = format!(
        "Tu fais la {} du travail de l'agent {} ({what}) sur {}, dans ce dossier. La base est {base}.\n\n",
        if kind == "review" { "revue" } else { "QA" },
        worker.id,
        worker.branch.as_deref().map_or("le dépôt principal".to_string(), |b| format!("la branche {b}")),
    );
    if let Some(item) = worker.item.as_deref().and_then(|i| store::item(i).ok()) {
        if !item.body.trim().is_empty() {
            prompt += &format!("La fiche {} :\n\n{}\n\n", item.id, item.body.trim());
        }
    }
    if !context.trim().is_empty() {
        prompt += &format!("{}\n\n", context.trim());
    }
    let run = match project.meta.run.trim() {
        "" => String::new(),
        cmd => format!(" (`{cmd}`)"),
    };
    prompt += &rules.replace("{base}", &base).replace("{run}", &run);

    let req = StartRequest {
        project: worker.project.clone(),
        item: None,
        agent: "claude".into(),
        place: "path".into(),
        branch: worker.branch.clone(),
        path: Some(worker.cwd.clone()),
        prompt,
        preset: None,
        title: Some(format!("{label} de {what}")),
        parent: None,
        role: Some(kind.into()),
        model: None,
        effort: effort.map(str::to_owned),
        worker_effort: None,
    };
    let session = start(app, req)?;
    update(&session.id, |s| s.parent = Some(target.to_string()));
    find(&session.id)
}

/// Hands a review or QA report to the agent whose work it looked at, as a message in its terminal.
pub fn hand_over(app: &AppHandle, id: &str) -> Result<String, String> {
    report(id)?;
    let session = find(id)?;
    let worker = session.parent.ok_or_else(|| format!("{id} ne relit le travail de personne"))?;
    let what = if session.role.as_deref() == Some("qa") { "Le rapport de QA" } else { "Le rapport de revue" };
    let text = format!(
        "{what} de {id} est dans {}. Lis-le, corrige ce qui doit l'être, et dis ce que tu laisses de côté et pourquoi.",
        report_file(id).display()
    );
    send(app, &worker, &text, None)?;
    Ok(worker)
}

/// A notification with a sound when an agent starts waiting on the user while the app is in the background; in front,
/// the app shows it itself.
fn notify_waiting(app: &AppHandle, session: &Session) {
    use tauri_plugin_notification::NotificationExt;
    if app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false) {
        return;
    }
    let project = store::project(&session.project).map(|p| p.meta.name).unwrap_or_else(|_| session.project.clone());
    let body = match &session.message {
        Some(m) => format!("{}\n{m}", session.title),
        None => session.title.clone(),
    };
    let _ = app
        .notification()
        .builder()
        .title(format!("{} t'attend sur {project}", agent_name(&session.agent)))
        .body(body)
        .sound("default")
        .show();
}

fn agent_name(agent: &str) -> &str {
    match agent {
        "claude" => "Claude",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        _ => "Un agent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> (store::Project, store::Item) {
        let meta = store::ProjectMeta { name: "Frostholm".into(), key: "FRO".into(), ..Default::default() };
        let project = store::Project { id: "frostholm".into(), meta, vision: String::new(), present: true };
        let item = store::Item {
            id: "FRO-1".into(),
            project: "frostholm".into(),
            meta: store::ItemMeta {
                title: "Armures".into(),
                kind: "task".into(),
                status: "todo".into(),
                priority: "normal".into(),
                milestone: None,
                labels: Vec::new(),
                created: String::new(),
                updated: String::new(),
                closed: None,
            },
            body: "Fait quand le torse se porte.".into(),
        };
        (project, item)
    }

    #[test]
    fn workers_get_the_unattended_rules() {
        let (project, item) = fixtures();
        let worker = brief(&project, Some(&item), "Seulement le torse.", true, None).unwrap();
        assert!(worker.contains("Fait quand le torse se porte.\n\nSeulement le torse."));
        assert!(worker.contains("`studio status FRO-1 review`"));
        assert!(worker.contains("Ne pousse pas"));
        let attended = brief(&project, Some(&item), "", false, None).unwrap();
        assert!(attended.contains("`done` une fois vérifié"));
        assert!(!attended.contains("sans surveillance"));
        assert!(brief(&project, None, " ", false, None).is_none());
    }

    #[test]
    fn free_sessions_get_a_mission() {
        let (project, _) = fixtures();
        let groom = brief(&project, None, "", false, Some("groom")).unwrap();
        assert!(groom.contains("roadmap de Frostholm"));
        assert!(groom.contains("`studio items --all`"));
        assert!(groom.contains("sans écrire de code"));
        let brainstorm = brief(&project, None, "Le combat.", false, Some("brainstorm")).unwrap();
        assert!(brainstorm.contains("Le combat.\n\n---\n"));
        assert!(brainstorm.contains("`studio idea"));
        assert!(brainstorm.contains("ne décide pas à sa place"));
    }

    #[test]
    fn review_and_qa_agents_cannot_change_the_work() {
        let flags = claude_flags(Some("review"), None, None);
        assert!(flags.starts_with(" --permission-mode auto --disallowedTools 'Edit' 'Write' 'NotebookEdit'"));
        assert!(flags.contains("'Bash(git commit:*)'") && flags.contains("'Bash(gh pr merge:*)'"));
        assert!(claude_flags(Some("qa"), None, None).contains("--disallowedTools"));
        assert!(!claude_flags(Some("worker"), None, None).contains("--disallowedTools"));
        assert!(REVIEW_RULES.contains("{base}") && QA_RULES.contains("{run}"));
    }

    #[test]
    fn unattended_sessions_run_in_auto_mode() {
        assert_eq!(claude_flags(None, None, None), "");
        assert_eq!(claude_flags(None, None, Some("low")), " --effort low");
        assert_eq!(claude_flags(None, Some("sonnet"), Some("low")), " --model 'sonnet' --effort low");
        assert_eq!(claude_flags(Some("worker"), None, None), " --permission-mode auto");
        assert_eq!(claude_flags(Some("groom"), None, None), " --permission-mode auto");
        assert_eq!(claude_flags(Some("orchestrator"), None, Some("xhigh")), " --model opus --effort xhigh --permission-mode auto");
        assert_eq!(claude_flags(Some("orchestrator"), Some("sonnet"), None), " --model opus --permission-mode auto");
        let (project, _) = fixtures();
        let prompt = orchestrate_prompt(&project, "orchestration-2026-10-29-1432", 3, None, "Seulement les bugs.");
        assert!(prompt.contains("`studio guide orchestrate`"));
        assert!(prompt.contains("orchestration-2026-10-29-1432 ; au plus 3 agents"));
        assert!(prompt.contains("Tu choisis l'effort de chaque agent"));
        assert!(prompt.ends_with("Seulement les bugs."));
        let fixed = orchestrate_prompt(&project, "orchestration-2026-10-29-1432", 3, Some("high"), "");
        assert!(fixed.contains("Tes agents tournent en effort high"));
    }

    #[test]
    fn the_user_sets_the_effort_of_an_orchestration() {
        let lead = |worker_effort: Option<&str>| Session {
            id: "orchestration".into(),
            project: "frostholm".into(),
            item: None,
            title: "Orchestration".into(),
            agent: "claude".into(),
            cwd: "/tmp".into(),
            branch: None,
            created: String::new(),
            claude_session: None,
            status: "working".into(),
            status_at: String::new(),
            message: None,
            parent: None,
            role: Some("orchestrator".into()),
            model: None,
            effort: Some("xhigh".into()),
            worker_effort: worker_effort.map(str::to_owned),
        };
        let effort = |agent, asked, lead: Option<&Session>| choice(agent, None, asked, lead).map(|(_, e)| e);
        assert_eq!(effort("claude", None, None), Ok(None));
        assert_eq!(effort("claude", Some(" "), None), Ok(None));
        assert_eq!(effort("claude", Some("medium"), None), Ok(Some("medium".into())));
        assert!(effort("claude", Some("ultra"), None).is_err());
        // Left to the orchestrator, or set by the user for all its agents.
        assert_eq!(effort("claude", Some("low"), Some(&lead(None))), Ok(Some("low".into())));
        assert_eq!(effort("claude", Some("low"), Some(&lead(Some("high")))), Ok(Some("high".into())));
        assert_eq!(effort("claude", None, Some(&lead(Some("high")))), Ok(Some("high".into())));
        assert_eq!(effort("codex", None, Some(&lead(Some("high")))), Ok(None));
    }

    #[test]
    fn a_model_takes_its_own_efforts() {
        assert_eq!(choice("claude", Some("opus"), Some("max"), None), Ok((Some("opus".into()), Some("max".into()))));
        assert_eq!(choice("claude", Some("haiku"), None, None), Ok((Some("haiku".into()), None)));
        assert!(choice("claude", Some("gpt-9"), None, None).is_err());
        assert_eq!(choice("claude", None, None, None), Ok((None, None)));
        assert!(choice("shell", None, Some("high"), None).is_err());
    }

    #[test]
    fn opencode_takes_its_effort_as_the_model_variant() {
        let command = agent_command("opencode", Some("worker"), Some("opencode-go/minimax-m3.1-flash"), Some("high"), None, None)
            .unwrap()
            .unwrap();
        assert!(command.contains("opencode --standalone --model 'opencode-go/minimax-m3.1-flash#high' --auto"));
        assert!(command.starts_with("OPENCODE_CONFIG_DIR='"));
        let attended = agent_command("opencode", None, None, None, None, None).unwrap().unwrap();
        assert!(attended.contains("opencode --standalone;") || attended.contains("opencode --standalone "));
        assert!(!attended.contains("--auto") && !attended.contains("--model"));
    }

    #[test]
    fn codex_gets_its_model_and_effort_as_flags() {
        assert_eq!(codex_flags(None, None), "");
        assert_eq!(codex_flags(Some("gpt-6.1-sol"), Some("high")), " -m 'gpt-6.1-sol' -c 'model_reasoning_effort=\"high\"'");
    }
}
