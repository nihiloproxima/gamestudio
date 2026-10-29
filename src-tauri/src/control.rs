//! The app side of the `studio` CLI: one JSON request per connection on ~/.gamestudio/studio.sock, answered by one
//! JSON line. Every change goes through the same code as the UI and tells the UI to refresh.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::{paths, sessions, store, sync};

pub fn socket_path() -> PathBuf {
    paths::studio_dir().join("studio.sock")
}

/// Another Gamestudio answers on the socket. Two instances would race on sessions.json, so the second must not start.
pub fn already_running() -> bool {
    std::os::unix::net::UnixStream::connect(socket_path()).is_ok()
}

/// Where a command was typed, to infer the project when none is named.
#[derive(Serialize, Deserialize, Default)]
pub struct Context {
    pub project: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum Request {
    Projects,
    Items { ctx: Context, status: Option<String>, all: bool },
    Item { id: String },
    /// `draft` holds `ItemDraft` fields.
    Add { ctx: Context, draft: Value },
    /// `patch` holds `ItemPatch` fields.
    Update { id: String, patch: Value },
    Note { id: String, text: String, author: String },
    Project { ctx: Context },
    /// `patch` holds `ProjectPatch` fields.
    ProjectSet { ctx: Context, patch: Value },
    /// Adds the game whose repo is `patch.path` (default: the repo holding `ctx.cwd`). Its data comes back when the
    /// repo already has a data branch; otherwise `patch` must give at least a name and a key.
    ProjectAdd { ctx: Context, patch: Value },
    Milestones { ctx: Context },
    /// Creates the milestone without `id`, otherwise changes the given fields (title, status, target, goal, order).
    MilestoneSave { ctx: Context, id: Option<String>, fields: Value },
    Import { ctx: Context, plan: Value, dry_run: bool },
    Open { id: String },
    Sessions { ctx: Context },
    /// Starts an agent; `place` is `main`, `worktree` (with `branch`) or `here` (the caller's directory). `caller`,
    /// the agent session running the command if any, becomes the new session's parent.
    Start {
        ctx: Context,
        item: Option<String>,
        agent: String,
        place: String,
        branch: Option<String>,
        prompt: String,
        caller: Option<String>,
    },
    /// Types `text` into a session and submits it; an agent (`caller`) only reaches the sessions it launched.
    Send { caller: Option<String>, session: String, text: String },
    /// The last `lines` lines of a session's screen.
    Peek { session: String, lines: usize },
    /// Ends a session; an agent (`caller`) only ends the sessions it launched.
    Stop { caller: Option<String>, session: String },
    Sync,
    Hook { session: String, event: String, payload: Value },
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reply {
    Ok(Value),
    Error(String),
}

pub fn serve(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let path = socket_path();
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => return eprintln!("Socket {} indisponible : {e}", path.display()),
        };
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        while let Ok((stream, _)) = listener.accept().await {
            tauri::async_runtime::spawn(handle(app.clone(), stream));
        }
    });
}

async fn handle(app: AppHandle, stream: UnixStream) {
    let (read, mut write) = stream.into_split();
    let Ok(Some(line)) = BufReader::new(read).lines().next_line().await else {
        return;
    };
    let reply = match serde_json::from_str::<Request>(&line) {
        Ok(req) => match tauri::async_runtime::spawn_blocking(move || dispatch(&app, req)).await {
            Ok(Ok(value)) => Reply::Ok(value),
            Ok(Err(e)) => Reply::Error(e),
            Err(e) => Reply::Error(e.to_string()),
        },
        Err(e) => Reply::Error(format!("Requête invalide : {e}")),
    };
    let mut bytes = serde_json::to_vec(&reply).unwrap_or_default();
    bytes.push(b'\n');
    let _ = write.write_all(&bytes).await;
}

