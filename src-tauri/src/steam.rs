use serde::Serialize;
use std::path::Path;
use steamlocate::SteamDir;

const SUB_ROSA_APP_ID: u32 = 272_230;
#[cfg(target_os = "windows")]
const WINDOWS_EXECUTABLES: [&str; 4] = ["subrosa.exe", "subrosa.x64.exe", "subrosa.x64", "subrosa"];
#[cfg(not(target_os = "windows"))]
const POSIX_EXECUTABLES: [&str; 2] = ["subrosa.x64", "subrosa"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub steam_dir: Option<String>,
    pub game_dir: Option<String>,
    pub executable_candidates: Vec<String>,
}

pub fn detect_subrosa() -> DetectionResult {
    let (steam_dir, game_dir) = match SteamDir::locate() {
        Ok(steam_dir) => {
            let game_dir = steam_dir
                .find_app(SUB_ROSA_APP_ID)
                .ok()
                .flatten()
                .map(|(app, library)| library.resolve_app_dir(&app))
                .filter(|path| path.exists());
            (Some(steam_dir.path().to_path_buf()), game_dir)
        }
        Err(_) => (None, None),
    };

    DetectionResult {
        steam_dir: steam_dir.as_deref().map(path_to_string),
        game_dir: game_dir.as_deref().map(path_to_string),
        executable_candidates: executable_candidates(game_dir.as_deref()),
    }
}

pub fn executable_candidates(game_dir: Option<&Path>) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        collect_executable_candidates(game_dir, &WINDOWS_EXECUTABLES)
    }
    #[cfg(not(target_os = "windows"))]
    {
        collect_executable_candidates(game_dir, &POSIX_EXECUTABLES)
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn collect_executable_candidates(game_dir: Option<&Path>, candidates: &[&str]) -> Vec<String> {
    let Some(game_dir) = game_dir else {
        return candidates.iter().map(|name| (*name).to_string()).collect();
    };

    let existing: Vec<String> = candidates
        .iter()
        .filter(|name| game_dir.join(name).exists())
        .map(|name| (*name).to_string())
        .collect();

    if existing.is_empty() {
        candidates.iter().map(|name| (*name).to_string()).collect()
    } else {
        existing
    }
}
