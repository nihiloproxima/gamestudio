//! Keeps `~/.gamestudio/data` in step with its GitHub remote, so switching machines is a pull away. One worker thread
//! owns every git operation on it: it commits what changed, fetches, rebases onto the remote and pushes, a few seconds
//! after the last edit and every few minutes otherwise. A conflicting rebase is aborted and reported, never forced.

use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::{paths, store};

const BRANCH: &str = "main";
const PERIOD: Duration = Duration::from_secs(180);
const DEBOUNCE: Duration = Duration::from_secs(6);

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// The data folder exists and is a git repo.
    pub initialized: bool,
    pub remote: Option<String>,
    pub syncing: bool,
    pub last_sync: Option<String>,
    pub error: Option<String>,
    /// Changes on this machine that the remote does not have yet.
    pub pending: u32,
    /// What the last sync merged that is worth knowing.
    pub notice: Option<String>,
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
    let dir = paths::data_dir();
    current.initialized = dir.join(".git").is_dir();
    current.remote = current.initialized.then(|| remote_url(&dir)).flatten();
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

/// Commits whatever changed in the data folder. Returns whether there was anything.
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
    let dir = paths::data_dir();
    if !dir.join(".git").is_dir() {
        return Ok(());
    }
    set_status(app, |s| s.syncing = true);
    let result = sync(app, &dir);
    let pending = unpushed(&dir);
    set_status(app, |s| {
        s.syncing = false;
        s.pending = pending;
        match &result {
            Ok(notice) => {
                s.error = None;
                s.notice = notice.clone();
                s.last_sync = Some(paths::now_iso());
            }
            Err(e) => s.error = Some(e.clone()),
        }
    });
    result.map(|_| ())
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

/// `testgame/items/TST-3.md` gives `TST-3`.
fn short_name(file: &str) -> &str {
    file.rsplit('/').next().unwrap_or(file).trim_end_matches(".md")
}

/// One round trip. Returns a notice when both machines had edited the same files.
fn sync(app: &AppHandle, dir: &Path) -> Result<Option<String>, String> {
    commit_local(dir)?;
    if remote_url(dir).is_none() {
        return Ok(None);
    }
    paths::run(git(dir).args(["fetch", "-q", "origin"])).map_err(|e| format!("GitHub injoignable : {}", first_line(&e)))?;
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
                "Conflit avec une autre machine, rien n'a été écrasé. À résoudre dans {} (git status) : {}",
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
            .map_err(|e| format!("Envoi refusé, nouvel essai à la prochaine synchro : {}", first_line(&e)))?;
    }
    Ok(notice)
}

fn first_line(text: &str) -> &str {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text).trim()
}

// ---------------------------------------------------------------------------------------------------------------
// Setting the data folder up

const GITIGNORE: &str = ".*.tmp\n.DS_Store\n";

/// First machine: an empty data repo; games are added next.
pub fn init_fresh() -> Result<(), String> {
    let dir = paths::data_dir();
    if dir.join(".git").exists() {
        return Err("Les données existent déjà".into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    paths::run(git(&dir).args(["init", "-q", "-b", BRANCH]))?;
    paths::write_atomic(&dir.join(".gitignore"), GITIGNORE)?;
    commit_local(&dir)?;
    Ok(())
}

/// Another machine: the data comes from the GitHub repo the first one pushed to.
pub fn init_clone(url: &str) -> Result<(), String> {
    let dir = paths::data_dir();
    if dir.exists() && std::fs::read_dir(&dir).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(format!("{} n'est pas vide", dir.display()));
    }
    std::fs::create_dir_all(paths::studio_dir()).map_err(|e| e.to_string())?;
    let mut cmd = git(&paths::studio_dir());
    paths::run(cmd.args(["clone", "-q", url.trim()]).arg(&dir)).map_err(|e| format!("Clonage impossible : {e}"))?;
    Ok(())
}

/// Creates a private GitHub repo for the data and pushes to it.
pub fn create_remote(name: &str) -> Result<String, String> {
    let dir = paths::data_dir();
    if remote_url(&dir).is_some() {
        return Err("Les données ont déjà un dépôt distant".into());
    }
    let name = name.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c)) {
        return Err(format!("Nom de dépôt invalide : {name}"));
    }
    commit_local(&dir)?;
    paths::run(
        Command::new("gh")
            .args(["repo", "create", name, "--private", "--source"])
            .arg(&dir)
            .args(["--remote", "origin", "--push"]),
    )
    .map_err(|e| format!("gh repo create a échoué : {e}"))?;
    remote_url(&dir).ok_or_else(|| "Dépôt créé, mais le remote origin manque".into())
}

/// Points the data at an existing, empty remote.
pub fn link_remote(url: &str) -> Result<(), String> {
    let dir = paths::data_dir();
    if remote_url(&dir).is_some() {
        return Err("Les données ont déjà un dépôt distant".into());
    }
    paths::run(git(&dir).args(["remote", "add", "origin", url.trim()]))?;
    Ok(())
}
