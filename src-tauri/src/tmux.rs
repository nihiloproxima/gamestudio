use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths;

pub const SOCKET: &str = "studio";
const CONF: &str = include_str!("tmux.conf");

pub fn conf_path() -> PathBuf {
    paths::studio_dir().join("tmux.conf")
}

/// Gamestudio's private tmux server: agents outlive the app, and `tmux -L studio attach` reaches them from any terminal.
pub fn command() -> Command {
    let mut cmd = Command::new("tmux");
    cmd.args(["-L", SOCKET, "-f"]).arg(conf_path());
    cmd
}

pub fn write_conf() -> Result<(), String> {
    let path = conf_path();
    if std::fs::read_to_string(&path).is_ok_and(|current| current == CONF) {
        return Ok(());
    }
    paths::write_atomic(&path, CONF)
}

/// Live session names. No server running simply means no session.
pub fn sessions() -> Vec<String> {
    paths::run(command().args(["list-sessions", "-F", "#{session_name}"]))
        .map(|out| out.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

pub fn has_session(name: &str) -> bool {
    sessions().iter().any(|s| s == name)
}

/// Starts a detached session in `cwd`; without a command it runs the default shell.
pub fn new_session(name: &str, cwd: &Path, env: &[(&str, &str)], shell_command: Option<&str>) -> Result<(), String> {
    let mut cmd = command();
    cmd.args(["new-session", "-d", "-s", name, "-c"]).arg(cwd);
    for (key, value) in env {
        cmd.arg("-e").arg(format!("{key}={value}"));
        // A new session takes PATH from the client creating it, whatever -e says.
        if *key == "PATH" {
            cmd.env("PATH", value);
        }
    }
    if let Some(c) = shell_command {
        cmd.arg(c);
    }
    paths::run(&mut cmd).map(|_| ()).map_err(|e| format!("tmux n'a pas pu créer la session {name} : {e}"))
}

pub fn kill_session(name: &str) -> Result<(), String> {
    paths::run(command().args(["kill-session", "-t", &format!("={name}")])).map(|_| ())
}
