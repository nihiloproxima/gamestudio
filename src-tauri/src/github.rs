//! Pull requests of a project's GitHub repo, through `gh` and the user's own login.

use serde::Serialize;
use serde_json::Value;
use std::process::Command;

use crate::{paths, store};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    /// `draft`, `open`, `merged` or `closed`.
    pub state: String,
    pub branch: String,
    pub base: String,
    pub url: String,
    pub author: String,
    pub updated_at: String,
    pub merged_at: Option<String>,
    pub additions: u64,
    pub deletions: u64,
    /// `approved`, `changes`, `required` or `none`.
    pub review: String,
    /// `pass`, `fail`, `pending` or `none`.
    pub checks: String,
}

const FIELDS: &str = "number,title,state,isDraft,headRefName,baseRefName,url,author,updatedAt,mergedAt,additions,\
deletions,reviewDecision,statusCheckRollup";

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// One word for a PR's checks: any failure wins, then anything still running.
fn summarize_checks(rollup: &Value) -> String {
    let checks = rollup.as_array().map(Vec::as_slice).unwrap_or_default();
    if checks.is_empty() {
        return "none".into();
    }
    let outcome = |c: &Value| {
        let conclusion = str_of(c, "conclusion");
        let state = str_of(c, "state");
        if matches!(conclusion, "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE")
            || matches!(state, "FAILURE" | "ERROR")
        {
            "fail"
        } else if (str_of(c, "__typename") == "CheckRun" && str_of(c, "status") != "COMPLETED")
            || matches!(state, "PENDING" | "EXPECTED")
        {
            "pending"
        } else {
            "pass"
        }
    };
    let all: Vec<&str> = checks.iter().map(outcome).collect();
    ["fail", "pending"].into_iter().find(|w| all.contains(w)).unwrap_or("pass").into()
}

fn to_pr(v: &Value) -> PullRequest {
    let state = match (str_of(v, "state"), v["isDraft"].as_bool() == Some(true)) {
        ("MERGED", _) => "merged",
        ("CLOSED", _) => "closed",
        (_, true) => "draft",
        _ => "open",
    };
    let review = match str_of(v, "reviewDecision") {
        "APPROVED" => "approved",
        "CHANGES_REQUESTED" => "changes",
        "REVIEW_REQUIRED" => "required",
        _ => "none",
    };
    PullRequest {
        number: v["number"].as_u64().unwrap_or(0),
        title: str_of(v, "title").into(),
        state: state.into(),
        branch: str_of(v, "headRefName").into(),
        base: str_of(v, "baseRefName").into(),
        url: str_of(v, "url").into(),
        author: str_of(&v["author"], "login").into(),
        updated_at: str_of(v, "updatedAt").into(),
        merged_at: v["mergedAt"].as_str().map(str::to_owned),
        additions: v["additions"].as_u64().unwrap_or(0),
        deletions: v["deletions"].as_u64().unwrap_or(0),
        review: review.into(),
        checks: summarize_checks(&v["statusCheckRollup"]),
    }
}

/// The 40 most recently updated PRs, open or not.
pub fn list(project_id: &str) -> Result<Vec<PullRequest>, String> {
    let project = store::project(project_id)?;
    if project.meta.github.is_empty() {
        return Err(format!("{} n'a pas de dépôt GitHub", project.meta.name));
    }
    let out = paths::run(Command::new("gh").args([
        "pr", "list", "--repo", &project.meta.github, "--state", "all", "--limit", "40", "--json", FIELDS,
    ]))
    .map_err(|e| format!("gh pr list : {e}"))?;
    let values: Vec<Value> = serde_json::from_str(&out).map_err(|e| e.to_string())?;
    Ok(values.iter().map(to_pr).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checks_fold_to_one_word() {
        assert_eq!(summarize_checks(&json!([])), "none");
        let pass = json!({"__typename": "CheckRun", "status": "COMPLETED", "conclusion": "SUCCESS"});
        let running = json!({"__typename": "CheckRun", "status": "IN_PROGRESS", "conclusion": ""});
        let failed = json!({"__typename": "StatusContext", "state": "FAILURE"});
        assert_eq!(summarize_checks(&json!([pass])), "pass");
        assert_eq!(summarize_checks(&json!([pass, running])), "pending");
        assert_eq!(summarize_checks(&json!([running, failed])), "fail");
    }
}
