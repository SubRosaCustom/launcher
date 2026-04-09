use chrono::{DateTime, Utc};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};
use tauri::{AppHandle, Manager};

use crate::{settings, steam};

const LAUNCHER_LOG_FILE: &str = "launcher.log";
const CLIENT_CONFIG_DIR_NAME: &str = "Sub Rosa Custom";
const CLIENT_CRASHLOG_DIR_NAME: &str = "crashlogs";
const CLIENT_LOCAL_RUNTIME_ROOT_NAME: &str = "subrosacustom";
const CLIENT_SYNC_CACHE_ROOT_NAME: &str = "sync";
const CLIENT_TEXTURE_EXPORTS_DIR_NAME: &str = "texture_exports";

pub fn repo_cache_dir(app: &AppHandle, repo: &str) -> Result<PathBuf, String> {
    let cache_dir = cache_root_dir(app)?.join(sanitize_path_part(repo));
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("io_error: cannot create inject dir: {e}"))?;
    Ok(cache_dir)
}

pub fn open_launcher_logs(app: &AppHandle) -> Result<String, String> {
    let log_dir = ensure_log_dir(app)?;
    let log_file = log_dir.join(LAUNCHER_LOG_FILE);
    if !log_file.exists() {
        fs::write(&log_file, b"").map_err(|e| format!("io_error: cannot create log file: {e}"))?;
    }
    open_path_in_file_manager(&log_dir)?;
    Ok(log_dir.to_string_lossy().into_owned())
}

pub fn open_client_crashlogs_folder() -> Result<String, String> {
    let crashlog_dir = ensure_client_crashlog_dir()?;
    open_path_in_file_manager(&crashlog_dir)?;
    Ok(crashlog_dir.to_string_lossy().into_owned())
}

pub fn open_client_config_folder() -> Result<String, String> {
    let client_dir = ensure_client_config_dir()?;
    open_path_in_file_manager(&client_dir)?;
    Ok(client_dir.to_string_lossy().into_owned())
}

pub fn open_cache_folder(app: &AppHandle) -> Result<String, String> {
    let cache_root = ensure_cache_root_dir(app)?;
    open_path_in_file_manager(&cache_root)?;
    Ok(cache_root.to_string_lossy().into_owned())
}

pub fn force_redownload(app: &AppHandle, repo: &str) -> Result<String, String> {
    remove_dir_if_exists(&cache_root_dir(app)?.join(sanitize_path_part(repo)))?;
    remove_dir_if_exists(&legacy_cache_root_dir(app)?.join(sanitize_path_part(repo)))?;
    let cache_dir = repo_cache_dir(app, repo)?;
    Ok(cache_dir.to_string_lossy().into_owned())
}

pub fn clear_cache(app: &AppHandle) -> Result<String, String> {
    remove_dir_if_exists(&cache_root_dir(app)?)?;
    remove_dir_if_exists(&legacy_cache_root_dir(app)?)?;
    let cache_root = ensure_cache_root_dir(app)?;
    Ok(cache_root.to_string_lossy().into_owned())
}

pub fn append_launcher_log(app: &AppHandle, message: &str) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let log_path = launcher_log_path(app)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("io_error: cannot open log file: {e}"))?;

    writeln!(file, "[{}] {}", timestamp_rfc3339_now(), trimmed)
        .map_err(|e| format!("io_error: cannot write log file: {e}"))
}

