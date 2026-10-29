//! How to run a game from a checkout of its repo: the command in its settings, or one read from the repo. Godot is
//! recognized; any other engine runs from the command in its settings.

use std::path::{Path, PathBuf};

use crate::{paths, store};

/// The shell command that runs the game, from `dir` (its main checkout or a worktree).
pub fn command(project: &store::Project, dir: &Path) -> Result<String, String> {
    let set = project.meta.run.trim();
    if !set.is_empty() {
        return Ok(set.to_string());
    }
    let Some(folder) = godot_project(dir) else {
        return Err(format!(
            "Gamestudio ne sait pas lancer {} : indique la commande qui le lance dans les réglages",
            project.meta.name
        ));
    };
    let godot = godot_binary(&folder, &app_folders())
        .ok_or("Godot est introuvable : installe-le dans Applications, ou indique la commande qui lance le jeu dans les réglages")?;
    let rel = folder.strip_prefix(dir).ok().filter(|r| !r.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Ok(format!("{} --path {}", paths::shell_quote(&godot), paths::shell_quote(&rel.display().to_string())))
}

/// The folder holding `project.godot`: the repo's root, or one of its folders (`game/`, `godot/`…).
fn godot_project(dir: &Path) -> Option<PathBuf> {
    if dir.join("project.godot").is_file() {
        return Some(dir.to_path_buf());
    }
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.join("project.godot").is_file()).collect();
    found.sort();
    found.into_iter().next()
}

fn app_folders() -> Vec<PathBuf> {
    let home = paths::home();
    vec![
        PathBuf::from("/Applications"),
        home.join("Applications"),
        home.join("Library/Application Support/Steam/steamapps/common/Godot Engine"),
    ]
}

/// `godot` on the PATH (Homebrew's cask), else the Godot app of an Applications folder: the .NET build for a C#
/// project, the newest name first.
fn godot_binary(project: &Path, folders: &[PathBuf]) -> Option<String> {
    let on_path = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("godot").is_file()));
    if on_path {
        return Some("godot".into());
    }
    let csharp = std::fs::read_dir(project)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x == "csproj"));
    let mut apps: Vec<(bool, String, PathBuf)> = folders
        .iter()
        .flat_map(|f| std::fs::read_dir(f).into_iter().flatten().flatten())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            (name.starts_with("godot") && name.ends_with(".app")).then(|| (name.contains("mono") == csharp, name, e.path()))
        })
        .collect();
    apps.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    apps.into_iter().find_map(|(_, _, app)| {
        let mut bins: Vec<PathBuf> = std::fs::read_dir(app.join("Contents/MacOS")).ok()?.flatten().map(|e| e.path()).collect();
        bins.sort();
        bins.into_iter().next().map(|b| b.display().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    #[test]
    fn godot_projects_at_the_root_or_one_folder_down() {
        let root = std::env::temp_dir().join(format!("studio-godot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        touch(&root.join("README.md"));
        assert_eq!(godot_project(&root), None);
        touch(&root.join("game/project.godot"));
        assert_eq!(godot_project(&root), Some(root.join("game")));
        touch(&root.join("project.godot"));
        assert_eq!(godot_project(&root), Some(root.clone()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_settings_command_wins() {
        let meta = store::ProjectMeta { name: "Frostholm".into(), run: " cargo run --release ".into(), ..Default::default() };
        let project = store::Project { id: "frostholm".into(), meta, vision: String::new(), present: true };
        assert_eq!(command(&project, Path::new("/nowhere")).unwrap(), "cargo run --release");
        let project = store::Project { meta: store::ProjectMeta { name: "Frostholm".into(), ..Default::default() }, ..project };
        assert!(command(&project, Path::new("/nowhere")).unwrap_err().contains("réglages"));
    }

    #[test]
    fn a_csharp_project_gets_the_dotnet_build() {
        let root = std::env::temp_dir().join(format!("studio-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let apps = root.join("Applications");
        touch(&apps.join("Godot.app/Contents/MacOS/Godot"));
        touch(&apps.join("Godot_mono.app/Contents/MacOS/Godot"));
        touch(&apps.join("Notes.app/Contents/MacOS/Notes"));
        let game = root.join("game");
        touch(&game.join("project.godot"));
        // Unless `godot` is on the PATH of the machine running the tests.
        if let Some(found) = godot_binary(&game, &[apps.clone()]).filter(|f| f != "godot") {
            assert_eq!(found, apps.join("Godot.app/Contents/MacOS/Godot").display().to_string());
            touch(&game.join("Game.csproj"));
            let found = godot_binary(&game, &[apps.clone()]).unwrap();
            assert_eq!(found, apps.join("Godot_mono.app/Contents/MacOS/Godot").display().to_string());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
