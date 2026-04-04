use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{async_runtime::Mutex as AsyncMutex, AppHandle};

use crate::{
    settings::{self, LauncherSettings},
    steam, support,
};

const DEFAULT_MANIFEST_NAME: &str = "manifest.json";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const MANIFEST_MAX_BYTES: u64 = 256 * 1024;
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
#[serde(rename_all = "camelCase")]
struct ReleaseManifest {
    schema_version: u32,
    version: String,
    created_at: String,
    artifacts: Vec<ReleaseArtifact>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ReleaseArtifact {
    target_os: String,
    target_arch: String,
    filename: String,
    sha256: String,
    size: u64,
}

#[derive(Debug)]
struct DownloadedArtifact {
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Copy)]
struct PlatformTarget {
    os: &'static str,
    arch: &'static str,
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
pub async fn download_injection_library(
    app: AppHandle,
    args: DownloadLibraryArgs,
) -> Result<String, String> {
    ensure_supported_platform()?;

    let _guard = download_lock().lock().await;

    let repo = normalize_repo(&args.repo)?;

    let client = build_http_client()?;

    let manifest_url = github_release_asset_url(&repo, DEFAULT_MANIFEST_NAME);

    let manifest_bytes =
        download_bytes_limited_with_retry(&client, &manifest_url, MANIFEST_MAX_BYTES).await?;
    let manifest = parse_manifest(&manifest_bytes)?;
    let artifact = pick_manifest_artifact(&manifest)?;
    let artifact_name = normalize_asset_name(&artifact.filename)?;

    let artifact_url = github_release_asset_url(&repo, &artifact_name);

    let cache_dir = support::repo_cache_dir(&app, &repo)?;
    let cached_artifact_path = cache_dir.join(&artifact_name);
    if cached_artifact_matches(&cached_artifact_path, artifact.size, &artifact.sha256)? {
        return Ok(cached_artifact_path.to_string_lossy().into_owned());
    }

    let downloaded_artifact = download_file_with_hash_with_retry(
        &client,
        &artifact_url,
        &cached_artifact_path,
        LIBRARY_MAX_BYTES,
    )
    .await?;

    validate_downloaded_artifact(&cached_artifact_path, &downloaded_artifact, &artifact)?;

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

async fn download_bytes_limited_with_retry(
    client: &reqwest::Client,
    url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match download_bytes_limited(client, url, max_bytes).await {
            Ok(bytes) => return Ok(bytes),
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

async fn download_bytes_limited(
    client: &reqwest::Client,
    url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
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

    let mut out = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(classify_request_error)? {
        let next = out.len() as u64 + chunk.len() as u64;
        if next > max_bytes {
            return Err(format!("download_too_large: {next} > {max_bytes}"));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

async fn download_file_with_hash_with_retry(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    max_bytes: u64,
) -> Result<DownloadedArtifact, String> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match download_file_with_hash(client, url, target, max_bytes).await {
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

async fn download_file_with_hash(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    max_bytes: u64,
) -> Result<DownloadedArtifact, String> {
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
    let mut hasher = Sha256::new();
    let mut size = 0_u64;

    while let Some(chunk) = res.chunk().await.map_err(classify_request_error)? {
        size += chunk.len() as u64;
        if size > max_bytes {
            let _ = fs::remove_file(&tmp);
            return Err(format!("download_too_large: {size} > {max_bytes}"));
        }
        file.write_all(&chunk)
            .map_err(|e| format!("io_error: cannot write file chunk: {e}"))?;
        hasher.update(&chunk);
    }

    file.flush()
        .map_err(|e| format!("io_error: cannot flush file: {e}"))?;

    move_tmp_into_place(&tmp, target)?;

    Ok(DownloadedArtifact {
        size,
        sha256: bytes_to_hex(&hasher.finalize()),
    })
}

fn validate_downloaded_artifact(
    artifact_path: &Path,
    downloaded_artifact: &DownloadedArtifact,
    manifest_artifact: &ReleaseArtifact,
) -> Result<(), String> {
    if downloaded_artifact.size != manifest_artifact.size {
        let _ = fs::remove_file(artifact_path);
        return Err(format!(
            "artifact_size_mismatch: expected {} bytes, got {}",
            manifest_artifact.size, downloaded_artifact.size
        ));
    }
    if !hash_matches(&downloaded_artifact.sha256, &manifest_artifact.sha256) {
        let _ = fs::remove_file(artifact_path);
        return Err("artifact_hash_mismatch".into());
    }

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

fn cached_artifact_matches(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    let meta = match fs::metadata(path) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    if meta.len() != expected_size {
        return Ok(false);
    }

    let hash = sha256_file(path)?;
    Ok(hash_matches(&hash, expected_sha256))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("io_error: cannot open file: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];

    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("io_error: cannot read file: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(bytes_to_hex(&hasher.finalize()))
}

fn parse_manifest(bytes: &[u8]) -> Result<ReleaseManifest, String> {
    let manifest: ReleaseManifest =
        serde_json::from_slice(bytes).map_err(|e| format!("manifest_invalid_json: {e}"))?;

    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "manifest_schema_unsupported: expected {MANIFEST_SCHEMA_VERSION}, got {}",
            manifest.schema_version
        ));
    }
    if manifest.version.trim().is_empty() {
        return Err("manifest_invalid_version".into());
    }
    if manifest.created_at.trim().is_empty() {
        return Err("manifest_invalid_created_at".into());
    }
    if manifest.artifacts.is_empty() {
        return Err("manifest_missing_artifacts".into());
    }

    for artifact in &manifest.artifacts {
        validate_manifest_artifact(artifact)?;
    }

    Ok(manifest)
}

fn validate_manifest_artifact(artifact: &ReleaseArtifact) -> Result<(), String> {
    normalize_asset_name(&artifact.filename)?;

    if artifact.target_os.trim().is_empty() {
        return Err("manifest_invalid_target_os".into());
    }
    if artifact.target_arch.trim().is_empty() {
        return Err("manifest_invalid_target_arch".into());
    }
    if artifact.size == 0 || artifact.size > LIBRARY_MAX_BYTES {
        return Err("manifest_invalid_artifact_size".into());
    }
    if !is_sha256_hex(&artifact.sha256) {
        return Err("manifest_invalid_sha256".into());
    }

    Ok(())
}

fn pick_manifest_artifact(manifest: &ReleaseManifest) -> Result<ReleaseArtifact, String> {
    let platform_target = current_platform_target();

    manifest
        .artifacts
        .iter()
        .find(|artifact| {
            artifact.target_os.eq_ignore_ascii_case(platform_target.os)
                && artifact
                    .target_arch
                    .eq_ignore_ascii_case(platform_target.arch)
        })
        .cloned()
        .ok_or_else(|| {
            format!(
                "manifest_missing_target_asset: {}/{}",
                platform_target.os, platform_target.arch
            )
        })
}

fn current_platform_target() -> PlatformTarget {
    PlatformTarget {
        os: current_target_os(),
        arch: current_target_arch(),
    }
}

fn current_target_os() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "unsupported"
    }
}

fn current_target_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x86_64"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "aarch64"
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        "unsupported"
    }
}

fn github_release_asset_url(repo: &str, asset_name: &str) -> String {
    format!("https://github.com/{repo}/releases/latest/download/{asset_name}")
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

fn normalize_asset_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("config_invalid_asset_name_empty".into());
    }
    if trimmed.contains("..") {
        return Err("config_invalid_asset_name".into());
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err("config_invalid_asset_name".into());
    }
    Ok(trimmed.to_string())
}

fn is_repo_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hash_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
        out.push(char::from_digit((b & 0x0F) as u32, 16).unwrap_or('0'));
    }
    out
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

#[cfg(test)]
fn injection_library_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "srcustom.dll"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "libsrcustom.so"
    }
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
    fn asset_name_validation_rules_are_enforced() {
        assert!(normalize_asset_name("manifest.json").is_ok());
        assert!(normalize_asset_name("../manifest.json").is_err());
        assert!(normalize_asset_name("manifest/.json").is_err());
        assert!(normalize_asset_name(" ").is_err());
    }