/// The project a command is about: named (by id, key or name), else the one whose repo or worktrees hold `cwd`.
pub fn resolve_project(ctx: &Context) -> Result<store::Project, String> {
    let projects = store::projects();
    if let Some(wanted) = ctx.project.as_deref().filter(|p| !p.is_empty()) {
        let w = wanted.to_lowercase();
        return projects
            .into_iter()
            .find(|p| p.id == w || p.meta.key.to_lowercase() == w || p.meta.name.to_lowercase() == w)
            .ok_or_else(|| format!("Projet inconnu : {wanted}"));
    }
    let cwd = paths::canonical(ctx.cwd.as_deref().map(Path::new).ok_or("Précise le projet avec --project")?);
    projects
        .into_iter()
        .find(|p| {
            cwd.starts_with(paths::canonical(&paths::expand(&p.meta.path)))
                || cwd.starts_with(paths::canonical(&paths::worktrees_dir().join(&p.id)))
        })
        .ok_or_else(|| "Ce dossier n'appartient à aucun projet : précise-le avec --project".into())
}

fn parse<T: serde::de::DeserializeOwned>(value: Value, what: &str) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("{what} invalide : {e}"))
}

/// A milestone named by id or title.
fn milestone_id(project: &str, name: &str) -> Result<String, String> {
    let wanted = name.trim().to_lowercase();
    store::milestones(project)
        .into_iter()
        .find(|m| m.id == wanted || m.meta.title.to_lowercase() == wanted)
        .map(|m| m.id)
        .ok_or_else(|| format!("Jalon inconnu : {name} (studio milestones les liste)"))
}

/// Tells the UI and the sync that data changed.
pub fn data_changed(app: &AppHandle) {
    let _ = app.emit("data-changed", ());
    sync::touch();
}

