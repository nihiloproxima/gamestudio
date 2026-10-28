//! Each game's tracker data lives on a `gamestudio` branch of the game's own repo, so it travels with the game: clone
//! the game on another Mac, add it to Gamestudio, and its roadmap comes back. The branch shares no history with the
//! game's code and is checked out as a small separate clone in `~/.gamestudio/data/<project>`, never in the game's
//! working tree. One worker thread owns every git operation on these clones: it commits what changed, fetches,
//! rebases onto the remote and pushes, a few seconds after the last edit and every few minutes otherwise.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::{paths, store};

/// The branch of each game's repo that holds its tracker data.
pub const BRANCH: &str = "gamestudio";
const PERIOD: Duration = Duration::from_secs(180);
const DEBOUNCE: Duration = Duration::from_secs(6);
const GITIGNORE: &str = ".*.tmp\n.DS_Store\n";

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub syncing: bool,
    pub last_sync: Option<String>,
    /// Per game, prefixed by its name.
    pub error: Option<String>,
    /// Changes on this machine that the remotes do not have yet.
    pub pending: u32,
    /// What the last sync merged that is worth knowing.
    pub notice: Option<String>,
    /// Where each game's data is pushed, by project id.
    pub remotes: BTreeMap<String, String>,
}

enum Msg {
    /// Something was written: sync once things settle.
    Touch,
    /// Sync now; the result goes back to the caller.
    Now(Sender<Result<SyncStatus, String>>),
}

static STATUS: Mutex<Option<SyncStatus>> = Mutex::new(None);
static WORKER: OnceLock<Sender<Msg>> = OnceLock::new();

pub fn status() -> SyncStatus {
    let mut current = STATUS.lock().unwrap().clone().unwrap_or_default();
    current.remotes = store::data_dirs()
        .into_iter()
        .filter_map(|(id, dir)| remote_url(&dir).map(|url| (id, url)))
        .collect();
    current
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut SyncStatus)) {
    let mut guard = STATUS.lock().unwrap();
    let s = guard.get_or_insert_with(SyncStatus::default);
    f(s);
    drop(guard);
    let _ = app.emit("sync-changed", status());
}

/// Asks for a sync once edits settle. Cheap; safe to call after every write.
pub fn touch() {
    if let Some(tx) = WORKER.get() {
        let _ = tx.send(Msg::Touch);
    }
}

/// Syncs right away and waits for the outcome.
pub fn now() -> Result<SyncStatus, String> {
    let tx = WORKER.get().ok_or("La synchro n'a pas démarré")?;
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(Msg::Now(reply_tx)).map_err(|_| "La synchro s'est arrêtée")?;
    reply_rx.recv().map_err(|_| "La synchro s'est arrêtée".to_string())?
}

pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel::<Msg>();
    if WORKER.set(tx).is_err() {
        return;
    }
    std::thread::spawn(move || {
        // A first pass at startup brings the other machine's changes in.
        let mut reply: Option<Sender<Result<SyncStatus, String>>> = None;
        loop {
            let result = run_once(&app);
            if let Some(r) = reply.take() {
                let _ = r.send(result.map(|_| status()));
            }
            match rx.recv_timeout(PERIOD) {
                Ok(Msg::Now(r)) => reply = Some(r),
                Ok(Msg::Touch) => loop {
                    match rx.recv_timeout(DEBOUNCE) {
                        Ok(Msg::Touch) => continue,
                        Ok(Msg::Now(r)) => {
                            reply = Some(r);
                            break;
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    });
}

fn git(dir: &Path) -> Command {
    let mut cmd = paths::git(dir);
    // Never wait on a prompt nobody can see: no passphrase, no credentials, a bounded connection attempt.
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=10");
    cmd
}

fn remote_url(dir: &Path) -> Option<String> {
    paths::run(git(dir).args(["remote", "get-url", "origin"])).ok().map(|s| s.trim().to_string())
}

fn machine_name() -> String {
    paths::run(Command::new("scutil").args(["--get", "ComputerName"]))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "ce Mac".into())
}

/// Commits whatever changed in a game's data. Returns whether there was anything.
fn commit_local(dir: &Path) -> Result<bool, String> {
    let _guard = store::lock();
    paths::run(git(dir).args(["add", "-A"]))?;
    let changed = paths::run(git(dir).args(["status", "--porcelain"]))?;
    let count = changed.lines().count();
    if count == 0 {
        return Ok(false);
    }
    let message = format!("{} : {count} fichier{} modifié{}", machine_name(), plural(count), plural(count));
    paths::run(git(dir).args(["commit", "-q", "-m", &message]))?;
    Ok(true)
}

fn plural(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

fn count(dir: &Path, range: &str) -> u32 {
    paths::run(git(dir).args(["rev-list", "--count", range])).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn run_once(app: &AppHandle) -> Result<(), String> {
    let dirs = store::data_dirs();
    if dirs.is_empty() {
        return Ok(());
    }
    set_status(app, |s| s.syncing = true);
    let (mut errors, mut notices, mut pending) = (Vec::new(), Vec::new(), 0);
    for (id, dir) in &dirs {
        let name = store::project(id).map(|p| p.meta.name).unwrap_or_else(|_| id.clone());
        match sync(app, dir) {
            Ok(Some(notice)) => notices.push(format!("{name} : {notice}")),
            Ok(None) => {}
            Err(e) => errors.push(format!("{name} : {e}")),
        }
        pending += unpushed(dir);
    }
    set_status(app, |s| {
        s.syncing = false;
        s.pending = pending;
        s.error = (!errors.is_empty()).then(|| errors.join("\n"));
        if errors.is_empty() {
            s.last_sync = Some(paths::now_iso());
        }
        s.notice = (!notices.is_empty()).then(|| notices.join("\n"));
    });
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

/// Local commits the remote does not have yet.
fn unpushed(dir: &Path) -> u32 {
    let upstream = format!("origin/{BRANCH}");
    if remote_url(dir).is_none() {
        0
    } else if paths::run(git(dir).args(["rev-parse", "--verify", "-q", &upstream])).is_ok() {
        count(dir, &format!("{upstream}..HEAD"))
    } else {
        count(dir, "HEAD")
    }
}

/// Files changed both here and on the remote since they last agreed.
fn changed_on_both_sides(dir: &Path, upstream: &str) -> Vec<String> {
    let Ok(base) = paths::run(git(dir).args(["merge-base", "HEAD", upstream])) else { return Vec::new() };
    let base = base.trim();
    let names = |range: String| -> Vec<String> {
        paths::run(git(dir).args(["diff", "--name-only", &range])).unwrap_or_default().lines().map(str::to_owned).collect()
    };
    let theirs = names(format!("{base}..{upstream}"));
    names(format!("{base}..HEAD")).into_iter().filter(|f| theirs.contains(f)).collect()
}

/// `items/JEU-3.md` gives `JEU-3`.
fn short_name(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file).trim_end_matches(".md")
}

/// One round trip for one game. Returns a notice when both machines had edited the same files.
fn sync(app: &AppHandle, dir: &Path) -> Result<Option<String>, String> {
    commit_local(dir)?;
    if remote_url(dir).is_none() {
        return Ok(None);
    }
    paths::run(git(dir).args(["fetch", "-q", "origin", BRANCH]))
        .or_else(|e| if e.contains("couldn't find remote ref") { Ok(String::new()) } else { Err(e) })
        .map_err(|e| format!("dépôt injoignable : {}", first_line(&e)))?;
    let upstream = format!("origin/{BRANCH}");
    let remote_exists = paths::run(git(dir).args(["rev-parse", "--verify", "-q", &upstream])).is_ok();

    let mut notice = None;
    if remote_exists && count(dir, &format!("HEAD..{upstream}")) > 0 {
        let both = changed_on_both_sides(dir, &upstream);
        let _guard = store::lock();
        // Each item is its own file, so the two machines rarely touch the same one. When they do, git merges line by
        // line and this machine's line wins where they disagree; the other version stays in the history.
        if let Err(e) = paths::run(git(dir).args(["rebase", "-q", "-X", "theirs", &upstream])) {
            let _ = paths::run(git(dir).args(["rebase", "--abort"]));
            return Err(format!(
                "conflit avec une autre machine, rien n'a été écrasé. À résoudre dans {} (git status) : {}",
                dir.display(),
                first_line(&e)
            ));
        }
        drop(_guard);
        if !both.is_empty() {
            let names: Vec<&str> = both.iter().map(|f| short_name(f)).collect();
            notice = Some(format!(
                "{} modifié{} sur les deux machines : fusionné, ta version l'emporte là où elles divergeaient",
                names.join(", "),
                plural(names.len())
            ));
        }
        let _ = app.emit("data-changed", ());
    }

    if unpushed(dir) > 0 {
        paths::run(git(dir).args(["push", "-q", "origin", &format!("HEAD:{BRANCH}")]))
            .map_err(|e| format!("envoi refusé, nouvel essai à la prochaine synchro : {}", first_line(&e)))?;
    }
    Ok(notice)
}

fn first_line(text: &str) -> &str {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text).trim()
}

// ---------------------------------------------------------------------------------------------------------------
// Adding a game

/// Whether `remote` has the data branch. A remote that cannot be reached is an error, not a "no": creating a second
/// history for a game that already has one would fork its data.
fn has_branch(repo: &Path, remote: &str) -> Result<bool, String> {
    let out = git(repo)
        .args(["ls-remote", "--exit-code", "--heads", remote, BRANCH])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(2) => Ok(false),
        _ => Err(format!(
            "Impossible de joindre {remote} pour savoir s'il a déjà des données Gamestudio : {}",
            first_line(&String::from_utf8_lossy(&out.stderr))
        )),
    }
}

/// Starts tracking the game whose repo is at `meta.path`, and returns its project id. When the repo (its `origin`,
/// else the repo itself) already has the data branch, its data comes back as it is, from another machine or an
/// earlier install; otherwise a new branch starts from `meta`.
pub fn attach(meta: store::ProjectMeta) -> Result<String, String> {
    let repo = paths::canonical(&paths::expand(meta.path.trim()));
    if !repo.join(".git").exists() {
        return Err(format!("{} n'est pas un dépôt git", meta.path.trim()));
    }
    if let Some((id, _)) = store::registry().into_iter().find(|(_, p)| paths::canonical(&paths::expand(p)) == repo) {
        return Err(format!("Ce dépôt est déjà suivi (projet {id})"));
    }
    let remote = remote_url(&repo).unwrap_or_else(|| repo.display().to_string());
    std::fs::create_dir_all(paths::data_dir()).map_err(|e| e.to_string())?;

    if has_branch(&repo, &remote)? {
        let tmp = paths::data_dir().join(format!(".attach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        paths::run(git(&paths::data_dir()).args(["clone", "-q", "--single-branch", "--branch", BRANCH, &remote]).arg(&tmp))
            .map_err(|e| format!("Récupération des données impossible : {}", first_line(&e)))?;
        let name = store::read_name(&tmp).unwrap_or_else(|| meta.name.clone());
        let id = store::free_id(&name)?;
        std::fs::rename(&tmp, paths::data_dir().join(&id)).map_err(|e| e.to_string())?;
        store::register(&id, &meta.path)?;
        return Ok(id);
    }

    store::validate_project(&meta)?;
    let id = store::free_id(&meta.name)?;
    let dir = paths::data_dir().join(&id);
    paths::run(git(&paths::data_dir()).args(["init", "-q", "-b", BRANCH]).arg(&dir))?;
    paths::write_atomic(&dir.join(".gitignore"), GITIGNORE)?;
    store::save_project(&id, meta, String::new())?;
    commit_local(&dir)?;
    paths::run(git(&dir).args(["remote", "add", "origin", &remote]))?;
    touch();
    Ok(id)
}
