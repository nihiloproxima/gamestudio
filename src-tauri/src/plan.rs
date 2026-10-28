//! `studio import`: a whole plan (project settings, vision, milestones, items) in one JSON document, as an agent
//! setting a game up writes it. Everything is checked before anything is written, a dry run reports what would
//! happen, and importing twice is harmless: milestones and items already there (same title) are reused or skipped.

use serde::{Deserialize, Serialize};

use crate::store::{self, ItemDraft, MilestoneMeta, ProjectPatch};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    #[serde(default)]
    pub project: Option<ProjectPatch>,
    #[serde(default)]
    pub milestones: Vec<PlanMilestone>,
    #[serde(default)]
    pub items: Vec<PlanItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanMilestone {
    pub title: String,
    #[serde(default = "planned")]
    pub status: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub goal: String,
}

fn planned() -> String {
    "planned".into()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanItem {
    pub title: String,
    #[serde(default = "task")]
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    /// A milestone of this plan or of the project, by title or id.
    #[serde(default)]
    pub milestone: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub body: String,
}

fn task() -> String {
    "task".into()
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub dry_run: bool,
    pub project_updated: bool,
    pub milestones_created: Vec<String>,
    pub milestones_existing: Vec<String>,
    pub items_created: Vec<String>,
    /// Items left out because one with the same title already exists.
    pub items_existing: Vec<String>,
    /// Per status, what the import adds.
    pub by_status: Vec<(String, usize)>,
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// A planned milestone named by its title or by the id its title will give.
fn names(title: &str, name: &str) -> bool {
    same(title, name) || store::slugify(title, 40) == name.trim()
}

pub fn import(project_id: &str, plan: Plan, dry_run: bool) -> Result<Report, String> {
    let project = store::project(project_id)?;
    let existing_milestones = store::milestones(project_id);
    let existing_items = store::items(project_id);
    let mut report = Report { dry_run, ..Default::default() };

    // Check everything first.
    let mut errors = Vec::new();
    if let Some(patch) = &plan.project {
        if patch.key.as_deref().is_some_and(|k| !k.eq_ignore_ascii_case(&project.meta.key)) && !existing_items.is_empty() {
            errors.push("La clé ne change pas sur un projet qui a déjà des éléments".to_string());
        }
    }
    for (i, m) in plan.milestones.iter().enumerate() {
        let meta = MilestoneMeta { title: m.title.clone(), status: m.status.clone(), target: m.target.clone(), order: 0 };
        if let Err(e) = store::validate_milestone(&meta) {
            errors.push(format!("Jalon {} « {} » : {e}", i + 1, m.title));
        }
    }
    let planned_titles: Vec<String> = plan.milestones.iter().map(|m| m.title.clone()).collect();
    let resolve = |name: &str| -> Option<String> {
        existing_milestones
            .iter()
            .find(|m| m.id == name || same(&m.meta.title, name))
            .map(|m| m.id.clone())
            .or_else(|| planned_titles.iter().any(|t| names(t, name)).then(|| name.to_string()))
    };
    let mut drafts = Vec::new();
    for (i, it) in plan.items.iter().enumerate() {
        let milestone = it.milestone.as_deref().filter(|m| !m.trim().is_empty());
        if let Some(m) = milestone {
            if resolve(m).is_none() {
                errors.push(format!("Élément {} « {} » : jalon inconnu « {m} »", i + 1, it.title));
            }
        }
        let mut draft = ItemDraft {
            title: it.title.clone(),
            kind: it.kind.clone(),
            status: it.status.clone(),
            priority: it.priority.clone(),
            milestone: milestone.map(str::to_owned),
            labels: it.labels.clone(),
            body: it.body.clone(),
        };
        match store::validate_draft(&mut draft) {
            Ok(()) => drafts.push(draft),
            Err(e) => errors.push(format!("Élément {} « {} » : {e}", i + 1, it.title)),
        }
    }
    if !errors.is_empty() {
        return Err(format!("Plan refusé, rien n'a été écrit :\n- {}", errors.join("\n- ")));
    }

    // Project settings and vision.
    if let Some(patch) = plan.project.filter(|p| !p.is_empty()) {
        report.project_updated = true;
        if !dry_run {
            store::update_project(project_id, patch)?;
        }
    }

    // Milestones, appended after the existing ones; titles map to the ids they get.
    let mut ids: Vec<(String, String)> = Vec::new();
    let mut order = existing_milestones.iter().map(|m| m.meta.order).max().unwrap_or(-1) + 1;
    for m in plan.milestones {
        if let Some(found) = existing_milestones.iter().find(|x| same(&x.meta.title, &m.title)) {
            report.milestones_existing.push(found.meta.title.clone());
            ids.push((m.title, found.id.clone()));
            continue;
        }
        report.milestones_created.push(m.title.clone());
        let id = if dry_run {
            store::slugify(&m.title, 40)
        } else {
            let meta = MilestoneMeta { title: m.title.trim().to_string(), status: m.status, target: m.target, order };
            store::save_milestone(project_id, None, meta, m.goal)?.id
        };
        order += 1;
        ids.push((m.title, id));
    }

    // Items.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for mut draft in drafts {
        if existing_items.iter().any(|x| same(&x.meta.title, &draft.title)) {
            report.items_existing.push(draft.title);
            continue;
        }
        draft.milestone = draft.milestone.and_then(|name| {
            ids.iter().find(|(title, _)| names(title, &name)).map(|(_, id)| id.clone()).or_else(|| resolve(&name))
        });
        let status = draft.status.clone().unwrap_or_default();
        match counts.iter_mut().find(|(s, _)| *s == status) {
            Some((_, n)) => *n += 1,
            None => counts.push((status, 1)),
        }
        if dry_run {
            report.items_created.push(draft.title);
        } else {
            let item = store::create_item(project_id, draft)?;
            report.items_created.push(format!("{} {}", item.id, item.meta.title));
        }
    }
    report.by_status = counts;
    Ok(report)
}