fn dispatch(app: &AppHandle, req: Request) -> Result<Value, String> {
    match req {
        Request::Projects => Ok(json!(store::projects())),
        Request::Items { ctx, status, all } => {
            let project = resolve_project(&ctx)?;
            let items: Vec<store::Item> = store::items(&project.id)
                .into_iter()
                .filter(|i| match &status {
                    Some(s) => &i.meta.status == s,
                    None => all || !matches!(i.meta.status.as_str(), "done" | "dropped"),
                })
                .collect();
            Ok(json!(items))
        }
        Request::Item { id } => Ok(json!(store::item(&id)?)),
        Request::Add { ctx, draft } => {
            let project = resolve_project(&ctx)?;
            let mut draft: store::ItemDraft = parse(draft, "élément")?;
            draft.milestone = draft.milestone.map(|m| milestone_id(&project.id, &m)).transpose()?;
            let item = store::create_item(&project.id, draft)?;
            data_changed(app);
            Ok(json!(item))
        }
        Request::Update { id, patch } => {
            let mut patch: store::ItemPatch = parse(patch, "modification")?;
            if let Some(m) = patch.milestone.as_deref().filter(|m| !m.is_empty()) {
                patch.milestone = Some(milestone_id(&store::item(&id)?.project, m)?);
            }
            let item = store::update_item(&id, patch)?;
            data_changed(app);
            Ok(json!(item))
        }
        Request::Project { ctx } => Ok(json!(resolve_project(&ctx)?)),
        Request::ProjectSet { ctx, patch } => {
            let project = resolve_project(&ctx)?;
            let updated = store::update_project(&project.id, parse(patch, "réglage")?)?;
            data_changed(app);
            Ok(json!(updated))
        }
        Request::ProjectAdd { ctx, patch } => {
            let patch: store::ProjectPatch = parse(patch, "projet")?;
            let blank = store::ProjectMeta { color: "#89b4fa".into(), ..Default::default() };
            let (mut meta, vision) = patch.apply(blank, String::new());
            if meta.path.trim().is_empty() {
                let cwd = ctx.cwd.as_deref().ok_or("Précise le dépôt avec --path")?;
                let top = paths::run(paths::git(Path::new(cwd)).args(["rev-parse", "--show-toplevel"]))
                    .map_err(|_| "Ce dossier n'est pas dans un dépôt git : précise-le avec --path")?;
                meta.path = top.trim().to_string();
            }
            crate::worktrees::fill_from_repo(&mut meta);
            let id = sync::attach(meta)?;
            let project = if vision.trim().is_empty() {
                store::project(&id)?
            } else {
                store::update_project(&id, store::ProjectPatch { vision: Some(vision), ..Default::default() })?
            };
            data_changed(app);
            Ok(json!(project))
        }
        Request::Milestones { ctx } => Ok(json!(store::milestones(&resolve_project(&ctx)?.id))),
        Request::MilestoneSave { ctx, id, fields } => {
            let project = resolve_project(&ctx)?;
            let text = |key: &str| fields.get(key).and_then(Value::as_str).map(str::to_owned);
            let list = store::milestones(&project.id);
            let current = match &id {
                Some(id) => Some(list.iter().find(|m| m.id == *id).ok_or_else(|| format!("Jalon inconnu : {id}"))?),
                None => None,
            };
            let meta = store::MilestoneMeta {
                title: text("title").or_else(|| current.map(|m| m.meta.title.clone())).unwrap_or_default(),
                status: text("status").or_else(|| current.map(|m| m.meta.status.clone())).unwrap_or_else(|| "planned".into()),
                target: match text("target") {
                    Some(t) if t.is_empty() => None,
                    Some(t) => Some(t),
                    None => current.and_then(|m| m.meta.target.clone()),
                },
                order: fields
                    .get("order")
                    .and_then(Value::as_i64)
                    .or_else(|| current.map(|m| m.meta.order))
                    .unwrap_or_else(|| list.iter().map(|m| m.meta.order).max().unwrap_or(-1) + 1),
            };
            let goal = text("goal").or_else(|| current.map(|m| m.goal.clone())).unwrap_or_default();
            let saved = store::save_milestone(&project.id, id, meta, goal)?;
            data_changed(app);
            Ok(json!(saved))
        }
        Request::Import { ctx, plan, dry_run } => {
            let project = resolve_project(&ctx)?;
            let report = crate::plan::import(&project.id, parse(plan, "plan")?, dry_run)?;
            if !dry_run {
                data_changed(app);
            }
            Ok(json!(report))
        }
        Request::Note { id, text, author } => {
            let item = store::add_note(&id, &text, &author)?;
            data_changed(app);
            Ok(json!(item))
        }
        Request::Open { id } => {
            let item = store::item(&id)?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            app.emit("open-item", json!({ "project": item.project, "id": item.id })).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        Request::Sessions { ctx } => {
            let project = resolve_project(&ctx)?;
            Ok(json!(sessions::list().into_iter().filter(|s| s.project == project.id).collect::<Vec<_>>()))
        }
        Request::Start { ctx, item, agent, place, branch, prompt, caller } => {
            let project = match item.as_deref() {
                Some(id) => store::item(id)?.project,
                None => resolve_project(&ctx)?.id,
            };
            let (place, path) = match place.as_str() {
                "here" => ("path".to_string(), ctx.cwd.clone()),
                other => (other.to_string(), None),
            };
            let req = sessions::StartRequest { project, item, agent, place, branch, path, prompt, title: None, parent: caller, role: None };
            let session = sessions::start(app, req)?;
            // Typed in a terminal: show it. Launched by an agent: leave the user's view alone.
            if session.parent.is_none() {
                app.emit("navigate-session", &session.id).map_err(|e| e.to_string())?;
            }
            Ok(json!(session))
        }
        Request::Send { caller, session, text } => {
            sessions::send(app, &session, &text, caller.as_deref())?;
            Ok(json!(session))
        }
        Request::Peek { session, lines } => Ok(json!(sessions::peek(&session, lines)?)),
        Request::Stop { caller, session } => {
            sessions::driven(&session, caller.as_deref())?;
            sessions::stop(app, &session)?;
            Ok(json!(session))
        }
        Request::Sync => Ok(json!(sync::now()?)),
        Request::Hook { session, event, payload } => {
            sessions::hook(app, &session, &event, &payload);
            Ok(Value::Null)
        }
    }
}