pub fn collect_launcher_diagnostics(app: &AppHandle, repo: Option<&str>) -> Result<String, String> {
    let detection = steam::detect_subrosa();
    let settings_summary = match settings::load_settings(app) {
        Ok(settings) => format!(
            "settings.executableName={}\nsettings.closeOnLaunch={}",
            settings.executable_name, settings.close_on_launch
        ),
        Err(err) => format!("settings.error={err}"),
    };

    let cache_root = cache_root_dir(app)?;
    let legacy_cache_root = legacy_cache_root_dir(app)?;
    let repo_cache = repo.map(|value| cache_root.join(sanitize_path_part(value)));
    let log_path = launcher_log_path(app)?;
    let log_tail = read_log_tail(&log_path, 20)?;
    let cache_summary = summarize_dir(&cache_root)?;
    let legacy_cache_summary = summarize_dir(&legacy_cache_root)?;
    let repo_cache_summary = match &repo_cache {
        Some(path) => summarize_dir(path)?,
        None => "repo cache not configured".to_string(),
    };

    let mut diagnostics = vec![
        format!("timestamp={}", timestamp_rfc3339_now()),
        format!("launcher.version={}", env!("CARGO_PKG_VERSION")),
        format!("platform.os={}", std::env::consts::OS),
        format!("platform.arch={}", std::env::consts::ARCH),
        format!("steam.dir={}", option_line(&detection.steam_dir)),
        format!("game.dir={}", option_line(&detection.game_dir)),
        format!(
            "game.executables={}",
            detection.executable_candidates.join(", ")
        ),
        settings_summary,
        format!("log.file={}", log_path.to_string_lossy()),
        format!("cache.root={}", cache_root.to_string_lossy()),
        format!("cache.root.summary={cache_summary}"),
        format!("cache.legacyRoot={}", legacy_cache_root.to_string_lossy()),
        format!("cache.legacyRoot.summary={legacy_cache_summary}"),
    ];

    if let Some(path) = repo_cache {
        diagnostics.push(format!("cache.repo={}", path.to_string_lossy()));
        diagnostics.push(format!("cache.repo.summary={repo_cache_summary}"));
    } else {
        diagnostics.push("cache.repo=not configured".to_string());
    }

    diagnostics.push("log.tail.begin".to_string());
    if log_tail.is_empty() {
        diagnostics.push("(empty)".to_string());
    } else {
        diagnostics.extend(log_tail);
    }
    diagnostics.push("log.tail.end".to_string());

    Ok(diagnostics.join("\n"))
}

pub fn collect_client_diagnostics() -> Result<String, String> {
    let config_root = client_config_dir()?;
    let crashlog_root = config_root.join(CLIENT_CRASHLOG_DIR_NAME);
    let local_runtime_root = config_root.join(CLIENT_LOCAL_RUNTIME_ROOT_NAME);
    let local_scripts_root = local_runtime_root.join("scripts");
    let sync_cache_root = config_root.join(CLIENT_SYNC_CACHE_ROOT_NAME);
    let texture_exports_root = config_root.join(CLIENT_TEXTURE_EXPORTS_DIR_NAME);
    let latest_crashlog = latest_file_in_dir(&crashlog_root)?;
    let has_latest_crashlog = latest_crashlog.is_some();

    let mut diagnostics = vec![
        format!("timestamp={}", timestamp_rfc3339_now()),
        format!("platform.os={}", std::env::consts::OS),
        format!("platform.arch={}", std::env::consts::ARCH),
        format!("client.configRoot={}", config_root.to_string_lossy()),
        format!("client.configRoot.exists={}", config_root.exists()),
        format!("client.crashlogRoot={}", crashlog_root.to_string_lossy()),
        format!(
            "client.crashlogRoot.summary={}",
            summarize_dir(&crashlog_root)?
        ),
        "client.syncMode=in-memory scripts".to_string(),
        format!(
            "client.localRuntimeRoot={}",
            local_runtime_root.to_string_lossy()
        ),
        format!(
            "client.localRuntimeRoot.summary={}",
            summarize_dir(&local_runtime_root)?
        ),
        format!(
            "client.localScriptsRoot={}",
            local_scripts_root.to_string_lossy()
        ),
        format!(
            "client.localScriptsRoot.summary={}",
            summarize_dir(&local_scripts_root)?
        ),
        format!("client.syncCacheRoot={}", sync_cache_root.to_string_lossy()),
        format!(
            "client.syncCacheRoot.summary={}",
            summarize_dir(&sync_cache_root)?
        ),
        format!(
            "client.textureExportsRoot={}",
            texture_exports_root.to_string_lossy()
        ),
        format!(
            "client.textureExportsRoot.summary={}",
            summarize_dir(&texture_exports_root)?
        ),
    ];

    match latest_crashlog {
        Some(path) => {
            let metadata = fs::metadata(&path).map_err(|e| {
                format!(
                    "io_error: cannot read crashlog metadata {}: {e}",
                    path.display()
                )
            })?;
            let modified = metadata.modified().map(timestamp_rfc3339).map_err(|e| {
                format!(
                    "io_error: cannot read crashlog modified time {}: {e}",
                    path.display()
                )
            })?;
            let crashlog_contents = read_text_file(&path)?;

            diagnostics.push(format!("client.latestCrashlog={}", path.to_string_lossy()));
            diagnostics.push(format!("client.latestCrashlog.bytes={}", metadata.len()));
            diagnostics.push(format!("client.latestCrashlog.modified={modified}"));
            diagnostics.push("client.latestCrashlog.begin".to_string());
            if crashlog_contents.is_empty() {
                diagnostics.push("(empty)".to_string());
            } else {
                diagnostics.extend(crashlog_contents.lines().map(ToString::to_string));
            }
            diagnostics.push("client.latestCrashlog.end".to_string());
        }
        None => {
            diagnostics.push("client.latestCrashlog=missing".to_string());
        }
    }

    let status = if has_latest_crashlog {
        "client crash evidence found"
    } else if sync_cache_root.exists() {
        "client sync cache present"
    } else if local_runtime_root.exists() {
        "client local runtime present"
    } else {
        "no client evidence found"
    };
    diagnostics.push(format!("client.status={status}"));

    Ok(diagnostics.join("\n"))
}

pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return run_copy_command("cmd", &["/C", "clip"], text);
    }

    #[cfg(target_os = "macos")]
    {
        return run_copy_command("pbcopy", &[], text);
    }

    #[cfg(target_os = "linux")]
    {
        for (program, args) in [
            ("wl-copy", &[][..]),
            ("xclip", &["-selection", "clipboard"][..]),
            ("xsel", &["--clipboard", "--input"][..]),
        ] {
            if run_copy_command(program, args, text).is_ok() {
                return Ok(());
            }
        }

        return Err("clipboard_unavailable: install wl-copy, xclip, or xsel".into());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("clipboard_unsupported_platform".into())
    }
}

fn launcher_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(ensure_log_dir(app)?.join(LAUNCHER_LOG_FILE))
}

fn client_config_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(path) = env::var_os("APPDATA") {
            return Ok(PathBuf::from(path).join(CLIENT_CONFIG_DIR_NAME));
        }

        let home = env::var_os("USERPROFILE")
            .ok_or_else(|| "path_error: cannot resolve USERPROFILE".to_string())?;
        return Ok(PathBuf::from(home)
            .join("AppData")
            .join("Roaming")
            .join(CLIENT_CONFIG_DIR_NAME));
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
            let config_home = PathBuf::from(path);
            if !config_home.is_absolute() {
                return Err("path_error: XDG_CONFIG_HOME must be absolute".to_string());
            }
            return Ok(config_home.join(CLIENT_CONFIG_DIR_NAME));
        }

        let home =
            env::var_os("HOME").ok_or_else(|| "path_error: cannot resolve HOME".to_string())?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join(CLIENT_CONFIG_DIR_NAME))
    }
}

fn ensure_client_config_dir() -> Result<PathBuf, String> {
    let client_dir = client_config_dir()?;
    fs::create_dir_all(&client_dir)
        .map_err(|e| format!("io_error: cannot create client config dir: {e}"))?;
    Ok(client_dir)
}

fn ensure_client_crashlog_dir() -> Result<PathBuf, String> {
    let crashlog_dir = ensure_client_config_dir()?.join(CLIENT_CRASHLOG_DIR_NAME);
    fs::create_dir_all(&crashlog_dir)
        .map_err(|e| format!("io_error: cannot create client crashlog dir: {e}"))?;
    Ok(crashlog_dir)
}

fn ensure_log_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("path_error: cannot resolve log dir: {e}"))?;
    fs::create_dir_all(&log_dir).map_err(|e| format!("io_error: cannot create log dir: {e}"))?;
    Ok(log_dir)
}

fn cache_root_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|path| path.join("inject").join("github"))
        .map_err(|e| format!("path_error: cannot resolve cache dir: {e}"))
}

