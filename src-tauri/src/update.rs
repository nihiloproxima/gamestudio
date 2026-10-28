//! Self-update from the GitHub releases of the repo the app is built from (`repository` in Cargo.toml), through `gh`
//! so a private repo works too and no token is stored. An installed app checks shortly after starting and every six
//! hours; a newer release is downloaded, checked, and swapped in place of the running bundle, then the user is told
//! to restart. Agents are not affected: they live in tmux.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::paths;

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const PERIOD: Duration = Duration::from_secs(6 * 3600);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current: String,
    /// `disabled` (not running from an installed app), `idle`, `checking`, `installing`, `ready` (installed, restart
    /// to use it) or `error`.
    pub state: String,
    pub latest: Option<String>,
    pub error: Option<String>,
    pub checked_at: Option<String>,
}

static STATUS: Mutex<Option<UpdateStatus>> = Mutex::new(None);
/// One check at a time, whether from the timer or the Settings button.
static RUNNING: Mutex<()> = Mutex::new(());

/// `owner/name` of the repo releases come from.
fn repo_slug() -> Option<String> {
    let slug = REPOSITORY.strip_prefix("https://github.com/")?.trim_end_matches('/').trim_end_matches(".git");
    (slug.split('/').count() == 2).then(|| slug.to_string())
}

/// The `.app` holding the running binary. A dev build (`pnpm tauri dev`) has none and never updates itself.
fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

pub fn status() -> UpdateStatus {
    STATUS.lock().unwrap().clone().unwrap_or_else(|| UpdateStatus {
        current: CURRENT.into(),
        state: if bundle().is_some() && repo_slug().is_some() { "idle" } else { "disabled" }.into(),
        latest: None,
        error: None,
        checked_at: None,
    })
}

fn set(app: &AppHandle, f: impl FnOnce(&mut UpdateStatus)) -> UpdateStatus {
    let mut s = status();
    f(&mut s);
    *STATUS.lock().unwrap() = Some(s.clone());
    let _ = app.emit("update-changed", &s);
    s
}

/// `v1.2.3` or `1.2.3` gives (1, 2, 3); anything after a `-` (pre-release) is ignored.
fn version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches('v').split('-').next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next().flatten().unwrap_or(0)))
}

/// The latest published release: its tag and the app archive built for this Mac's architecture.
fn latest_release(slug: &str) -> Result<(String, String), String> {
    let out = paths::run(Command::new("gh").args(["release", "view", "--repo", slug, "--json", "tagName,assets"]))
        .map_err(|e| format!("gh release view : {e}"))?;
    pick_asset(&serde_json::from_str(&out).map_err(|e| e.to_string())?)
}

fn pick_asset(release: &Value) -> Result<(String, String), String> {
    let tag = release["tagName"].as_str().ok_or("Release sans tag")?.to_string();
    let archives: Vec<&str> = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| a["name"].as_str())
        .filter(|n| n.ends_with(".app.tar.gz"))
        .collect();
    let asset = archives
        .iter()
        .find(|n| n.contains(std::env::consts::ARCH))
        .or(archives.first())
        .ok_or_else(|| format!("La release {tag} n'a pas d'archive .app.tar.gz"))?;
    Ok((tag, asset.to_string()))
}

/// Puts `new` where `target` is, keeping `target` until the new one is in place, and putting it back on failure.
/// Copying first (ditto) works across volumes; the two renames that follow stay in `target`'s folder.
fn swap(new: &Path, target: &Path) -> Result<(), String> {
    let parent = target.parent().ok_or("Bundle sans dossier parent")?;
    let name = target.file_name().and_then(|n| n.to_str()).ok_or("Bundle sans nom")?;
    let staged = parent.join(format!(".{name}.new"));
    let old = parent.join(format!(".{name}.old"));
    let _ = std::fs::remove_dir_all(&staged);
    let _ = std::fs::remove_dir_all(&old);
    paths::run(Command::new("ditto").arg(new).arg(&staged)).map_err(|e| format!("Copie de la mise à jour : {e}"))?;
    std::fs::rename(target, &old).map_err(|e| format!("{} ne peut pas être remplacée : {e}", target.display()))?;
    if let Err(e) = std::fs::rename(&staged, target) {
        let _ = std::fs::rename(&old, target);
        return Err(format!("Mise en place de la mise à jour : {e}"));
    }
    let _ = std::fs::remove_dir_all(&old);
    // Downloaded through gh, it carries no quarantine flag; clear one anyway so Gatekeeper never blocks the restart.
    let _ = paths::run(Command::new("xattr").args(["-dr", "com.apple.quarantine"]).arg(target));
    Ok(())
}

fn bundle_version(app: &Path) -> Option<String> {
    paths::run(Command::new("plutil").args(["-extract", "CFBundleShortVersionString", "raw"]).arg(app.join("Contents/Info.plist")))
        .ok()
        .map(|v| v.trim().to_string())
}

