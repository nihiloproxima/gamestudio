//! The git worktrees of a project's repo, read from git each time: where every parallel line of work stands.

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::{paths, sessions, store};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    pub branch: Option<String>,
    pub head: String,
    /// The repo's main checkout.
    pub main: bool,
    pub locked: bool,
    /// Uncommitted files.
    pub dirty: u32,
    /// Commits ahead of and behind the project's base.
    pub ahead: u32,
    pub behind: u32,
    pub last_commit: Option<String>,
    pub last_commit_at: Option<String>,
}

pub fn repo_path(project: &store::Project) -> Result<PathBuf, String> {
    let path = paths::expand(&project.meta.path);
    if !path.join(".git").exists() {
        return Err(format!("{} n'est pas cloné dans {}", project.meta.name, project.meta.path));
    }
    Ok(path)
}

/// `git@github.com:owner/name.git` or `https://github.com/owner/name` gives `owner/name`.
fn github_slug(url: &str) -> Option<String> {
    let rest = url.trim().split_once("github.com").map(|(_, r)| r)?;
    let slug = rest.trim_start_matches([':', '/']).trim_end_matches('/').trim_end_matches(".git");
    (slug.split('/').count() == 2).then(|| slug.to_string())
}

/// Fills what a new project leaves blank from its repo: the GitHub repo of `origin`, and the remote's default
/// branch as the base of new worktrees (`origin/main` when it cannot tell).
pub fn fill_from_repo(meta: &mut store::ProjectMeta) {
    let repo = paths::expand(&meta.path);
    let present = repo.join(".git").exists();
    if present && meta.github.trim().is_empty() {
        if let Some(slug) = paths::run(paths::git(&repo).args(["remote", "get-url", "origin"])).ok().and_then(|u| github_slug(&u)) {
            meta.github = slug;
        }
    }
    if meta.base.trim().is_empty() {
        meta.base = present
            .then(|| paths::run(paths::git(&repo).args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])).ok())
            .flatten()
            .map(|h| h.trim().to_string())
            .unwrap_or_else(|| "origin/main".into());
    }
}

fn parse_porcelain(text: &str) -> Vec<(String, String, Option<String>, bool, bool)> {
    let mut out = Vec::new();
    for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let (mut path, mut head, mut branch, mut locked, mut prunable) = (String::new(), String::new(), None, false, false);
        for line in block.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "worktree" => path = value.to_string(),
                "HEAD" => head = value.to_string(),
                "branch" => branch = Some(value.trim_start_matches("refs/heads/").to_string()),
                "locked" => locked = true,
                "prunable" => prunable = true,
                _ => {}
            }
        }
        if !path.is_empty() {
            out.push((path, head, branch, locked, prunable));
        }
    }
    out
}

fn inspect(path: &Path, base: &str) -> (u32, u32, u32, Option<String>, Option<String>) {
    let dirty = paths::run(paths::git(path).args(["status", "--porcelain"])).map(|s| s.lines().count() as u32).unwrap_or(0);
    let (behind, ahead) = paths::run(paths::git(path).args(["rev-list", "--left-right", "--count", &format!("{base}...HEAD")]))
        .ok()
        .and_then(|s| {
            let mut it = s.split_whitespace().map(|n| n.parse::<u32>().unwrap_or(0));
            Some((it.next()?, it.next()?))
        })
        .unwrap_or((0, 0));
    let last = paths::run(paths::git(path).args(["log", "-1", "--format=%s%x1f%cI"])).ok();
    let (subject, at) = match last.as_deref().and_then(|l| l.trim().split_once('\x1f')) {
        Some((s, a)) => (Some(s.to_string()), Some(a.to_string())),
        None => (None, None),
    };
    (dirty, ahead, behind, subject, at)
}

