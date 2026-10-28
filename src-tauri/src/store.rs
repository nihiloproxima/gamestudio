//! Projects, milestones and items, one Markdown file each under `~/.gamestudio/data/<project>/`, with a TOML front
//! matter. Files are the source of truth: they are re-read on every request, so a `git pull`, a hand edit or another
//! machine's change shows up without any cache to invalidate. Every read and write holds `DATA_LOCK`, which the sync
//! also takes while git rewrites the tree.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::paths;

static DATA_LOCK: Mutex<()> = Mutex::new(());

pub fn lock() -> MutexGuard<'static, ()> {
    DATA_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub const KINDS: [&str; 3] = ["task", "bug", "idea"];
pub const STATUSES: [&str; 7] = ["inbox", "backlog", "todo", "doing", "review", "done", "dropped"];
pub const PRIORITIES: [&str; 4] = ["urgent", "high", "normal", "low"];
pub const MILESTONE_STATUSES: [&str; 3] = ["planned", "active", "done"];

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub name: String,
    /// Item prefix, `JEU` gives `JEU-12`.
    pub key: String,
    pub color: String,
    /// Main checkout, `~/`-relative so it holds on every machine.
    pub path: String,
    /// `owner/name` on GitHub, for PRs and cloning on a new machine.
    #[serde(default)]
    pub github: String,
    /// Where new worktrees branch from.
    #[serde(default = "default_base")]
    pub base: String,
    /// Folders scanned by the assets view, relative to the repo.
    #[serde(default)]
    pub assets: Vec<String>,
    /// Run in a new worktree before its agent starts (dependencies, generated files).
    #[serde(default)]
    pub setup: String,
}

fn default_base() -> String {
    "origin/main".into()
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    #[serde(flatten)]
    pub meta: ProjectMeta,
    /// The game's pitch and current objectives, free Markdown.
    pub vision: String,
    /// The repo is checked out at `path` on this machine.
    #[serde(default)]
    pub present: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneMeta {
    pub title: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub order: i64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub id: String,
    #[serde(flatten)]
    pub meta: MilestoneMeta,
    /// What "reached" means, and anything else worth saying about it.
    pub goal: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemMeta {
    pub title: String,
    pub kind: String,
    pub status: String,
    pub priority: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    pub created: String,
    pub updated: String,
    /// When it went to done or dropped; cleared when reopened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub project: String,
    #[serde(flatten)]
    pub meta: ItemMeta,
    pub body: String,
}

// ---------------------------------------------------------------------------------------------------------------
// Front matter

fn split_front(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("+++\n")?;
    let end = rest.find("\n+++")?;
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']);
    Some((&rest[..end + 1], body))
}

fn read_doc<T: DeserializeOwned>(path: &Path) -> Result<(T, String), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} illisible : {e}", path.display()))?;
    let (front, body) = split_front(&text).ok_or_else(|| format!("{} : en-tête +++ manquant", path.display()))?;
    let meta = toml::from_str(front).map_err(|e| format!("{} : en-tête invalide : {e}", path.display()))?;
    Ok((meta, body.trim_end().to_string()))
}

fn write_doc<T: Serialize>(path: &Path, meta: &T, body: &str) -> Result<(), String> {
    let front = toml::to_string(meta).map_err(|e| e.to_string())?;
    let body = body.trim();
    let text = if body.is_empty() { format!("+++\n{front}+++\n") } else { format!("+++\n{front}+++\n\n{body}\n") };
    paths::write_atomic(path, &text)
}

/// Every `*.md` in a folder, sorted by name; missing folder means none.
fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();
    files
}

fn stem(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string()
}

fn check(value: &str, allowed: &[&str], what: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("{what} inconnu : {value} (attendu : {})", allowed.join(", ")))
    }
}