fn ensure_cache_root_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let cache_root = cache_root_dir(app)?;
    fs::create_dir_all(&cache_root)
        .map_err(|e| format!("io_error: cannot create cache dir: {e}"))?;
    Ok(cache_root)
}

fn legacy_cache_root_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("inject").join("github"))
        .map_err(|e| format!("path_error: cannot resolve legacy cache dir: {e}"))
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path)
        .map_err(|e| format!("io_error: cannot remove directory {}: {e}", path.display()))
}

fn summarize_dir(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok("missing".to_string());
    }

    let (files, bytes) = walk_dir(path)?;
    Ok(format!("files={files}, bytes={bytes}"))
}

fn walk_dir(path: &Path) -> Result<(u64, u64), String> {
    let mut files = 0_u64;
    let mut bytes = 0_u64;

    for entry in fs::read_dir(path)
        .map_err(|e| format!("io_error: cannot read directory {}: {e}", path.display()))?
    {
        let entry = entry.map_err(|e| format!("io_error: cannot read directory entry: {e}"))?;
        let entry_path = entry.path();
        let metadata = entry.metadata().map_err(|e| {
            format!(
                "io_error: cannot read metadata {}: {e}",
                entry_path.display()
            )
        })?;

        if metadata.is_dir() {
            let (nested_files, nested_bytes) = walk_dir(&entry_path)?;
            files += nested_files;
            bytes += nested_bytes;
            continue;
        }

        if metadata.is_file() {
            files += 1;
            bytes += metadata.len();
        }
    }

    Ok((files, bytes))
}

fn read_log_tail(path: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents =
        fs::read_to_string(path).map_err(|e| format!("io_error: cannot read log file: {e}"))?;
    let mut lines: Vec<String> = contents.lines().map(ToString::to_string).collect();
    if lines.len() > max_lines {
        lines.drain(..lines.len() - max_lines);
    }
    Ok(lines)
}

fn read_text_file(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok(String::new());
    }

    fs::read_to_string(path).map_err(|e| format!("io_error: cannot read log file: {e}"))
}

fn latest_file_in_dir(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let mut latest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(path)
        .map_err(|e| format!("io_error: cannot read directory {}: {e}", path.display()))?
    {
        let entry = entry.map_err(|e| format!("io_error: cannot read directory entry: {e}"))?;
        let entry_path = entry.path();
        let metadata = entry.metadata().map_err(|e| {
            format!(
                "io_error: cannot read metadata {}: {e}",
                entry_path.display()
            )
        })?;

        if !metadata.is_file() {
            continue;
        }

        let modified = metadata.modified().map_err(|e| {
            format!(
                "io_error: cannot read modified time {}: {e}",
                entry_path.display()
            )
        })?;

        match &latest {
            Some((current, _)) if modified <= *current => {}
            _ => latest = Some((modified, entry_path)),
        }
    }

    Ok(latest.map(|(_, path)| path))
}

fn option_line(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("missing")
}

pub fn sanitize_path_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn open_path_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = path;
        return Err("open_path_unsupported_platform".into());
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        command.arg(path);
        command
            .spawn()
            .map_err(|e| format!("io_error: cannot open path {}: {e}", path.display()))?;
        Ok(())
    }
}

fn run_copy_command(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("clipboard_spawn_failed: {program}: {e}"))?;

    let Some(mut stdin) = child.stdin.take() else {
        return Err(format!("clipboard_stdin_unavailable: {program}"));
    };

    stdin
        .write_all(text.as_bytes())
        .map_err(|e| format!("clipboard_write_failed: {program}: {e}"))?;
    drop(stdin);

    let status = child
        .wait()
        .map_err(|e| format!("clipboard_wait_failed: {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("clipboard_command_failed: {program}"))
    }
}

fn timestamp_rfc3339_now() -> String {
    timestamp_rfc3339(SystemTime::now())
}

fn timestamp_rfc3339(time: SystemTime) -> String {
    let timestamp: DateTime<Utc> = time.into();
    timestamp.to_rfc3339()
}
