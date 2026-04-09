import { invoke } from '@tauri-apps/api/core';
import type {
  DetectionResult,
  LauncherUpdateState,
  LibraryDownloadRequest,
  LauncherSettings,
  ReleaseVersion,
} from '../types/launcher';

export function loadSettings() {
  return invoke<LauncherSettings>('load_settings');
}

export function saveSettings(settings: LauncherSettings) {
  return invoke<void>('save_settings', { settings });
}

export function detectSubrosa() {
  return invoke<DetectionResult>('detect_subrosa');
}

export function appendLauncherLog(message: string) {
  return invoke<void>('append_launcher_log', { message });
}

export function openLauncherLogs() {
  return invoke<string>('open_launcher_logs');
}

export function openClientCrashlogsFolder() {
  return invoke<string>('open_client_crashlogs_folder');
}

export function openClientConfigFolder() {
  return invoke<string>('open_client_config_folder');
}

export function openCacheFolder() {
  return invoke<string>('open_cache_folder');
}

export function forceRedownload(repo: string) {
  return invoke<string>('force_redownload', { args: { repo } });
}

export function clearCache() {
  return invoke<string>('clear_cache');
}

export function collectLauncherDiagnostics(repo?: string) {
  return invoke<string>('collect_launcher_diagnostics', { args: { repo } });
}

export function collectClientDiagnostics() {
  return invoke<string>('collect_client_diagnostics');
}

export function copyTextToClipboard(text: string) {
  return invoke<void>('copy_text_to_clipboard', { text });
}

export function getLauncherUpdateState() {
  return invoke<LauncherUpdateState>('get_launcher_update_state');
}

export function installLauncherUpdate() {
  return invoke<void>('install_launcher_update');
}

export function getReleaseVersion(repo: string) {
  return invoke<ReleaseVersion>('get_release_version', { args: { repo } });
}

export function downloadInjectionLibrary(request: LibraryDownloadRequest) {
  return invoke<string>('download_injection_library', {
    args: request,
  });
}

export function launchGame(gameDir: string, executableName: string, injectLibraryPath: string) {
  return invoke<void>('launch_game', {
    args: { gameDir, executableName, injectLibraryPath },
  });
}
