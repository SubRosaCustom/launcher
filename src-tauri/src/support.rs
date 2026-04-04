use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

use crate::{settings, steam};

const LAUNCHER_LOG_FILE: &str = "launcher.log";

pub fn repo_cache_dir(app: &AppHandle, repo: &str) -> Result<PathBuf, String> {
    let cache_dir = cache_root_dir(app)?.join(sanitize_path_part(repo));
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("io_error: cannot create inject dir: {e}"))?;
    Ok(cache_dir)
}

pub fn open_logs(app: &AppHandle) -> Result<String, String> {
    let log_dir = ensure_log_dir(app)?;
    let log_file = log_dir.join(LAUNCHER_LOG_FILE);
    if !log_file.exists() {
        fs::write(&log_file, b"").map_err(|e| format!("io_error: cannot create log file: {e}"))?;
    }
    open_path_in_file_manager(&log_dir)?;
    Ok(log_dir.to_string_lossy().into_owned())
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

    writeln!(file, "[{}] {}", unix_timestamp_secs(), trimmed)
        .map_err(|e| format!("io_error: cannot write log file: {e}"))
}

pub fn collect_diagnostics(app: &AppHandle, repo: Option<&str>) -> Result<String, String> {
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
        format!("timestamp={}", unix_timestamp_secs()),
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

fn option_line(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("missing")
}

fn sanitize_path_part(value: &str) -> String {
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

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
