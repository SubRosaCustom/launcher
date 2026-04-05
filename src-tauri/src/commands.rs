use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{async_runtime::Mutex as AsyncMutex, AppHandle};
use tauri_plugin_updater::UpdaterExt;

use crate::{
    launcher_updater_pubkey,
    settings::{self, LauncherSettings},
    steam, support,
};

const RELEASE_TAG: &str = "release";
const LIBRARY_MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DOWNLOAD_ATTEMPTS: usize = 3;
const RETRY_DELAYS_MS: [u64; 2] = [250, 750];
const CONNECT_TIMEOUT_SECS: u64 = 8;
const REQUEST_TIMEOUT_SECS: u64 = 30;

static DOWNLOAD_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

fn download_lock() -> &'static AsyncMutex<()> {
    DOWNLOAD_LOCK.get_or_init(|| AsyncMutex::new(()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLibraryArgs {
    pub repo: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchGameArgs {
    pub game_dir: String,
    pub executable_name: String,
    pub inject_library_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoArgs {
    pub repo: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsArgs {
    pub repo: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseVersion {
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateState {
    pub enabled: bool,
    pub current_version: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<LauncherSettings, String> {
    settings::load_settings(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: LauncherSettings) -> Result<(), String> {
    settings::save_settings(&app, &settings)
}

#[tauri::command]
pub fn detect_subrosa() -> steam::DetectionResult {
    steam::detect_subrosa()
}

#[tauri::command]
pub fn append_launcher_log(app: AppHandle, message: String) -> Result<(), String> {
    support::append_launcher_log(&app, &message)
}

#[tauri::command]
pub fn open_logs(app: AppHandle) -> Result<String, String> {
    support::open_logs(&app)
}

#[tauri::command]
pub fn open_cache_folder(app: AppHandle) -> Result<String, String> {
    support::open_cache_folder(&app)
}

#[tauri::command]
pub fn force_redownload(app: AppHandle, args: RepoArgs) -> Result<String, String> {
    let repo = normalize_repo(&args.repo)?;
    support::force_redownload(&app, &repo)
}

#[tauri::command]
pub fn clear_cache(app: AppHandle) -> Result<String, String> {
    support::clear_cache(&app)
}

#[tauri::command]
pub fn collect_diagnostics(app: AppHandle, args: DiagnosticsArgs) -> Result<String, String> {
    let repo = args.repo.as_deref().map(normalize_repo).transpose()?;
    support::collect_diagnostics(&app, repo.as_deref())
}

#[tauri::command]
pub fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    support::copy_text_to_clipboard(&text)
}

#[tauri::command]
pub async fn get_launcher_update_state(app: AppHandle) -> Result<LauncherUpdateState, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    if launcher_updater_pubkey().is_none() {
        return Ok(LauncherUpdateState {
            enabled: false,
            current_version,
            available: false,
            version: None,
            notes: None,
        });
    }

    let updater = app
        .updater()
        .map_err(|e| format!("launcher_updater_init_failed: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("launcher_update_check_failed: {e}"))?;

    Ok(match update {
        Some(update) => LauncherUpdateState {
            enabled: true,
            current_version,
            available: true,
            version: Some(update.version.to_string()),
            notes: update.body,
        },
        None => LauncherUpdateState {
            enabled: true,
            current_version,
            available: false,
            version: None,
            notes: None,
        },
    })
}

#[tauri::command]
pub async fn install_launcher_update(app: AppHandle) -> Result<(), String> {
    if launcher_updater_pubkey().is_none() {
        return Err("launcher_updater_not_configured".into());
    }

    let updater = app
        .updater()
        .map_err(|e| format!("launcher_updater_init_failed: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("launcher_update_check_failed: {e}"))?
        .ok_or_else(|| "launcher_update_not_available".to_string())?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("launcher_update_install_failed: {e}"))?;

    app.restart();
}

#[tauri::command]
pub async fn get_release_version(args: RepoArgs) -> Result<ReleaseVersion, String> {
    let repo = normalize_repo(&args.repo)?;
    let client = build_http_client()?;
    let release_url = github_release_api_url(&repo, RELEASE_TAG);
    let response = client
        .get(&release_url)
        .send()
        .await
        .map_err(classify_request_error)?;
    classify_status(response.status())?;
    let release_bytes = response
        .bytes()
        .await
        .map_err(classify_request_error)?
        .to_vec();
    let release: GitHubRelease = serde_json::from_slice(&release_bytes)
        .map_err(|e| format!("release_metadata_invalid_json: {e}"))?;
    let value = if release.name.trim().is_empty() {
        release.tag_name.trim().to_string()
    } else {
        release.name.trim().to_string()
    };
    if value.is_empty() {
        return Err("release_metadata_missing_version".into());
    }
    Ok(ReleaseVersion { value })
}

#[tauri::command]
pub async fn download_injection_library(
    app: AppHandle,
    args: DownloadLibraryArgs,
) -> Result<String, String> {
    ensure_supported_platform()?;

    let _guard = download_lock().lock().await;

    let repo = normalize_repo(&args.repo)?;

    let client = build_http_client()?;

    let library_name = platform_library_name();
    let artifact_url = github_release_asset_url(&repo, RELEASE_TAG, library_name);

    let cache_dir = support::repo_cache_dir(&app, &repo)?;
    let cached_artifact_path = cache_dir.join(library_name);
    if cached_artifact_path.exists() {
        return Ok(cached_artifact_path.to_string_lossy().into_owned());
    }

    download_file_with_retry(
        &client,
        &artifact_url,
        &cached_artifact_path,
        LIBRARY_MAX_BYTES,
    )
    .await?;

    Ok(cached_artifact_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn launch_game(args: LaunchGameArgs) -> Result<(), String> {
    ensure_supported_platform()?;
    let executable_path = resolve_executable_path(&args);
    ensure_executable_exists(&executable_path)?;
    let inject_library_path =
        validate_injection_library_path(args.inject_library_path.as_deref())?.map(str::to_owned);

    launch_game_for_platform(
        &executable_path,
        &args.game_dir,
        inject_library_path.as_deref(),
    )
}

fn resolve_executable_path(args: &LaunchGameArgs) -> PathBuf {
    let exe = PathBuf::from(&args.game_dir).join(&args.executable_name);
    #[cfg(target_os = "windows")]
    {
        if exe.exists() {
            return exe;
        }
        if args.executable_name.to_ascii_lowercase().ends_with(".exe") {
            return exe;
        }
        let alt = PathBuf::from(&args.game_dir).join(format!("{}.exe", args.executable_name));
        if alt.exists() {
            return alt;
        }
        exe
    }
    #[cfg(not(target_os = "windows"))]
    {
        exe
    }
}

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("srclauncher")
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("http_client_build_failed: {e}"))
}

async fn download_file_with_retry(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    max_bytes: u64,
) -> Result<(), String> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match download_file(client, url, target, max_bytes).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if attempt >= MAX_DOWNLOAD_ATTEMPTS || !is_retryable_error(&err) {
                    return Err(err);
                }
                let delay = RETRY_DELAYS_MS[(attempt - 1).min(RETRY_DELAYS_MS.len() - 1)];
                thread::sleep(Duration::from_millis(delay));
            }
        }
    }
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    max_bytes: u64,
) -> Result<(), String> {
    let tmp = unique_tmp_path(target)?;

    let mut res = client
        .get(url)
        .send()
        .await
        .map_err(classify_request_error)?;

    classify_status(res.status())?;

    if let Some(len) = res.content_length() {
        if len > max_bytes {
            return Err(format!("download_too_large: {len} > {max_bytes}"));
        }
    }

    let mut file =
        fs::File::create(&tmp).map_err(|e| format!("io_error: cannot create file: {e}"))?;
    let mut size = 0_u64;

    while let Some(chunk) = res.chunk().await.map_err(classify_request_error)? {
        size += chunk.len() as u64;
        if size > max_bytes {
            let _ = fs::remove_file(&tmp);
            return Err(format!("download_too_large: {size} > {max_bytes}"));
        }
        file.write_all(&chunk)
            .map_err(|e| format!("io_error: cannot write file chunk: {e}"))?;
    }

    file.flush()
        .map_err(|e| format!("io_error: cannot flush file: {e}"))?;

    move_tmp_into_place(&tmp, target)?;

    Ok(())
}

fn ensure_executable_exists(executable_path: &Path) -> Result<(), String> {
    if executable_path.exists() {
        return Ok(());
    }

    Err(format!(
        "executable not found: {}",
        executable_path.to_string_lossy()
    ))
}

fn validate_injection_library_path(
    inject_library_path: Option<&str>,
) -> Result<Option<&str>, String> {
    if let Some(inject_library_path) = inject_library_path {
        if !Path::new(inject_library_path).exists() {
            return Err(format!("inject library not found: {inject_library_path}"));
        }
    }

    Ok(inject_library_path)
}

fn move_tmp_into_place(tmp: &Path, target: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if target.exists() {
            fs::remove_file(target)
                .map_err(|e| format!("io_error: cannot replace existing artifact: {e}"))?;
        }
    }

    fs::rename(tmp, target).map_err(|e| format!("io_error: cannot move artifact into cache: {e}"))
}

fn unique_tmp_path(target: &Path) -> Result<PathBuf, String> {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock_error: {e}"))?
        .as_nanos();
    Ok(target.with_extension(format!("tmp.{n}")))
}

fn platform_library_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "srcustom.dll"
    }
    #[cfg(target_os = "linux")]
    {
        "libsrcustom.so"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "unsupported"
    }
}

fn github_release_asset_url(repo: &str, tag: &str, asset_name: &str) -> String {
    format!("https://github.com/{repo}/releases/download/{tag}/{asset_name}")
}

fn github_release_api_url(repo: &str, tag: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
}

fn normalize_repo(repo: &str) -> Result<String, String> {
    let trimmed = repo.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err("config_invalid_repo_empty".into());
    }

    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() != 2 {
        return Err("config_invalid_repo_format".into());
    }
    if !is_repo_segment(parts[0]) || !is_repo_segment(parts[1]) {
        return Err("config_invalid_repo_chars".into());
    }

    Ok(format!("{}/{}", parts[0], parts[1]))
}

fn is_repo_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn classify_request_error(err: reqwest::Error) -> String {
    if err.is_timeout() {
        "download_timeout".to_string()
    } else if err.is_connect() || err.is_request() || err.is_body() {
        format!("download_transport_error: {err}")
    } else {
        format!("download_request_error: {err}")
    }
}

fn classify_status(status: StatusCode) -> Result<(), String> {
    if status.is_success() {
        return Ok(());
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err("download_rate_limited".into());
    }
    if status.is_server_error() {
        return Err(format!("download_server_error: {status}"));
    }

    Err(format!("download_http_status: {status}"))
}

fn is_retryable_error(err: &str) -> bool {
    err.starts_with("download_timeout")
        || err.starts_with("download_transport_error")
        || err.starts_with("download_server_error")
        || err.starts_with("download_rate_limited")
}

fn ensure_supported_platform() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Err("macOS is not supported".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn launch_game_for_platform(
    executable_path: &Path,
    game_dir: &str,
    inject_library_path: Option<&str>,
) -> Result<(), String> {
    if let Some(inject_library_path) = inject_library_path {
        return launch_game_windows(executable_path, game_dir, inject_library_path);
    }

    launch_game_process(executable_path, game_dir, None)
}

#[cfg(target_os = "linux")]
fn launch_game_for_platform(
    executable_path: &Path,
    game_dir: &str,
    inject_library_path: Option<&str>,
) -> Result<(), String> {
    launch_game_process(executable_path, game_dir, inject_library_path)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn launch_game_for_platform(
    executable_path: &Path,
    game_dir: &str,
    _inject_library_path: Option<&str>,
) -> Result<(), String> {
    launch_game_process(executable_path, game_dir, None)
}

fn launch_game_process(
    executable_path: &Path,
    game_dir: &str,
    preload_library_path: Option<&str>,
) -> Result<(), String> {
    let mut game_process = Command::new(executable_path);
    game_process.current_dir(game_dir);
    #[cfg(not(target_os = "linux"))]
    let _ = preload_library_path;
    #[cfg(target_os = "linux")]
    if let Some(preload_library_path) = preload_library_path {
        game_process.env("LD_PRELOAD", preload_library_path);
    }

    game_process
        .spawn()
        .map_err(|e| format!("failed to launch game process: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn launch_game_windows(exe: &Path, game_dir: &str, lib: &str) -> Result<(), String> {
    use std::{
        mem::zeroed,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            CreateProcessW, ResumeThread, TerminateProcess, CREATE_SUSPENDED, PROCESS_INFORMATION,
            STARTUPINFOW,
        },
    };

    let exe_w = to_wide(exe.as_os_str().to_string_lossy().as_ref());
    let dir_w = to_wide(game_dir);
    let mut si: STARTUPINFOW = unsafe { zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };

    let ok = unsafe {
        CreateProcessW(
            exe_w.as_ptr(),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            CREATE_SUSPENDED,
            null(),
            dir_w.as_ptr(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(format!(
            "failed to launch game process: {}",
            std::io::Error::last_os_error()
        ));
    }

    if let Err(e) = inject_dll(pi.dwProcessId, lib) {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
        }
        return Err(e);
    }

    let n = unsafe { ResumeThread(pi.hThread) };
    if n == u32::MAX {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
        }
        return Err(format!(
            "failed to resume game thread: {}",
            std::io::Error::last_os_error()
        ));
    }

    unsafe {
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn inject_dll(pid: u32, lib: &str) -> Result<(), String> {
    use dll_syringe::{process::OwnedProcess, Syringe};

    let proc = OwnedProcess::from_pid(pid).map_err(|e| format!("cannot target process: {e}"))?;
    let syringe = Syringe::for_process(proc);
    syringe
        .inject(lib)
        .map_err(|e| format!("dll injection failed: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn repo_validation_rules_are_enforced() {
        assert!(normalize_repo("owner/repo").is_ok());
        assert!(normalize_repo("owner/repo-extra").is_ok());
        assert!(normalize_repo("owner").is_err());
        assert!(normalize_repo("owner/repo/path").is_err());
        assert!(normalize_repo("owner/rep o").is_err());
    }

    #[test]
    fn platform_library_name_matches_current_target() {
        #[cfg(target_os = "windows")]
        assert_eq!(platform_library_name(), "srcustom.dll");
        #[cfg(target_os = "linux")]
        assert_eq!(platform_library_name(), "libsrcustom.so");
    }

    #[tokio::test]
    async fn download_file_writes_expected_content() {
        let server = MockServer::start().await;
        let body = b"test".to_vec();
        Mock::given(method("GET"))
            .and(path("/artifact"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(body.clone(), "application/octet-stream"),
            )
            .mount(&server)
            .await;

        let client = build_http_client().expect("client should build");
        let dir = tempdir().expect("tmp dir should exist");
        let target = dir.path().join("artifact.bin");
        let url = format!("{}/artifact", server.uri());

        download_file_with_retry(&client, &url, &target, 1024)
            .await
            .expect("download should succeed");

        assert_eq!(fs::read(target).expect("artifact should exist"), body);
    }

    #[test]
    fn capability_scope_is_least_privilege() {
        let caps = include_str!("../capabilities/default.json");
        assert!(!caps.contains("opener:default"));
        assert!(!caps.contains("process:default"));
        assert!(!caps.contains("updater:default"));
    }
}
