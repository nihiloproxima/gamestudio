//! Agent and shell sessions, each a tmux session on Gamestudio's private server so it outlives the app. Records live
//! in `~/.gamestudio/sessions.json`, on this machine only; liveness is read from tmux, status comes from the agents'
//! hooks (`studio hook …`), never from screen scraping.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::{paths, store, tmux, worktrees};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Also the tmux session name.
    pub id: String,
    pub project: String,
    pub item: Option<String>,
    pub title: String,
    /// `claude`, `codex` or `shell`.
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
}

static RECORDS: Mutex<Vec<Session>> = Mutex::new(Vec::new());

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

/// Claude Code settings layered on the sessions Gamestudio launches (`claude --settings`), so their hooks report to
/// the app without touching the user's global settings. Rewritten at startup with the current binary.
pub fn write_hooks() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let hook = |event: &str| {
        serde_json::json!([{ "hooks": [{ "type": "command", "command": format!("'{}' hook {event}", exe.display()), "timeout": 5 }] }])
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

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub project: String,
    pub item: Option<String>,
    /// `claude`, `codex` or `shell`.
    pub agent: String,
    /// `main` (the repo's checkout), `worktree` (new, for `branch`) or `path` (an existing worktree).
    pub place: String,
    pub branch: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub prompt: String,
    /// Shown instead of the item's title or the branch.
    #[serde(default)]
    pub title: Option<String>,
    /// The session running `studio start`, set by the control socket.
    #[serde(skip)]
    pub parent: Option<String>,
    /// Set by `orchestrate`; a worker's role comes from its parent.
    #[serde(skip)]
    pub role: Option<String>,
}

/// What the "Configurer avec Claude" session is told; the procedure itself is `studio guide setup`.
pub const SETUP_PROMPT: &str = "Configure ce jeu dans Gamestudio. Lance d'abord `studio guide setup` et suis-le pas à \
pas : inventaire du dépôt, questions groupées, aperçu avec `studio import --dry-run`, et import seulement après mon \
accord explicite.";

/// Told to every agent an orchestrator launches, after its item: nobody answers its questions, the orchestrator
/// merges its branch.
const WORKER_RULES: &str = "---\nTu travailles sans surveillance, pour l'orchestrateur de Gamestudio : personne ne \
répondra à tes questions, et c'est lui qui relira et fusionnera ton travail.\n\
- Reste dans le périmètre de la consigne ; ce qui en sort devient `studio idea \"…\"` ou `studio bug \"…\"`.\n\
- Devant un choix, tranche raisonnablement et consigne-le : `studio note {id} \"Décision : …\"`.\n\
- Vérifie ton travail (tests, build) avant de rendre la main.\n\
- Commite tout sur ta branche. Ne pousse pas, ne fusionne pas, ne touche à aucune autre branche.\n\
- Pour finir : `studio note {id} \"…\"` résume ce qui est fait et comment le vérifier, puis `studio status {id} review`.\n\
- Bloqué pour de bon : explique pourquoi dans une note et arrête-toi sans passer en review.\n";

fn brief(project: &store::Project, item: Option<&store::Item>, prompt: &str, worker: bool) -> Option<String> {
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
            "---\nGamestudio suit ce travail. Avec la commande `studio` :\n\
             - `studio item {id}` relit la fiche ;\n\
             - `studio note {id} \"…\"` consigne une décision ou un point d'étape ;\n\
             - `studio status {id} review` quand c'est prêt à vérifier (`done` une fois vérifié) ;\n\
             - `studio idea \"…\"` note une idée hors sujet sans t'y attarder.\n",
            id = item.id
        ),
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
/// thinks with the strongest model, at a high effort.
fn claude_flags(role: Option<&str>) -> &'static str {
    match role {
        Some("orchestrator") => " --model opus --effort xhigh --permission-mode auto",
        Some(_) => " --permission-mode auto",
        None => "",
    }
}

