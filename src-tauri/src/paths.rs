use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
}

/// `~/.gamestudio`: the synced data repo in `data/`, plus what only makes sense on this machine (sessions, socket,
/// worktrees, tmux.conf).
pub fn studio_dir() -> PathBuf {
    home().join(".gamestudio")
}

pub fn data_dir() -> PathBuf {
    studio_dir().join("data")
}

pub fn worktrees_dir() -> PathBuf {
    studio_dir().join("worktrees")
}

/// Expands a leading `~/` so synced files stay valid on every machine.
pub fn expand(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(path),
    }
}

/// The real path, symlinks resolved, so paths from git, tmux and the shell compare equal; unchanged if missing.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn now_iso() -> String {
    humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string()
}

/// `2026-10-28 22:32` in this machine's time zone, for stamps people read.
pub fn local_stamp() -> String {
    run(Command::new("date").arg("+%Y-%m-%d %H:%M"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| now_iso().replace('T', " ")[..16].to_string())
}

/// Apps opened from the Finder get launchd's bare PATH. Adopt the interactive login shell's PATH so git, gh, tmux,
/// claude and code resolve exactly like in the user's terminal.
pub fn adopt_login_path() {
    const MARK: &str = "__STUDIO_PATH__";
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let Ok(out) = Command::new(shell)
        .args(["-lic", &format!("printf '{MARK}%s' \"$PATH\"")])
        .stdin(Stdio::null())
        .output()
    else {
        return;
    };
    if let Some((_, path)) = String::from_utf8_lossy(&out.stdout).rsplit_once(MARK) {
        // SAFETY: called first thing in `run`, before any other thread exists.
        unsafe { std::env::set_var("PATH", path.trim()) };
    }
}

/// Runs a command to completion: its stdout on success, its stderr (or stdout) as the error otherwise.
pub fn run(cmd: &mut Command) -> Result<String, String> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{} n'a pas pu démarrer : {e}", cmd.get_program().to_string_lossy()))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { err })
    }
}

/// One word for `sh`, whatever `s` holds.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn git(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir);
    cmd
}

/// Writes through a temporary sibling then renames, so a reader (or a sync) never sees half a file.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("Chemin sans dossier parent")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Impossible de créer {} : {e}", dir.display()))?;
    let tmp = dir.join(format!(".{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("file")));
    std::fs::write(&tmp, contents)
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| format!("Impossible d'écrire {} : {e}", path.display()))
}

/// Opens a file in its default app (Aseprite, Preview, Blender…) or reveals it in the Finder.
pub fn open(path: &str, reveal: bool) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Err(format!("{path} n'existe plus"));
    }
    let mut cmd = Command::new("open");
    if reveal {
        cmd.arg("-R");
    }
    run(cmd.arg(path)).map(|_| ())
}

pub fn open_in_vscode(path: &str) -> Result<(), String> {
    if !Path::new(path).is_dir() {
        return Err(format!("{path} n'existe plus"));
    }
    run(Command::new("code").arg(path)).map(|_| ())
}
