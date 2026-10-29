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

fn set_env(cmd: &mut Command, env: &[(&str, &str)]) {
    for (key, value) in env {
        cmd.arg("-e").arg(format!("{key}={value}"));
        // A new process takes PATH from the client starting it, whatever -e says.
        if *key == "PATH" {
            cmd.env("PATH", value);
        }
    }
}

/// Starts a detached session in `cwd`; without a command it runs the default shell.
pub fn new_session(name: &str, cwd: &Path, env: &[(&str, &str)], shell_command: Option<&str>) -> Result<(), String> {
    let mut cmd = command();
    cmd.args(["new-session", "-d", "-s", name, "-c"]).arg(cwd);
    set_env(&mut cmd, env);
    if let Some(c) = shell_command {
        cmd.arg(c);
    }
    paths::run(&mut cmd).map(|_| ()).map_err(|e| format!("tmux n'a pas pu créer la session {name} : {e}"))
}

/// Kills what runs in a live session and runs `shell_command` in its place: terminal views stay attached.
pub fn respawn(name: &str, cwd: &Path, env: &[(&str, &str)], shell_command: &str) -> Result<(), String> {
    let mut cmd = command();
    cmd.args(["respawn-pane", "-k", "-t", &format!("={name}:"), "-c"]).arg(cwd);
    set_env(&mut cmd, env);
    cmd.arg(shell_command);
    paths::run(&mut cmd).map(|_| ()).map_err(|e| format!("tmux n'a pas pu relancer la session {name} : {e}"))
}

pub fn kill_session(name: &str) -> Result<(), String> {
    paths::run(command().args(["kill-session", "-t", &format!("={name}")])).map(|_| ())
}

/// Types `text` into the session as one bracketed paste, then presses Enter: the agent gets it as one message,
/// newlines included.
pub fn paste(name: &str, text: &str) -> Result<(), String> {
    let target = format!("={name}:");
    let buffer = format!("studio-{name}");
    paths::run(command().args(["set-buffer", "-b", &buffer, "--", text]))?;
    paths::run(command().args(["paste-buffer", "-p", "-d", "-b", &buffer, "-t", &target]))?;
    // Lets the agent's input take the paste in before it is submitted.
    std::thread::sleep(std::time::Duration::from_millis(300));
    paths::run(command().args(["send-keys", "-t", &target, "Enter"])).map(|_| ())
}

/// The last `lines` lines of the session's screen and scrollback, as plain text.
pub fn capture(name: &str, lines: usize) -> Result<String, String> {
    let out = paths::run(command().args(["capture-pane", "-p", "-J", "-t", &format!("={name}:"), "-S", &format!("-{lines}")]))?;
    let all: Vec<&str> = out.trim_end().lines().collect();
    Ok(all[all.len().saturating_sub(lines)..].join("\n"))
}
