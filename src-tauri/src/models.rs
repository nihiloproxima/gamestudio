//! The models an agent can run and the efforts each one takes, as the agent's own CLI reports them, so the launch sheet
//! offers what is really there. Claude's CLI has no catalog to ask: its aliases and effort levels are fixed here.

use serde::Serialize;
use serde_json::Value;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{paths, sessions};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Given to the agent as `--model` (Claude) or `-m` (Codex).
    pub id: String,
    pub label: String,
    /// The efforts it takes, lowest first; empty when it has none to set.
    pub efforts: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub models: Vec<Model>,
    /// The efforts available when no model is picked: those of the agent's default model.
    pub efforts: Vec<String>,
}

type Slot = Mutex<Option<(Instant, Options)>>;
static CODEX: Slot = Mutex::new(None);
static OPENCODE: Slot = Mutex::new(None);
const TTL: Duration = Duration::from_secs(300);

/// What `agent` can run. The catalogs of Codex and OpenCode are read from their CLIs and kept for a few minutes.
pub fn options(agent: &str) -> Result<Options, String> {
    match agent {
        "claude" => Ok(claude()),
        "codex" => cached(&CODEX, read_codex),
        "opencode" => cached(&OPENCODE, read_opencode),
        _ => Ok(Options { models: Vec::new(), efforts: Vec::new() }),
    }
}

fn claude() -> Options {
    let efforts: Vec<String> = sessions::EFFORTS.iter().map(|e| e.to_string()).collect();
    let model = |id: &str, label: &str| Model { id: id.into(), label: label.into(), efforts: efforts.clone() };
    Options { models: vec![model("opus", "Opus"), model("sonnet", "Sonnet"), model("haiku", "Haiku")], efforts }
}

fn cached(slot: &Slot, read: fn() -> Result<Options, String>) -> Result<Options, String> {
    let hit = slot.lock().unwrap().as_ref().filter(|(at, _)| at.elapsed() < TTL).map(|(_, o)| o.clone());
    if let Some(options) = hit {
        return Ok(options);
    }
    let options = read()?;
    *slot.lock().unwrap() = Some((Instant::now(), options.clone()));
    Ok(options)
}

/// `opencode models` lists the models its providers offer as `provider/model`. Their names and efforts come from
/// OpenCode's own cache of the catalog; a model with only toggles or token budgets offers no effort.
fn read_opencode() -> Result<Options, String> {
    let out = paths::run(Command::new("opencode").arg("models")).map_err(|e| format!("OpenCode ne donne pas ses modèles : {e}"))?;
    let catalog = std::fs::read_to_string(paths::home().join(".cache/opencode/models.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null);
    Ok(parse_opencode(&out, &catalog))
}

fn parse_opencode(list: &str, catalog: &Value) -> Options {
    let models = list
        .lines()
        .map(str::trim)
        .filter_map(|id| {
            let (provider, model) = id.split_once('/')?;
            let info = &catalog[provider]["models"][model];
            let efforts = info["reasoning_options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|o| o["type"] == "effort")
                .flat_map(|o| o["values"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            let label = format!("{} ({provider})", info["name"].as_str().unwrap_or(model));
            Some(Model { id: id.into(), label, efforts })
        })
        .collect();
    Options { models, efforts: Vec::new() }
}

/// `codex debug models` prints the catalog as JSON: the models meant for the picker, with their reasoning levels.
fn read_codex() -> Result<Options, String> {
    let out = paths::run(Command::new("codex").args(["debug", "models"]))
        .map_err(|e| format!("Codex ne donne pas ses modèles : {e}"))?;
    parse_codex(&out, codex_default_model())
}

fn parse_codex(catalog: &str, configured: Option<String>) -> Result<Options, String> {
    let catalog: Value = serde_json::from_str(catalog).map_err(|e| format!("Catalogue de Codex illisible : {e}"))?;
    let models: Vec<Model> = catalog["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| {
            let id = m["slug"].as_str()?;
            let efforts = m["supported_reasoning_levels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| l["effort"].as_str())
                .map(str::to_owned)
                .collect();
            Some(Model { id: id.into(), label: m["display_name"].as_str().unwrap_or(id).into(), efforts })
        })
        .collect();
    let default = configured.filter(|d| models.iter().any(|m| &m.id == d)).or_else(|| models.first().map(|m| m.id.clone()));
    let efforts = models.iter().find(|m| Some(&m.id) == default.as_ref()).map(|m| m.efforts.clone()).unwrap_or_default();
    Ok(Options { models, efforts })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_lists_the_pickable_models_with_their_efforts() {
        let catalog = r#"{"models": [
            {"slug": "hidden", "display_name": "Hidden", "visibility": "hide", "supported_reasoning_levels": []},
            {"slug": "gpt-a", "display_name": "GPT A", "visibility": "list",
             "supported_reasoning_levels": [{"effort": "low"}, {"effort": "high"}]},
            {"slug": "gpt-b", "visibility": "list", "supported_reasoning_levels": [{"effort": "max"}]}
        ]}"#;
        let options = parse_codex(catalog, Some("gpt-b".into())).unwrap();
        let ids: Vec<&str> = options.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gpt-a", "gpt-b"]);
        assert_eq!(options.models[0].label, "GPT A");
        assert_eq!(options.models[1].label, "gpt-b");
        assert_eq!(options.efforts, ["max"]);
        let first = parse_codex(catalog, Some("unknown".into())).unwrap();
        assert_eq!(first.efforts, ["low", "high"]);
        assert!(parse_codex("not json", None).is_err());
    }

    #[test]
    fn opencode_names_its_models_and_their_efforts_from_its_catalog() {
        let catalog: Value = serde_json::from_str(
            r#"{"opencode-go": {"models": {
                "minimax-m3.1-flash": {"name": "MiniMax M3.1 Flash",
                    "reasoning_options": [{"type": "effort", "values": ["low", "high"]}]},
                "glm": {"name": "GLM", "reasoning_options": [{"type": "toggle"}]}
            }}}"#,
        )
        .unwrap();
        let options = parse_opencode("opencode-go/minimax-m3.1-flash\nopencode-go/glm\nlocal/unknown\n", &catalog);
        let ids: Vec<&str> = options.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["opencode-go/minimax-m3.1-flash", "opencode-go/glm", "local/unknown"]);
        assert_eq!(options.models[0].label, "MiniMax M3.1 Flash (opencode-go)");
        assert_eq!(options.models[0].efforts, ["low", "high"]);
        assert!(options.models[1].efforts.is_empty());
        assert_eq!(options.models[2].label, "unknown (local)");
        assert!(options.efforts.is_empty());
    }
}

/// Codex's own default: `model` in `~/.codex/config.toml`.
fn codex_default_model() -> Option<String> {
    let text = std::fs::read_to_string(paths::home().join(".codex/config.toml")).ok()?;
    let config: toml::Table = toml::from_str(&text).ok()?;
    config.get("model")?.as_str().map(str::to_owned)
}
