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
}

/// What the "Configurer avec Claude" session is told; the procedure itself is `studio guide setup`.
pub const SETUP_PROMPT: &str = "Configure ce jeu dans Gamestudio. Lance d'abord `studio guide setup` et suis-le pas à \
pas : inventaire du dépôt, questions groupées, aperçu avec `studio import --dry-run`, et import seulement après mon \
accord explicite.";

fn brief(project: &store::Project, item: Option<&store::Item>, prompt: &str) -> Option<String> {
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
    if let Some(item) = item {
        text += &format!(
            "---\nGamestudio suit ce travail. Avec la commande `studio` :\n\
             - `studio item {id}` relit la fiche ;\n\
             - `studio note {id} \"…\"` consigne une décision ou un point d'étape ;\n\
             - `studio status {id} review` quand c'est prêt à vérifier (`done` une fois vérifié) ;\n\
             - `studio idea \"…\"` note une idée hors sujet sans t'y attarder.\n",
            id = item.id
        );
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

/// The shell command that runs the agent, then leaves a login shell behind when it exits.
fn agent_command(agent: &str, brief_file: Option<&Path>, resume: Option<&Session>) -> Result<Option<String>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let brief_arg = brief_file.map(|f| format!(" \"$(cat {})\"", shell_quote(&f.display().to_string()))).unwrap_or_default();
    let run = match agent {
        "claude" => {
            let settings = format!("--settings {}", shell_quote(&hooks_file().display().to_string()));
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
    // Fail before creating anything the command could not use.
    agent_command(&req.agent, None, None)?;

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
            (worktrees::create(&project, branch)?, Some(branch.trim().to_string()))
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

    let brief_file = match brief(&project, item.as_ref(), &req.prompt) {
        Some(text) if req.agent != "shell" => {
            let f = paths::studio_dir().join("briefs").join(format!("{id}.md"));
            paths::write_atomic(&f, &text)?;
            Some(f)
        }
        _ => None,
    };
    let mut command = agent_command(&req.agent, brief_file.as_deref(), None)?;
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
        status: if req.agent == "shell" { "idle" } else { "working" }.into(),
        status_at: now,
        message: None,
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
    let session = list().into_iter().find(|s| s.id == id).ok_or_else(|| format!("Session inconnue : {id}"))?;
    if session.status != "offline" {
        return Err(format!("{id} tourne déjà"));
    }
    if !Path::new(&session.cwd).is_dir() {
        return Err(format!("{} n'existe plus", session.cwd));
    }
    let command = agent_command(&session.agent, None, Some(&session))?;
    spawn(&session, command.as_deref())?;
    update(id, |s| {
        s.status = if s.agent == "shell" { "idle" } else { "working" }.into();
        s.status_at = paths::now_iso();
        s.message = None;
    });
    changed(app);
    list().into_iter().find(|s| s.id == id).ok_or_else(|| "Session perdue".into())
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
        "stop" | "start" | "codex" => "idle",
        _ => return,
    };
    let mut moved = false;
    update(id, |s| {
        if event == "start" {
            if let Some(sid) = text("session_id") {
                s.claude_session = Some(sid);
            }
        }
        // A tool call while waiting means the permission was granted; otherwise only real transitions count.
        if s.status != status {
            moved = true;
            s.status = status.into();
            s.status_at = now;
            s.message = match event {
                "notification" => text("message"),
                "codex" => text("last-assistant-message").map(|m| m.chars().take(400).collect()),
                _ => None,
            };
        }
    });
    if moved {
        changed(app);
    }
}
