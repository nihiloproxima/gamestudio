//! Moves an install made under the app's former name, Gamestudio, to Studio's places: `~/.gamestudio` becomes
//! `~/.studio`. Sessions started before the move keep running: `~/.gamestudio` stays as a link to the new folder, the
//! worktrees' links with their repos are repaired, and each Claude conversation recorded under an old worktree path is
//! also reachable under the new one, so `--resume` finds it from the new path. The data branch moves on its own, in
//! `sync`.

use std::path::{Path, PathBuf};

use crate::paths;

fn legacy_dir() -> PathBuf {
    paths::home().join(".gamestudio")
}

/// Whether the app under its former name is still running: moving its folder under it would break it.
pub fn legacy_running() -> bool {
    std::os::unix::net::UnixStream::connect(legacy_dir().join("studio.sock")).is_ok()
}

/// Runs once, before anything reads `~/.studio`. Does nothing when there is nothing to move.
pub fn from_gamestudio() -> Result<(), String> {
    let (old, new) = (legacy_dir(), paths::studio_dir());
    let is_real_dir = std::fs::symlink_metadata(&old).is_ok_and(|m| m.is_dir());
    if !is_real_dir || new.exists() {
        return Ok(());
    }
    std::fs::rename(&old, &new).map_err(|e| format!("{} ne peut pas devenir {} : {e}", old.display(), new.display()))?;
    std::os::unix::fs::symlink(&new, &old).map_err(|e| format!("Lien {} impossible : {e}", old.display()))?;

    for worktree in worktrees(&new.join("worktrees")) {
        // Points the repo's record of the worktree at its new place.
        if let Err(e) = paths::run(paths::git(&worktree).args(["worktree", "repair"])) {
            eprintln!("git worktree repair {} : {e}", worktree.display());
        }
    }
    let sessions = new.join("sessions.json");
    if let Ok(text) = std::fs::read_to_string(&sessions) {
        paths::write_atomic(&sessions, &text.replace("/.gamestudio/", "/.studio/"))?;
    }
    link_claude_projects(&paths::home().join(".claude/projects"), &paths::home());
    Ok(())
}

/// `worktrees/<project>/<branch>`.
fn worktrees(root: &Path) -> Vec<PathBuf> {
    let dirs = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()
    };
    dirs(root).iter().flat_map(|project| dirs(project)).collect()
}

/// Claude keeps a folder of conversations per working directory, named after its path with every character other
/// than a letter or a digit turned into `-`.
fn claude_folder(path: &Path) -> String {
    path.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn link_claude_projects(projects: &Path, home: &Path) {
    let old = claude_folder(&home.join(".gamestudio"));
    let new = claude_folder(&home.join(".studio"));
    for entry in std::fs::read_dir(projects).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix(&old) else { continue };
        let link = projects.join(format!("{new}{rest}"));
        if !link.exists() {
            let _ = std::os::unix::fs::symlink(entry.path(), link);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_conversations_are_reachable_from_the_new_path() {
        let root = std::env::temp_dir().join(format!("studio-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let projects = root.join("projects");
        let home = Path::new("/Users/ada");
        let old = projects.join(claude_folder(&home.join(".gamestudio/worktrees/frostholm/fro-1")));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(projects.join("-Users-ada-code-frostholm")).unwrap();
        link_claude_projects(&projects, home);
        let new = projects.join("-Users-ada--studio-worktrees-frostholm-fro-1");
        assert_eq!(std::fs::read_link(&new).unwrap(), old);
        assert_eq!(std::fs::read_dir(&projects).unwrap().count(), 3);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