pub fn list(project_id: &str) -> Result<Vec<Worktree>, String> {
    let project = store::project(project_id)?;
    let repo = repo_path(&project)?;
    let text = paths::run(paths::git(&repo).args(["worktree", "list", "--porcelain"]))?;
    Ok(parse_porcelain(&text)
        .into_iter()
        .enumerate()
        .filter(|(_, (path, _, _, _, prunable))| !prunable && Path::new(path).exists())
        .map(|(i, (path, head, branch, locked, _))| {
            let (dirty, ahead, behind, last_commit, last_commit_at) = inspect(Path::new(&path), &project.meta.base);
            Worktree { path, branch, head, main: i == 0, locked, dirty, ahead, behind, last_commit, last_commit_at }
        })
        .collect())
}

/// A new worktree for `branch` under `~/.gamestudio/worktrees/<project>/`: the branch is checked out if it exists
/// (locally or on origin), created from `from` (default: the project's base) otherwise.
pub fn create(project: &store::Project, branch: &str, from: Option<&str>) -> Result<PathBuf, String> {
    let repo = repo_path(project)?;
    let branch = branch.trim();
    paths::run(paths::git(&repo).args(["check-ref-format", "--branch", branch]))
        .map_err(|_| format!("Nom de branche invalide : {branch}"))?;
    let folder = store::slugify(&branch.replace('/', "-"), 60);
    let path = paths::worktrees_dir().join(&project.id).join(folder);
    if path.exists() {
        return Err(format!("{} existe déjà", path.display()));
    }
    let local = paths::run(paths::git(&repo).args(["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")])).is_ok();
    let mut cmd = paths::git(&repo);
    cmd.args(["worktree", "add"]);
    if local {
        cmd.arg(&path).arg(branch);
    } else {
        let _ = paths::run(paths::git(&repo).args(["fetch", "-q", "origin"]));
        let remote = format!("origin/{branch}");
        if paths::run(paths::git(&repo).args(["rev-parse", "--verify", "-q", &remote])).is_ok() {
            cmd.args(["--track", "-b", branch]).arg(&path).arg(remote);
        } else {
            cmd.args(["--no-track", "-b", branch]).arg(&path).arg(from.unwrap_or(&project.meta.base));
        }
    }
    paths::run(&mut cmd).map_err(|e| format!("git worktree add a échoué : {e}"))?;
    Ok(path)
}

/// Removes a finished worktree, keeping its branch. Refused while it holds uncommitted work or a live session.
pub fn remove(project_id: &str, path: &str) -> Result<(), String> {
    let project = store::project(project_id)?;
    let repo = repo_path(&project)?;
    let target = paths::canonical(Path::new(path));
    let target = target.as_path();
    if target == paths::canonical(&repo) {
        return Err("Le dépôt principal ne se supprime pas".into());
    }
    let known = list(project_id)?.into_iter().find(|w| paths::canonical(Path::new(&w.path)) == target);
    let wt = known.ok_or_else(|| format!("{path} n'est pas un worktree de {}", project.meta.name))?;
    if wt.dirty > 0 {
        return Err(format!("{} fichier(s) non commité(s) dans ce worktree : rien n'est supprimé", wt.dirty));
    }
    if let Some(s) = sessions::live_in(target) {
        return Err(format!("La session {s} tourne encore dans ce worktree"));
    }
    paths::run(paths::git(&repo).args(["worktree", "remove"]).arg(target)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_slugs() {
        assert_eq!(github_slug("git@github.com:acme/frostholm.git").as_deref(), Some("acme/frostholm"));
        assert_eq!(github_slug("https://github.com/acme/frostholm/").as_deref(), Some("acme/frostholm"));
        assert_eq!(github_slug("git@gitlab.com:acme/frostholm.git"), None);
    }

    #[test]
    fn porcelain_blocks() {
        let text = "worktree /a\nHEAD 1\nbranch refs/heads/main\n\nworktree /b\nHEAD 2\ndetached\nlocked\n\nworktree /c\nHEAD 3\nbranch refs/heads/feat/x\nprunable gitdir file points to non-existent location\n";
        let parsed = parse_porcelain(text);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].2.as_deref(), Some("main"));
        assert_eq!(parsed[1].2, None);
        assert!(parsed[1].3);
        assert_eq!(parsed[2].2.as_deref(), Some("feat/x"));
        assert!(parsed[2].4);
    }
}