    #[test]
    fn manifest_parsing_and_selection_work_for_current_target() {
        let manifest = format!(
            r#"{{
                "schemaVersion":1,
                "version":"1.2.3",
                "createdAt":"2026-03-07T00:00:00Z",
                "artifacts":[
                    {{"targetOs":"{}","targetArch":"{}","filename":"{}","sha256":"{}","size":4}}
                ]
            }}"#,
            current_target_os(),
            current_target_arch(),
            injection_library_name(),
            "a".repeat(64)
        );

        let parsed = parse_manifest(manifest.as_bytes()).expect("manifest should parse");
        let selected = pick_manifest_artifact(&parsed).expect("artifact should be selected");

        assert_eq!(selected.filename, injection_library_name());
        assert_eq!(selected.size, 4);
    }

    #[tokio::test]
    async fn download_bytes_limited_rejects_large_payloads() {
        let server = MockServer::start().await;
        let body = vec![0_u8; 6];
        Mock::given(method("GET"))
            .and(path("/oversize"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/octet-stream"))
            .mount(&server)
            .await;

        let client = build_http_client().expect("client should build");
        let url = format!("{}/oversize", server.uri());
        let err = download_bytes_limited_with_retry(&client, &url, 4)
            .await
            .expect_err("oversize should fail");

        assert!(err.starts_with("download_too_large"));
    }

    #[tokio::test]
    async fn download_file_with_hash_writes_expected_content() {
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

        let downloaded = download_file_with_hash_with_retry(&client, &url, &target, 1024)
            .await
            .expect("download should succeed");

        assert_eq!(downloaded.size, 4);
        assert_eq!(
            downloaded.sha256,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
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