/// Looks for a newer release and installs it. Returns the status it leaves.
pub fn check(app: &AppHandle) -> UpdateStatus {
    let Ok(_running) = RUNNING.try_lock() else { return status() };
    let (Some(target), Some(slug)) = (bundle(), repo_slug()) else { return status() };
    if status().state == "ready" {
        return status();
    }
    set(app, |s| {
        s.state = "checking".into();
        s.error = None;
    });
    match install_latest(app, &target, &slug) {
        Ok(Some(tag)) => {
            let s = set(app, |s| {
                s.state = "ready".into();
                s.latest = Some(tag.clone());
                s.checked_at = Some(paths::now_iso());
            });
            let _ = app
                .notification()
                .builder()
                .title(format!("Gamestudio {} est installée", tag.trim_start_matches('v')))
                .body("Redémarre l'app pour l'utiliser. Tes agents continuent de tourner.")
                .show();
            s
        }
        Ok(None) => set(app, |s| {
            s.state = "idle".into();
            s.checked_at = Some(paths::now_iso());
        }),
        Err(e) => set(app, |s| {
            s.state = "error".into();
            s.error = Some(e);
            s.checked_at = Some(paths::now_iso());
        }),
    }
}

/// The tag installed, or None when this version is already the latest.
fn install_latest(app: &AppHandle, target: &Path, slug: &str) -> Result<Option<String>, String> {
    let (tag, asset) = latest_release(slug)?;
    set(app, |s| s.latest = Some(tag.clone()));
    let (Some(latest), Some(current)) = (version(&tag), version(CURRENT)) else {
        return Err(format!("Version illisible : {tag}"));
    };
    if latest <= current {
        return Ok(None);
    }
    set(app, |s| s.state = "installing".into());
    let dir = paths::studio_dir().join("updates").join(&tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    paths::run(Command::new("gh").args(["release", "download", &tag, "--repo", slug, "--pattern", &asset, "--dir"]).arg(&dir))
        .map_err(|e| format!("Téléchargement de {tag} : {e}"))?;
    paths::run(Command::new("tar").arg("-xzf").arg(dir.join(&asset)).arg("-C").arg(&dir))
        .map_err(|e| format!("Archive de {tag} illisible : {e}"))?;
    let name = target.file_name().ok_or("Bundle sans nom")?;
    let new = dir.join(name);
    match bundle_version(&new) {
        Some(v) if version(&v) == Some(latest) => {}
        other => return Err(format!("L'archive de {tag} contient la version {}", other.unwrap_or_else(|| "?".into()))),
    }
    swap(&new, target)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(Some(tag))
}

/// Checks a little after startup, then every six hours.
pub fn start(app: AppHandle) {
    if bundle().is_none() || repo_slug().is_none() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        loop {
            check(&app);
            std::thread::sleep(PERIOD);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(version("v0.10.2"), Some((0, 10, 2)));
        assert_eq!(version("1.2"), Some((1, 2, 0)));
        assert_eq!(version("v2.0.0-beta.1"), Some((2, 0, 0)));
        assert!(version("v0.10.0") > version("0.9.9"));
        assert_eq!(version("nightly"), None);
    }

    #[test]
    fn asset_for_this_architecture() {
        let release = json!({ "tagName": "v0.2.0", "assets": [
            { "name": "SHA256SUMS" },
            { "name": "Gamestudio_0.2.0_x86_64.app.tar.gz" },
            { "name": "Gamestudio_0.2.0_aarch64.app.tar.gz" },
        ]});
        let (tag, asset) = pick_asset(&release).unwrap();
        assert_eq!(tag, "v0.2.0");
        assert!(asset.contains(std::env::consts::ARCH));
        assert!(pick_asset(&json!({ "tagName": "v1.0.0", "assets": [] })).is_err());
    }

    #[test]
    fn swap_replaces_and_keeps_nothing_behind() {
        let root = std::env::temp_dir().join(format!("studio-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (target, new) = (root.join("apps/Game.app"), root.join("dl/Game.app"));
        for (dir, v) in [(&target, "old"), (&new, "new")] {
            std::fs::create_dir_all(dir.join("Contents")).unwrap();
            std::fs::write(dir.join("Contents/v"), v).unwrap();
        }
        swap(&new, &target).unwrap();
        assert_eq!(std::fs::read_to_string(target.join("Contents/v")).unwrap(), "new");
        let left: Vec<_> = std::fs::read_dir(root.join("apps")).unwrap().filter_map(Result::ok).map(|e| e.file_name()).collect();
        assert_eq!(left, vec![std::ffi::OsString::from("Game.app")]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repo_comes_from_the_manifest() {
        assert!(repo_slug().is_some_and(|s| s.ends_with("/gamestudio")));
    }
}
