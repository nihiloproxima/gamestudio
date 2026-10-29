//! `studio <command>`: reads and updates the tracker of the running app over its socket, for people in a terminal
//! and for the agents it launches. `studio guide setup` is how an agent learns to set a game up.

use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::control::{socket_path, Context, Request};

const HELP: &str = "\
studio : le tracker de Gamestudio depuis un terminal (l'app doit être ouverte)

Éléments (tâches, bugs, idées)
  studio items [--all] [--status <statut>]   ce qui est ouvert dans le projet (--all : terminé compris)
  studio item <ID>                           la fiche complète, JEU-12 par exemple
  studio add <titre> [--kind task|bug|idea] [--status <s>] [--priority <p>] [--milestone <jalon>]
                     [--labels a,b] [--body <texte>]
                                             crée un élément ; une tâche va dans À faire, un bug ou une idée à trier
  studio idea <titre> [--body <texte>]       note une idée à trier plus tard
  studio bug <titre> [--body <texte>]        signale un bug à trier
  studio set <ID> [--title …] [--kind …] [--status …] [--priority …] [--milestone …] [--labels a,b] [--body …]
                                             change des champs (--milestone \"\" détache du jalon)
  studio status <ID> <statut>                inbox, backlog, todo, doing, review, done ou dropped
  studio note <ID> <texte>                   ajoute une note datée à la fiche
  studio open <ID>                           affiche la fiche dans l'app

Roadmap et projet
  studio milestones                          les jalons, dans l'ordre
  studio milestone add <titre> [--status planned|active|done] [--target AAAA-MM-JJ] [--goal <texte>]
  studio milestone set <id> [--title …] [--status …] [--target …] [--goal …] [--order <n>]
  studio project                             les réglages du projet
  studio project set [--name …] [--color …] [--path …] [--github …] [--base …] [--assets a,b] [--setup …]
                     [--run <commande qui lance le jeu, lue dans le dépôt si vide>] [--vision <texte>]
  studio project add [--path <dépôt>] [--name <nom> --key <CLÉ>] [mêmes options]
                                             suit le jeu du dépôt (par défaut celui du dossier courant) ; ses
                                             données reviennent si le dépôt a déjà une branche gamestudio, sinon
                                             il faut un nom et une clé
  studio projects                            les jeux suivis
  studio import <plan.json|-> [--dry-run]    importe tout un plan (jalons, éléments, réglages) ; --dry-run montre
                                             ce qui serait créé sans rien écrire. Format : studio guide setup

Agents
  studio start [<ID>] [--agent claude|codex|shell] [--branch <nom> | --here] [--prompt <texte>]
                                             lance un agent : nouveau worktree (--branch), dossier courant (--here)
                                             ou, par défaut, dépôt principal
  studio sessions                            les sessions du projet et ce qu'elles font
  studio wait [<session>…] [--timeout <s>]   attend qu'une de ces sessions finisse son tour, attende ou s'arrête
                                             (par défaut : les agents que tu as lancés ; 540 s au plus)
  studio peek <session> [--lines <n>]        la fin de son écran (60 lignes par défaut)
  studio send <session> <texte>              lui écrit, comme au clavier, et valide
  studio stop <session>                      l'arrête
  studio sync                                synchronise les données avec GitHub maintenant
  studio guide setup|orchestrate             la marche à suivre (pour un agent) : configurer un jeu, ou orchestrer
                                             des agents sur sa roadmap

  Le projet se déduit du dossier courant (dépôt ou worktree) ; sinon --project <nom|clé>.
  --json donne la réponse brute. Un texte « - » (--body -, --goal -, --vision -, send … -) se lit sur l'entrée
  standard. Un agent ne pilote (send, stop) que les sessions qu'il a lancées.
";

const SETUP_GUIDE: &str = include_str!("guide_setup.md");
const ORCHESTRATE_GUIDE: &str = include_str!("guide_orchestrate.md");

const SWITCHES: [&str; 6] = ["--all", "--json", "--here", "--dry-run", "--help", "-h"];

struct Args {
    positional: Vec<String>,
    values: HashMap<String, String>,
    switches: HashSet<String>,
}

impl Args {
    fn value(&self, flag: &str) -> Option<String> {
        self.values.get(flag).cloned()
    }

    fn on(&self, flag: &str) -> bool {
        self.switches.contains(flag)
    }

    /// A long text flag; `-` reads it from stdin.
    fn text(&self, flag: &str) -> Result<Option<String>, String> {
        match self.values.get(flag).map(String::as_str) {
            Some("-") => {
                let mut text = String::new();
                std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
                Ok(Some(text))
            }
            other => Ok(other.map(str::to_owned)),
        }
    }

    fn list(&self, flag: &str) -> Option<Vec<String>> {
        self.values.get(flag).map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
    }

    /// The words after the command, as one text.
    fn rest(&self, from: usize, what: &str) -> Result<String, String> {
        let words = self.positional.get(from..).unwrap_or_default().join(" ");
        if words.trim().is_empty() { Err(format!("Il manque {what}")) } else { Ok(words) }
    }

    fn pos(&self, i: usize, what: &str) -> Result<String, String> {
        self.positional.get(i).cloned().ok_or_else(|| format!("Il manque {what}"))
    }
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args { positional: Vec::new(), values: HashMap::new(), switches: HashSet::new() };
    let mut it = raw.iter();
    while let Some(arg) = it.next() {
        if SWITCHES.contains(&arg.as_str()) {
            args.switches.insert(arg.clone());
        } else if arg.starts_with("--") {
            let value = it.next().ok_or_else(|| format!("{arg} attend une valeur"))?;
            args.values.insert(arg.clone(), value.clone());
        } else {
            args.positional.push(arg.clone());
        }
    }
    Ok(args)
}

/// The agent session running this command, if any. A person in one of the app's terminals counts as no one.
fn caller() -> Option<String> {
    let person = std::env::var("STUDIO_AGENT").is_ok_and(|a| a == "shell" || a == "game");
    std::env::var("STUDIO_SESSION").ok().filter(|s| !s.is_empty() && !person)
}

fn context(args: &Args) -> Context {
    Context {
        project: args.value("--project").or_else(|| std::env::var("STUDIO_PROJECT").ok().filter(|p| !p.is_empty())),
        cwd: std::env::current_dir().ok().map(|p| p.display().to_string()),
    }
}

/// The given flags as a JSON object, under their names without dashes.
fn fields(args: &Args, texts: &[&str], lists: &[&str]) -> Result<Map<String, Value>, String> {
    let mut map = Map::new();
    for flag in texts {
        if let Some(v) = args.text(flag)? {
            map.insert(flag.trim_start_matches("--").to_string(), json!(v));
        }
    }
    for flag in lists {
        if let Some(v) = args.list(flag) {
            map.insert(flag.trim_start_matches("--").to_string(), json!(v));
        }
    }
    Ok(map)
}

const ITEM_TEXTS: [&str; 6] = ["--title", "--kind", "--status", "--priority", "--milestone", "--body"];
const PROJECT_TEXTS: [&str; 9] = ["--name", "--key", "--color", "--path", "--github", "--base", "--setup", "--run", "--vision"];

fn request(args: &Args) -> Result<Option<Request>, String> {
    let Some(cmd) = args.positional.first() else {
        return Ok(None);
    };
    if args.on("--help") || args.on("-h") {
        return Ok(None);
    }
    let add = |kind: &str| -> Result<Request, String> {
        let mut draft = fields(args, &ITEM_TEXTS, &["--labels"])?;
        draft.insert("title".into(), json!(args.rest(1, "le titre")?));
        draft.entry("kind").or_insert(json!(kind));
        Ok(Request::Add { ctx: context(args), draft: Value::Object(draft) })
    };
    let sub = args.positional.get(1).map(String::as_str);
    Ok(Some(match (cmd.as_str(), sub) {
        ("help", _) => return Ok(None),
        ("projects", _) => Request::Projects,
        ("items", _) => Request::Items { ctx: context(args), status: args.value("--status"), all: args.on("--all") },
        ("item" | "show", _) => Request::Item { id: args.pos(1, "l'identifiant, JEU-12 par exemple")? },
        ("add", _) => add("task")?,
        ("idea", _) => add("idea")?,
        ("bug", _) => add("bug")?,
        ("set", _) => {
            let patch = fields(args, &ITEM_TEXTS, &["--labels"])?;
            if patch.is_empty() {
                return Err("Rien à changer : précise au moins un champ (--status, --title…)".into());
            }
            Request::Update { id: args.pos(1, "l'identifiant")?, patch: Value::Object(patch) }
        }
        ("status", _) => Request::Update {
            id: args.pos(1, "l'identifiant")?,
            patch: json!({ "status": args.pos(2, "le statut")? }),
        },
        ("note", _) => Request::Note {
            id: args.pos(1, "l'identifiant")?,
            text: args.rest(2, "le texte de la note")?,
            author: std::env::var("STUDIO_AGENT").unwrap_or_else(|_| "terminal".into()),
        },
        ("open", _) => Request::Open { id: args.pos(1, "l'identifiant")? },
        ("milestones", _) => Request::Milestones { ctx: context(args) },
        ("milestone", Some("add")) => {
            let mut f = fields(args, &["--status", "--target", "--goal"], &[])?;
            f.insert("title".into(), json!(args.rest(2, "le titre du jalon")?));
            Request::MilestoneSave { ctx: context(args), id: None, fields: Value::Object(f) }
        }
        ("milestone", Some("set")) => {
            let mut f = fields(args, &["--title", "--status", "--target", "--goal"], &[])?;
            if let Some(order) = args.value("--order") {
                f.insert("order".into(), json!(order.parse::<i64>().map_err(|_| "--order attend un nombre")?));
            }
            Request::MilestoneSave { ctx: context(args), id: Some(args.pos(2, "l'id du jalon")?), fields: Value::Object(f) }
        }
        ("milestone", _) => return Err("studio milestone add <titre> ou studio milestone set <id>".into()),
        ("project", None) => Request::Project { ctx: context(args) },
        ("project", Some("set")) => Request::ProjectSet {
            ctx: context(args),
            patch: Value::Object(fields(args, &PROJECT_TEXTS, &["--assets"])?),
        },
        ("project", Some("add")) => {
            Request::ProjectAdd { ctx: context(args), patch: Value::Object(fields(args, &PROJECT_TEXTS, &["--assets"])?) }
        }
        ("project", Some(other)) => return Err(format!("studio project {other} n'existe pas (set ou add)")),
        ("import", _) => {
            let source = args.pos(1, "le fichier du plan (ou - pour l'entrée standard)")?;
            let text = if source == "-" {
                let mut text = String::new();
                std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
                text
            } else {
                std::fs::read_to_string(&source).map_err(|e| format!("{source} illisible : {e}"))?
            };
            let plan: Value = serde_json::from_str(&text).map_err(|e| format!("{source} n'est pas du JSON valide : {e}"))?;
            Request::Import { ctx: context(args), plan, dry_run: args.on("--dry-run") }
        }
        ("sessions", _) => Request::Sessions { ctx: context(args) },
        ("start", _) => {
            let branch = args.value("--branch");
            let place = if branch.is_some() { "worktree" } else if args.on("--here") { "here" } else { "main" };
            Request::Start {
                ctx: context(args),
                item: args.positional.get(1).cloned(),
                agent: args.value("--agent").unwrap_or_else(|| "claude".into()),
                place: place.into(),
                branch,
                prompt: args.text("--prompt")?.unwrap_or_default(),
                caller: caller(),
            }
        }
        ("send", _) => {
            let text = match args.rest(2, "le texte à envoyer")? {
                dash if dash == "-" => {
                    let mut text = String::new();
                    std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
                    text
                }
                words => words,
            };
            Request::Send { caller: caller(), session: args.pos(1, "la session")?, text }
        }
        ("peek", _) => Request::Peek {
            session: args.pos(1, "la session")?,
            lines: args.value("--lines").map(|n| n.parse().map_err(|_| "--lines attend un nombre")).transpose()?.unwrap_or(60),
        },
        ("stop", _) => Request::Stop { caller: caller(), session: args.pos(1, "la session")? },
        ("sync", _) => Request::Sync,
        (other, _) => return Err(format!("Commande inconnue : {other}")),
    }))
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn print_item_line(i: &Value) {
    let milestone = i.get("milestone").and_then(Value::as_str).unwrap_or("");
    println!(
        "{:<9} {:<8} {:<7} {:<5} {}{}",
        text(i, "id"),
        text(i, "status"),
        text(i, "priority"),
        text(i, "kind"),
        text(i, "title"),
        if milestone.is_empty() { String::new() } else { format!("  [{milestone}]") }
    );
}

fn print_session_line(s: &Value) {
    let item = s.get("item").and_then(Value::as_str).unwrap_or("-");
    println!("{:<24} {:<8} {:<7} {:<9} {}", text(s, "id"), text(s, "status"), text(s, "agent"), item, text(s, "cwd"));
    // What it asks while waiting, or what it said last at the end of its turn.
    if matches!(text(s, "status"), "waiting" | "idle") && !text(s, "message").is_empty() {
        for line in text(s, "message").lines().filter(|l| !l.trim().is_empty()) {
            println!("{:<24} {line}", "");
        }
    }
}

fn print_names(label: &str, list: &Value) {
    let names: Vec<&str> = list.as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if names.is_empty() {
        return;
    }
    println!("{label} ({}) :", names.len());
    for n in names {
        println!("  {n}");
    }
}

fn print_result(cmd: &str, v: &Value) {
    match cmd {
        "projects" => {
            for p in v.as_array().into_iter().flatten() {
                let here = if p["present"].as_bool() == Some(true) { "" } else { "  (pas cloné ici)" };
                println!("{:<12} {:<5} {}{here}", text(p, "id"), text(p, "key"), text(p, "path"));
            }
        }
        "project" => {
            println!("{} ({}, clé {})", text(v, "name"), text(v, "id"), text(v, "key"));
            for (label, key) in [("dépôt", "path"), ("github", "github"), ("base", "base"), ("installation", "setup"), ("lancement", "run"), ("couleur", "color")] {
                println!("  {label:<13} {}", text(v, key));
            }
            let assets: Vec<&str> = v["assets"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            println!("  {:<13} {}", "assets", assets.join(", "));
            let vision = text(v, "vision");
            if !vision.is_empty() {
                println!("\n{vision}");
            }
        }
        "items" => {
            let items = v.as_array().map(Vec::as_slice).unwrap_or_default();
            if items.is_empty() {
                println!("Rien d'ouvert.");
            }
            items.iter().for_each(print_item_line);
        }
        "item" | "show" => {
            println!("{}  {}", text(v, "id"), text(v, "title"));
            println!("  type      {}", text(v, "kind"));
            println!("  statut    {}", text(v, "status"));
            println!("  priorité  {}", text(v, "priority"));
            if let Some(m) = v.get("milestone").and_then(Value::as_str) {
                println!("  jalon     {m}");
            }
            let labels: Vec<&str> = v["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            if !labels.is_empty() {
                println!("  labels    {}", labels.join(", "));
            }
            let body = text(v, "body");
            if !body.is_empty() {
                println!("\n{body}");
            }
        }
        "add" | "idea" | "bug" => println!("{} créé : {}", text(v, "id"), text(v, "title")),
        "set" | "status" => print_item_line(v),
        "note" => println!("Note ajoutée à {}", text(v, "id")),
        "milestones" => {
            let list = v.as_array().map(Vec::as_slice).unwrap_or_default();
            if list.is_empty() {
                println!("Aucun jalon.");
            }
            for m in list {
                let target = m.get("target").and_then(Value::as_str).unwrap_or("");
                println!("{:<32} {:<8} {:<11} {}", text(m, "id"), text(m, "status"), target, text(m, "title"));
            }
        }
        "milestone" => println!("Jalon {} : {} ({})", text(v, "id"), text(v, "title"), text(v, "status")),
        "import" => {
            let dry = v["dryRun"].as_bool() == Some(true);
            println!("{}", if dry { "Aperçu, rien n'a été écrit :" } else { "Import terminé :" });
            if v["projectUpdated"].as_bool() == Some(true) {
                println!("Réglages du projet {}.", if dry { "à mettre à jour" } else { "mis à jour" });
            }
            print_names(if dry { "Jalons à créer" } else { "Jalons créés" }, &v["milestonesCreated"]);
            print_names("Jalons déjà là", &v["milestonesExisting"]);
            print_names(if dry { "Éléments à créer" } else { "Éléments créés" }, &v["itemsCreated"]);
            print_names("Éléments déjà là (même titre), ignorés", &v["itemsExisting"]);
            let counts: Vec<String> = v["byStatus"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|pair| format!("{} {}", pair[1], pair[0].as_str().unwrap_or("")))
                .collect();
            if !counts.is_empty() {
                println!("Par statut : {}", counts.join(", "));
            }
        }
        "start" => println!("Session {} lancée dans {} (tmux -L studio attach -t {})", text(v, "id"), text(v, "cwd"), text(v, "id")),
        "sessions" => {
            let list = v.as_array().map(Vec::as_slice).unwrap_or_default();
            if list.is_empty() {
                println!("Aucune session.");
            }
            list.iter().for_each(print_session_line);
        }
        "send" => println!("Envoyé à {}", v.as_str().unwrap_or("")),
        "peek" => println!("{}", v.as_str().unwrap_or("")),
        "stop" => println!("Session {} arrêtée", v.as_str().unwrap_or("")),
        "sync" => match v.get("error").and_then(Value::as_str) {
            Some(e) => println!("Synchro en erreur : {e}"),
            None => {
                let remotes = v["remotes"].as_object().map(|m| m.len()).unwrap_or(0);
                println!("Données synchronisées ({remotes} jeu{} relié{} à son dépôt).", if remotes > 1 { "x" } else { "" }, if remotes > 1 { "s" } else { "" });
            }
        },
        _ => {}
    }
}

fn send(req: &Request) -> Result<Value, String> {
    let mut stream =
        UnixStream::connect(socket_path()).map_err(|_| "Gamestudio n'est pas ouverte : lance l'app, puis relance la commande.")?;
    let line = serde_json::to_string(req).expect("requests serialize");
    writeln!(stream, "{line}").map_err(|_| "Gamestudio n'a pas accepté la commande.")?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_str(&reply).map_err(|_| "Gamestudio a fermé la connexion sans répondre.")?;
    match (reply.get("ok"), reply.get("error")) {
        (Some(v), _) => Ok(v.clone()),
        (_, Some(e)) => Err(e.as_str().unwrap_or("Erreur inconnue").to_string()),
        _ => Err("Réponse illisible".into()),
    }
}

/// `studio hook <event>`, run by an agent's hooks: silent and always successful, so a closed app never gets in
/// the agent's way. Claude passes its payload on stdin, Codex as the last argument.
fn hook(raw: &[String]) -> i32 {
    let Ok(session) = std::env::var("STUDIO_SESSION") else { return 0 };
    let event = raw.get(1).cloned().unwrap_or_default();
    let payload = match raw.get(2) {
        Some(arg) => serde_json::from_str(arg).unwrap_or(Value::Null),
        None => {
            let mut text = String::new();
            let _ = std::io::stdin().read_to_string(&mut text);
            serde_json::from_str(&text).unwrap_or(Value::Null)
        }
    };
    let Ok(mut stream) = UnixStream::connect(socket_path()) else { return 0 };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let req = Request::Hook { session, event, payload };
    if writeln!(stream, "{}", serde_json::to_string(&req).unwrap_or_default()).is_ok() {
        let _ = BufReader::new(stream).read_line(&mut String::new());
    }
    0
}

/// The sessions `studio wait` watches by default: the working ones `me` launched.
fn working_children(list: &[Value], me: &str) -> Vec<String> {
    list.iter()
        .filter(|s| text(s, "parent") == me && text(s, "status") == "working")
        .map(|s| text(s, "id").to_string())
        .collect()
}

/// Each watched id with its session, `None` once forgotten; settled when one of them no longer works.
fn watch<'a>(list: &'a [Value], ids: &'a [String]) -> (Vec<(&'a String, Option<&'a Value>)>, bool) {
    let watched: Vec<_> = ids.iter().map(|id| (id, list.iter().find(|s| text(s, "id") == id))).collect();
    let settled = watched.iter().any(|(_, s)| s.is_none_or(|s| text(s, "status") != "working"));
    (watched, settled)
}

/// `studio wait`: polls until one of the watched sessions stops working (end of its turn, a question, a stop) or
/// the timeout, then prints them. With no ids, it watches the working sessions the caller launched.
fn wait(args: &Args) -> Result<(), String> {
    let timeout = match args.value("--timeout") {
        Some(t) => t.parse::<u64>().map_err(|_| "--timeout attend un nombre de secondes")?,
        None => 540,
    };
    let req = Request::Sessions { ctx: context(args) };
    let fetch = || send(&req).map(|v| v.as_array().cloned().unwrap_or_default());
    let mut list = fetch()?;
    let ids: Vec<String> = match args.positional.get(1..).filter(|ids| !ids.is_empty()) {
        Some(ids) => ids.to_vec(),
        None => {
            working_children(&list, &caller().ok_or("Précise les sessions à attendre : studio wait <session>…")?)
        }
    };
    if ids.is_empty() {
        println!("Aucun de tes agents ne travaille.");
        return Ok(());
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout);
    loop {
        let (watched, settled) = watch(&list, &ids);
        let late = std::time::Instant::now() >= deadline;
        if settled || late {
            if !settled {
                println!("Toujours au travail au bout de {timeout} s :");
            }
            for (id, s) in watched {
                match s {
                    Some(s) => print_session_line(s),
                    None => println!("{id:<24} n'existe plus"),
                }
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(3));
        list = fetch()?;
    }
}

pub fn main(raw: Vec<String>) -> i32 {
    match raw.first().map(String::as_str) {
        Some("hook") => return hook(&raw),
        // Needs no running app: an agent reads it before anything else.
        Some("guide") => {
            match raw.get(1).map(String::as_str) {
                Some("setup") => print!("{SETUP_GUIDE}"),
                Some("orchestrate") => print!("{ORCHESTRATE_GUIDE}"),
                _ => {
                    eprintln!("studio guide setup : configurer un jeu\nstudio guide orchestrate : orchestrer des agents");
                    return 2;
                }
            }
            return 0;
        }
        _ => {}
    }
    let args = match parse_args(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\nstudio help pour la liste des commandes.");
            return 2;
        }
    };
    if args.positional.first().is_some_and(|c| c == "wait") && !args.on("--help") && !args.on("-h") {
        return match wait(&args) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                if e.starts_with("Gamestudio n'est pas ouverte") { 3 } else { 1 }
            }
        };
    }
    let req = match request(&args) {
        Ok(Some(r)) => r,
        Ok(None) => {
            print!("{HELP}");
            return 0;
        }
        Err(e) => {
            eprintln!("{e}\n\nstudio help pour la liste des commandes.");
            return 2;
        }
    };
    match send(&req) {
        Ok(v) if args.on("--json") => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Ok(v) => {
            print_result(&args.positional[0], &v);
            0
        }
        Err(e) => {
            eprintln!("{e}");
            if e.starts_with("Gamestudio n'est pas ouverte") { 3 } else { 1 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Args {
        parse_args(&list.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    fn req(list: &[&str]) -> Value {
        serde_json::to_value(request(&args(list)).unwrap().unwrap()).unwrap()
    }

    #[test]
    fn titles_join_words() {
        let v = req(&["idea", "un", "pêcheur", "--project", "ember", "--labels", "faune, nuit"]);
        assert_eq!(v["cmd"], "add");
        assert_eq!(v["draft"]["kind"], "idea");
        assert_eq!(v["draft"]["title"], "un pêcheur");
        assert_eq!(v["draft"]["labels"], json!(["faune", "nuit"]));
        assert_eq!(v["ctx"]["project"], "ember");
    }

    #[test]
    fn set_sends_only_given_fields() {
        let v = req(&["set", "JEU-3", "--status", "doing", "--milestone", ""]);
        assert_eq!(v["cmd"], "update");
        assert_eq!(v["patch"], json!({ "status": "doing", "milestone": "" }));
    }

    #[test]
    fn milestone_and_project_subcommands() {
        let v = req(&["milestone", "add", "Démo", "Steam", "--target", "2026-12-01"]);
        assert_eq!(v["cmd"], "milestone-save");
        assert_eq!(v["fields"]["title"], "Démo Steam");
        let v = req(&["project", "set", "--assets", "assets,content"]);
        assert_eq!(v["patch"]["assets"], json!(["assets", "content"]));
        let v = req(&["project", "add", "--name", "Mon jeu", "--key", "JEU"]);
        assert_eq!(v["cmd"], "project-add");
        assert_eq!(v["patch"]["key"], "JEU");
    }

    #[test]
    fn driving_sessions() {
        let v = req(&["send", "fro-1-claude", "Continue,", "c'est", "validé"]);
        assert_eq!(v["cmd"], "send");
        assert_eq!(v["session"], "fro-1-claude");
        assert_eq!(v["text"], "Continue, c'est validé");
        assert_eq!(req(&["peek", "fro-1-claude"])["lines"], 60);
        assert_eq!(req(&["peek", "fro-1-claude", "--lines", "20"])["lines"], 20);
        assert_eq!(req(&["stop", "fro-1-claude"])["cmd"], "stop");
        assert!(request(&args(&["send", "fro-1-claude"])).is_err());
        assert!(request(&args(&["peek", "fro-1-claude", "--lines", "beaucoup"])).is_err());
    }

    #[test]
    fn wait_watches_the_callers_working_agents() {
        let session = |id: &str, parent: Option<&str>, status: &str| json!({ "id": id, "parent": parent, "status": status });
        let mut list = vec![
            session("fro-1-claude", Some("orch"), "working"),
            session("fro-2-claude", Some("orch"), "idle"),
            session("fro-3-claude", None, "working"),
        ];
        let ids = working_children(&list, "orch");
        assert_eq!(ids, ["fro-1-claude"]);
        assert!(!watch(&list, &ids).1);
        list[0]["status"] = json!("waiting");
        assert!(watch(&list, &ids).1);
        // A session stopped and forgotten meanwhile settles the wait too.
        assert!(watch(&list[1..], &ids).1);
        assert!(working_children(&list, "someone-else").is_empty());
    }

    #[test]
    fn missing_values_are_errors() {
        assert!(request(&args(&["status", "JEU-1"])).is_err());
        assert!(request(&args(&["set", "JEU-1"])).is_err());
        assert!(parse_args(&["--kind".to_string()]).is_err());
    }
}
