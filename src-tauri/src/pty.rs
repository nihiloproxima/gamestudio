use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::tmux;

struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    client: Box<dyn Child + Send + Sync>,
}

/// Terminal views currently attached to tmux sessions, by view id.
#[derive(Default)]
pub struct Ptys {
    next: AtomicU32,
    open: Mutex<HashMap<u32, Pty>>,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

/// Attaches a terminal view to a tmux session. Closing the view only detaches: the session keeps running.
#[tauri::command]
pub fn pty_attach(
    ptys: State<'_, Ptys>,
    session: String,
    cols: u16,
    rows: u16,
    on_data: Channel<InvokeResponseBody>,
) -> Result<u32, String> {
    if !tmux::has_session(&session) {
        return Err(format!("La session {session} ne tourne plus"));
    }
    let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::new("tmux");
    // -u: xterm.js always speaks UTF-8, whatever the locale tmux would otherwise infer it from.
    cmd.args(["-u", "-L", tmux::SOCKET, "-f"]);
    cmd.arg(tmux::conf_path());
    cmd.args(["attach-session", "-t", &format!("={session}")]);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let client = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || on_data.send(InvokeResponseBody::Raw(buf[..n].to_vec())).is_err() {
                break;
            }
        }
    });

    let id = ptys.next.fetch_add(1, Ordering::Relaxed);
    ptys.open.lock().unwrap().insert(id, Pty { master: pair.master, writer, client });
    Ok(id)
}

#[tauri::command]
pub fn pty_write(ptys: State<'_, Ptys>, id: u32, data: String) -> Result<(), String> {
    let mut open = ptys.open.lock().unwrap();
    let pty = open.get_mut(&id).ok_or("Terminal fermé")?;
    pty.writer.write_all(data.as_bytes()).and_then(|_| pty.writer.flush()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pty_resize(ptys: State<'_, Ptys>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let open = ptys.open.lock().unwrap();
    let pty = open.get(&id).ok_or("Terminal fermé")?;
    pty.master.resize(size(cols, rows)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pty_detach(ptys: State<'_, Ptys>, id: u32) {
    if let Some(mut pty) = ptys.open.lock().unwrap().remove(&id) {
        let _ = pty.client.kill();
    }
}
