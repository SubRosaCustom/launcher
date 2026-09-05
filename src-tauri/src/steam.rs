use serde::Serialize;
use std::path::Path;
use steamlocate::SteamDir;

const SUB_ROSA_APP_ID: u32 = 272_230;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub steam_dir: Option<String>,
    pub game_dir: Option<String>,
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
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
