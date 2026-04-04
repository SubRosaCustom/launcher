use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

const SETTINGS_FILE: &str = "launcher-settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    #[serde(default = "default_executable_name_string")]
    pub executable_name: String,
    #[serde(default)]
    pub close_on_launch: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            executable_name: default_executable_name().to_string(),
            close_on_launch: false,
        }
    }
}

pub fn default_executable_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "subrosa.x64.exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "subrosa.x64"
    }
}

fn default_executable_name_string() -> String {
    default_executable_name().to_string()
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("cannot resolve config dir: {e}"))?;
    Ok(dir.join(SETTINGS_FILE))
}

pub fn load_settings(app: &AppHandle) -> Result<LauncherSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(LauncherSettings::default());
    }
    let settings_json =
        fs::read_to_string(&path).map_err(|e| format!("cannot read settings: {e}"))?;
    serde_json::from_str(&settings_json).map_err(|e| format!("invalid settings: {e}"))
}

pub fn save_settings(app: &AppHandle, settings: &LauncherSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create config dir: {e}"))?;
    }
    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("cannot encode settings: {e}"))?;
    fs::write(&path, settings_json).map_err(|e| format!("cannot write settings: {e}"))
}