/// `Armures : slot de torse` gives `armures-slot-de-torse`, accents folded.
pub fn slugify(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.to_lowercase().chars() {
        let c = match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c == 'œ' {
            out.push_str("oe");
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= max {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

// ---------------------------------------------------------------------------------------------------------------
// Projects

fn project_dir(id: &str) -> PathBuf {
    paths::data_dir().join(id)
}

fn read_project(dir: &Path) -> Result<Project, String> {
    let (meta, vision): (ProjectMeta, String) = read_doc(&dir.join("project.md"))?;
    let present = paths::expand(&meta.path).join(".git").exists();
    Ok(Project { id: stem(dir), meta, vision, present })
}

pub fn projects() -> Vec<Project> {
    let _guard = lock();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(paths::data_dir())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join("project.md").is_file())
        .collect();
    dirs.sort();
    dirs.iter().filter_map(|d| read_project(d).map_err(|e| eprintln!("{e}")).ok()).collect()
}

pub fn project(id: &str) -> Result<Project, String> {
    let _guard = lock();
    read_project(&project_dir(id)).map_err(|_| format!("Projet inconnu : {id}"))
}

fn validate_project(meta: &ProjectMeta) -> Result<(), String> {
    if meta.name.trim().is_empty() {
        return Err("Le projet n'a pas de nom".into());
    }
    if !(2..=5).contains(&meta.key.len()) || !meta.key.chars().all(|c| c.is_ascii_uppercase()) {
        return Err("La clé tient en 2 à 5 majuscules, comme JEU".into());
    }
    if meta.path.trim().is_empty() {
        return Err("Il manque le chemin du dépôt".into());
    }
    Ok(())
}

/// Creates the project when `id` is new (from its name), updates it otherwise.
pub fn save_project(id: Option<String>, meta: ProjectMeta, vision: String) -> Result<Project, String> {
    validate_project(&meta)?;
    let _guard = lock();
    let id = match id {
        Some(id) => id,
        None => {
            let id = slugify(&meta.name, 32);
            if id.is_empty() || project_dir(&id).exists() {
                return Err(format!("Un projet {id} existe déjà"));
            }
            id
        }
    };
    let dir = project_dir(&id);
    write_doc(&dir.join("project.md"), &meta, &vision)?;
    read_project(&dir)
}

/// Fields to change on a project; the rest stays.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub key: Option<String>,
    pub color: Option<String>,
    pub path: Option<String>,
    pub github: Option<String>,
    pub base: Option<String>,
    pub assets: Option<Vec<String>>,
    pub setup: Option<String>,
    pub vision: Option<String>,
}

impl ProjectPatch {
    pub fn is_empty(&self) -> bool {
        let ProjectPatch { name, key, color, path, github, base, assets, setup, vision } = self;
        name.is_none()
            && key.is_none()
            && color.is_none()
            && path.is_none()
            && github.is_none()
            && base.is_none()
            && assets.is_none()
            && setup.is_none()
            && vision.is_none()
    }

    /// The project's settings and vision once patched.
    pub fn apply(self, mut meta: ProjectMeta, vision: String) -> (ProjectMeta, String) {
        if let Some(v) = self.name {
            meta.name = v;
        }
        if let Some(v) = self.key {
            meta.key = v.to_uppercase();
        }
        if let Some(v) = self.color {
            meta.color = v;
        }
        if let Some(v) = self.path {
            meta.path = v;
        }
        if let Some(v) = self.github {
            meta.github = v;
        }
        if let Some(v) = self.base {
            meta.base = v;
        }
        if let Some(v) = self.assets {
            meta.assets = v;
        }
        if let Some(v) = self.setup {
            meta.setup = v;
        }
        (meta, self.vision.unwrap_or(vision))
    }
}

pub fn update_project(id: &str, patch: ProjectPatch) -> Result<Project, String> {
    let current = project(id)?;
    let (meta, vision) = patch.apply(current.meta, current.vision);
    save_project(Some(id.to_string()), meta, vision)
}

// ---------------------------------------------------------------------------------------------------------------
// Milestones

fn milestone_path(project: &str, id: &str) -> PathBuf {
    project_dir(project).join("milestones").join(format!("{id}.md"))
}

pub fn milestones(project: &str) -> Vec<Milestone> {
    let _guard = lock();
    let mut list: Vec<Milestone> = md_files(&project_dir(project).join("milestones"))
        .iter()
        .filter_map(|p| match read_doc::<MilestoneMeta>(p) {
            Ok((meta, goal)) => Some(Milestone { id: stem(p), meta, goal }),
            Err(e) => {
                eprintln!("{e}");
                None
            }
        })
        .collect();
    list.sort_by(|a, b| a.meta.order.cmp(&b.meta.order).then(a.id.cmp(&b.id)));
    list
}

pub fn validate_milestone(meta: &MilestoneMeta) -> Result<(), String> {
    check(&meta.status, &MILESTONE_STATUSES, "Statut de jalon")?;
    if meta.title.trim().is_empty() {
        return Err("Le jalon n'a pas de titre".into());
    }
    if let Some(t) = &meta.target {
        let ok = t.len() == 10 && t.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
        if !ok {
            return Err(format!("Échéance invalide : {t} (attendu AAAA-MM-JJ)"));
        }
    }
    Ok(())
}

pub fn save_milestone(project: &str, id: Option<String>, meta: MilestoneMeta, goal: String) -> Result<Milestone, String> {
    validate_milestone(&meta)?;
    let _guard = lock();
    if !project_dir(project).join("project.md").is_file() {
        return Err(format!("Projet inconnu : {project}"));
    }
    let id = match id {
        Some(id) => id,
        None => {
            let base = slugify(&meta.title, 40);
            let base = if base.is_empty() { "jalon".to_string() } else { base };
            let mut id = base.clone();
            let mut n = 2;
            while milestone_path(project, &id).exists() {
                id = format!("{base}-{n}");
                n += 1;
            }
            id
        }
    };
    write_doc(&milestone_path(project, &id), &meta, &goal)?;
    Ok(Milestone { id, meta, goal })
}

/// Deletes a milestone and detaches its items, which stay in the tracker.
pub fn delete_milestone(project: &str, id: &str) -> Result<(), String> {
    let _guard = lock();
    for path in md_files(&project_dir(project).join("items")) {
        let (mut meta, body): (ItemMeta, String) = read_doc(&path)?;
        if meta.milestone.as_deref() == Some(id) {
            meta.milestone = None;
            write_doc(&path, &meta, &body)?;
        }
    }
    std::fs::remove_file(milestone_path(project, id)).map_err(|e| format!("Jalon {id} : {e}"))
}

// ---------------------------------------------------------------------------------------------------------------
// Items

fn items_dir(project: &str) -> PathBuf {
    project_dir(project).join("items")
}

fn read_item(project: &str, path: &Path) -> Result<Item, String> {
    let (meta, body) = read_doc(path)?;
    Ok(Item { id: stem(path), project: project.to_string(), meta, body })
}

/// `JEU-12` gives 12.
fn number(id: &str) -> u32 {
    id.rsplit_once('-').and_then(|(_, n)| n.parse().ok()).unwrap_or(0)
}

/// In creation order: JEU-2 before JEU-10.
pub fn items(project: &str) -> Vec<Item> {
    let _guard = lock();
    let mut list: Vec<Item> = md_files(&items_dir(project))
        .iter()
        .filter_map(|p| read_item(project, p).map_err(|e| eprintln!("{e}")).ok())
        .collect();
    list.sort_by_key(|i| number(&i.id));
    list
}

/// The project holding an item, found by its file rather than its prefix: a project's key may have changed since.
fn locate(id: &str) -> Result<(String, PathBuf), String> {
    let file = format!("{id}.md");
    std::fs::read_dir(paths::data_dir())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| (stem(&e.path()), e.path().join("items").join(&file)))
        .find(|(_, path)| path.is_file())
        .ok_or_else(|| format!("Élément inconnu : {id}"))
}

pub fn item(id: &str) -> Result<Item, String> {
    let _guard = lock();
    let (project, path) = locate(&id.to_uppercase())?;
    read_item(&project, &path)
}

/// Highest number used by any item of the folder, whatever its prefix.
fn last_number(dir: &Path) -> u32 {
    md_files(dir).iter().map(|p| number(&stem(p))).max().unwrap_or(0)
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemDraft {
    pub title: String,
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub milestone: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub body: String,
}

/// Checks a draft and fills its defaults: ideas and raw bugs land in the inbox for triage, tasks are already thought
/// through.
pub fn validate_draft(draft: &mut ItemDraft) -> Result<(), String> {
    draft.title = draft.title.trim().to_string();
    if draft.title.is_empty() {
        return Err("L'élément n'a pas de titre".into());
    }
    check(&draft.kind, &KINDS, "Type")?;
    let status = draft.status.get_or_insert_with(|| if draft.kind == "task" { "todo" } else { "inbox" }.into());
    check(status, &STATUSES, "Statut")?;
    check(draft.priority.get_or_insert_with(|| "normal".into()), &PRIORITIES, "Priorité")
}

pub fn create_item(project: &str, mut draft: ItemDraft) -> Result<Item, String> {
    validate_draft(&mut draft)?;
    let (title, status, priority) = (draft.title, draft.status.unwrap_or_default(), draft.priority.unwrap_or_default());

    let _guard = lock();
    let proj = read_project(&project_dir(project)).map_err(|_| format!("Projet inconnu : {project}"))?;
    let dir = items_dir(project);
    let id = format!("{}-{}", proj.meta.key, last_number(&dir) + 1);
    let now = paths::now_iso();
    let meta = ItemMeta {
        title,
        kind: draft.kind,
        closed: matches!(status.as_str(), "done" | "dropped").then(|| now.clone()),
        status,
        priority,
        milestone: draft.milestone.filter(|m| !m.is_empty()),
        labels: draft.labels,
        created: now.clone(),
        updated: now,
    };
    let path = dir.join(format!("{id}.md"));
    write_doc(&path, &meta, &draft.body)?;
    read_item(project, &path)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemPatch {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    /// `Some("")` detaches the item from its milestone.
    pub milestone: Option<String>,
    pub labels: Option<Vec<String>>,
    pub body: Option<String>,
    /// The `updated` the editor started from: a body edit is refused if the file changed since, rather than
    /// silently dropping what an agent appended meanwhile.
    pub base_updated: Option<String>,
}

pub fn update_item(id: &str, patch: ItemPatch) -> Result<Item, String> {
    let _guard = lock();
    let (project, path) = locate(&id.to_uppercase())?;
    let mut item = read_item(&project, &path)?;
    let m = &mut item.meta;
    if let (Some(_), Some(base)) = (&patch.body, &patch.base_updated) {
        if *base != m.updated {
            return Err(format!("{} a changé entre-temps : recharge-le avant d'enregistrer ta description", item.id));
        }
    }
    if let Some(title) = patch.title {
        if title.trim().is_empty() {
            return Err("Le titre ne peut pas être vide".into());
        }
        m.title = title.trim().to_string();
    }
    if let Some(kind) = patch.kind {
        check(&kind, &KINDS, "Type")?;
        m.kind = kind;
    }
    if let Some(priority) = patch.priority {
        check(&priority, &PRIORITIES, "Priorité")?;
        m.priority = priority;
    }
    if let Some(status) = patch.status {
        check(&status, &STATUSES, "Statut")?;
        let closing = matches!(status.as_str(), "done" | "dropped");
        if !closing {
            m.closed = None;
        } else if m.closed.is_none() || m.status != status {
            m.closed = Some(paths::now_iso());
        }
        m.status = status;
    }
    if let Some(milestone) = patch.milestone {
        m.milestone = Some(milestone).filter(|s| !s.is_empty());
    }
    if let Some(labels) = patch.labels {
        m.labels = labels.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    }
    if let Some(body) = patch.body {
        item.body = body;
    }
    m.updated = paths::now_iso();
    write_doc(&path, &item.meta, &item.body)?;
    Ok(item)
}

/// Appends a dated line under the item's `## Notes`, for agents reporting progress without rewriting the body.
pub fn add_note(id: &str, text: &str, author: &str) -> Result<Item, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("La note est vide".into());
    }
    let _guard = lock();
    let (project, path) = locate(&id.to_uppercase())?;
    let mut item = read_item(&project, &path)?;
    if !item.body.contains("## Notes") {
        item.body = format!("{}\n\n## Notes\n", item.body.trim_end());
    }
    let when = paths::local_stamp();
    let by = if author.is_empty() { String::new() } else { format!(" ({author})") };
    let indented = text.replace('\n', "\n  ");
    item.body = format!("{}\n- **{when}**{by} : {indented}", item.body.trim_end());
    item.meta.updated = paths::now_iso();
    write_doc(&path, &item.meta, &item.body)?;
    Ok(item)
}

pub fn delete_item(id: &str) -> Result<String, String> {
    let _guard = lock();
    let (project, path) = locate(&id.to_uppercase())?;
    std::fs::remove_file(&path).map_err(|e| format!("{id} : {e}"))?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_round_trips() {
        let meta = MilestoneMeta { title: "Jalon 3".into(), status: "active".into(), target: None, order: 3 };
        let dir = std::env::temp_dir().join(format!("studio-test-{}", std::process::id()));
        let path = dir.join("m.md");
        write_doc(&path, &meta, "Fait quand : on joue.\n\n- un\n- deux").unwrap();
        let (back, body): (MilestoneMeta, String) = read_doc(&path).unwrap();
        assert_eq!(back.title, "Jalon 3");
        assert_eq!(back.order, 3);
        assert_eq!(body, "Fait quand : on joue.\n\n- un\n- deux");
        write_doc(&path, &meta, "").unwrap();
        let (_, body): (MilestoneMeta, String) = read_doc(&path).unwrap();
        assert_eq!(body, "");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn body_may_contain_plus_lines() {
        let text = "+++\ntitle = \"x\"\nstatus = \"planned\"\n+++\n\nA\n+++\nB\n";
        let (front, body) = split_front(text).unwrap();
        assert_eq!(front, "title = \"x\"\nstatus = \"planned\"\n");
        assert_eq!(body, "A\n+++\nB\n");
    }

    #[test]
    fn slugs_fold_accents() {
        assert_eq!(slugify("Armures : slot de torse", 40), "armures-slot-de-torse");
        assert_eq!(slugify("Été — œuvre!", 40), "ete-oeuvre");
        assert_eq!(slugify("abcdef", 3), "abc");
    }

    #[test]
    fn numbers_ignore_prefixes() {
        let dir = std::env::temp_dir().join(format!("studio-num-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["JEU-2.md", "OLD-9.md", "JEU-10.md", "notes.txt"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        assert_eq!(last_number(&dir), 10);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