/// The shell command that runs the agent, then leaves a login shell behind when it exits.
fn agent_command(agent: &str, role: Option<&str>, brief_file: Option<&Path>, resume: Option<&Session>) -> Result<Option<String>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let brief_arg = brief_file.map(|f| format!(" \"$(cat {})\"", shell_quote(&f.display().to_string()))).unwrap_or_default();
    let run = match agent {
        "claude" => {
            let settings = format!("--settings {}{}", shell_quote(&hooks_file().display().to_string()), claude_flags(role));
            match resume {
                Some(Session { claude_session: Some(id), .. }) => format!("claude {settings} --resume {}", shell_quote(id)),
                Some(_) => format!("claude {settings} --continue"),
                None => format!("claude {settings}{brief_arg}"),
            }
        }
        "codex" => {
            let notify = format!("notify=[\"{}\",\"hook\",\"codex\"]", exe.display());
            match resume {
                Some(_) => format!("codex -c {} resume --last", shell_quote(&notify)),
                None => format!("codex -c {}{brief_arg}", shell_quote(&notify)),
            }
        }
        "shell" => return Ok(None),
        other => return Err(format!("Agent inconnu : {other}")),
    };
    Ok(Some(format!("{run}; exec \"$SHELL\" -l")))
}

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
    tmux::new_session(&session.id, Path::new(&session.cwd), &env, command)
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
    let role = req.role.clone().or_else(|| lead.map(|_| "worker".to_string()));
    // Fail before creating anything the command could not use.
    agent_command(&req.agent, role.as_deref(), None, None)?;

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
        (None, None) => if req.agent == "shell" { "Terminal".into() } else { format!("{} libre", req.agent) },
    };

    let brief_file = match brief(&project, item.as_ref(), &req.prompt, role.as_deref() == Some("worker")) {
        Some(text) if req.agent != "shell" => {
            let f = paths::studio_dir().join("briefs").join(format!("{id}.md"));
            paths::write_atomic(&f, &text)?;
            Some(f)
        }
        _ => None,
    };
    let mut command = agent_command(&req.agent, role.as_deref(), brief_file.as_deref(), None)?;
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
    if !Path::new(&session.cwd).is_dir() {
        return Err(format!("{} n'existe plus", session.cwd));
    }
    let command = agent_command(&session.agent, session.role.as_deref(), None, Some(&session))?;
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

/// What the orchestrator is told; the procedure itself is `studio guide orchestrate`.
fn orchestrate_prompt(project: &store::Project, branch: &str, parallel: u32, instructions: &str) -> String {
    let mut text = format!(
        "Tu orchestres le travail sur {} en autonomie. Lance d'abord `studio guide orchestrate` et suis-le jusqu'au \
         bout. Ta branche d'intégration est {branch} ; au plus {parallel} agents en parallèle.",
        project.meta.name
    );
    if !instructions.trim().is_empty() {
        text += &format!("\n\nConsigne de l'utilisateur, qui prime sur le guide :\n{}", instructions.trim());
    }
    text
}

/// Starts a project's orchestrator: Claude in a worktree of its own, on a new integration branch, where it merges
/// the work of the agents it launches. One at a time per project, or two would pick the same items.
pub fn orchestrate(app: &AppHandle, project: &str, parallel: u32, instructions: &str) -> Result<Session, String> {
    if !(1..=8).contains(&parallel) {
        return Err(format!("Entre 1 et 8 agents en parallèle, pas {parallel}"));
    }
    let project = store::project(project)?;
    let live = list().into_iter().find(|s| s.project == project.id && s.role.as_deref() == Some("orchestrator") && s.status != "offline");
    if let Some(s) = live {
        return Err(format!("Un orchestrateur tourne déjà sur {} : {}", project.meta.name, s.id));
    }
    let branch = format!("orchestration-{}", paths::local_stamp().replace(' ', "-").replace(':', ""));
    let req = StartRequest {
        project: project.id.clone(),
        item: None,
        agent: "claude".into(),
        place: "worktree".into(),
        prompt: orchestrate_prompt(&project, &branch, parallel, instructions),
        branch: Some(branch),
        path: None,
        title: Some("Orchestration".into()),
        parent: None,
        role: Some("orchestrator".into()),
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
    if session.agent != "shell" {
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
    update(id, |s| {
        // A worker idle at its prompt waits on its orchestrator, not on the user: only a permission turns it amber.
        if s.role.as_deref() == Some("worker") && text("notification_type").as_deref() == Some("idle_prompt") {
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
                _ => None,
            };
        }
    });
    if moved {
        changed(app);
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
        let worker = brief(&project, Some(&item), "Seulement le torse.", true).unwrap();
        assert!(worker.contains("Fait quand le torse se porte.\n\nSeulement le torse."));
        assert!(worker.contains("`studio status FRO-1 review`"));
        assert!(worker.contains("Ne pousse pas"));
        let attended = brief(&project, Some(&item), "", false).unwrap();
        assert!(attended.contains("`done` une fois vérifié"));
        assert!(!attended.contains("sans surveillance"));
        assert!(brief(&project, None, " ", false).is_none());
    }

    #[test]
    fn unattended_sessions_run_in_auto_mode() {
        assert_eq!(claude_flags(None), "");
        assert_eq!(claude_flags(Some("worker")), " --permission-mode auto");
        assert!(claude_flags(Some("orchestrator")).contains("--model opus --effort xhigh"));
        let (project, _) = fixtures();
        let prompt = orchestrate_prompt(&project, "orchestration-2026-10-29-1432", 3, "Seulement les bugs.");
        assert!(prompt.contains("`studio guide orchestrate`"));
        assert!(prompt.contains("orchestration-2026-10-29-1432 ; au plus 3 agents"));
        assert!(prompt.ends_with("Seulement les bugs."));
    }
}
