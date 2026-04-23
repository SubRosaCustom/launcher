export type Phase = 'idle' | 'downloading' | 'launching' | 'running';

export interface LauncherSettings {
  executableName: string;
  closeOnLaunch: boolean;
}

export interface LibraryDownloadRequest {
  repo: string;
}

export interface DetectionResult {
  steamDir: string | null;
  gameDir: string | null;
  executableCandidates: string[];
}

export interface ReleaseVersion {
  value: string;
  publishedAt: string | null;
}

export interface ReleaseDetails {
  tagName: string;
  value: string;
  publishedAt: string | null;
  notes: string | null;
}

export interface LauncherUpdateState {
  enabled: boolean;
  currentVersion: string;
  available: boolean;
  version: string | null;
  notes: string | null;
}
