//! The media files of a project, from the asset folders it declares: what the game looks and sounds like, and what
//! background agents just generated.

use serde::Serialize;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::{store, worktrees};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub path: String,
    /// Relative to the repo.
    pub rel: String,
    /// `image`, `audio`, `model`, `source` or `font`.
    pub kind: &'static str,
    pub size: u64,
    /// Milliseconds since the epoch.
    pub modified: u64,
}

const LIMIT: usize = 8000;
const SKIP_DIRS: [&str; 4] = ["node_modules", "build", "dist", "__pycache__"];

fn kind_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "avif" => "image",
        "wav" | "ogg" | "mp3" | "flac" | "m4a" | "aac" => "audio",
        "glb" | "gltf" | "obj" | "fbx" | "blend" | "vox" => "model",
        "aseprite" | "ase" | "psd" | "kra" | "xcf" => "source",
        "ttf" | "otf" | "woff" | "woff2" => "font",
        _ => return None,
    })
}

pub fn list(project_id: &str) -> Result<Vec<Asset>, String> {
    let project = store::project(project_id)?;
    let repo = worktrees::repo_path(&project)?;
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = project.meta.assets.iter().map(|a| repo.join(a)).collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if !SKIP_DIRS.contains(&name.as_ref()) {
                    stack.push(path);
                }
                continue;
            }
            let Some(kind) = kind_of(&path) else { continue };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push(Asset {
                rel: path.strip_prefix(&repo).unwrap_or(&path).display().to_string(),
                path: path.display().to_string(),
                kind,
                size: meta.len(),
                modified,
            });
            if out.len() >= LIMIT {
                return Ok(out);
            }
        }
    }
    Ok(out)
}
