import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import logoImage from './assets/logo.png';
import './App.css';
import SettingsPanel from './components/SettingsPanel';
import {
  appendLauncherLog,
  clearCache,
  collectDiagnostics,
  copyTextToClipboard,
  detectSubrosa,
  downloadInjectionLibrary,
  forceRedownload,
  getLauncherUpdateState,
  getReleaseVersion,
  installLauncherUpdate,
  launchGame,
  loadSettings,
  openCacheFolder,
  openLogs,
  saveSettings,
} from './api/launcher';
import type {
  DetectionResult,
  LauncherUpdateState,
  LibraryDownloadRequest,
  LauncherSettings,
  Phase,
} from './types/launcher';

const GH_REPO = 'SubRosaCustom/client_releases';
const isWin = navigator.userAgent.includes('Windows');
const isLinux = navigator.userAgent.includes('Linux');
const defaultExecutableName = isWin ? 'subrosa.x64.exe' : 'subrosa.x64';

const phaseLabels: Record<Phase, string> = {
  idle: 'Launch',
  downloading: 'Downloading',
  launching: 'Launching',
  running: 'Running',
};

const configuredLibraryRequest: LibraryDownloadRequest = {
  repo: GH_REPO,
};

const CLIENT_DOWNLOAD_PROGRESS_EVENT = 'client-download-progress';
const LAUNCHER_UPDATE_PROGRESS_EVENT = 'launcher-update-progress';
const PROGRESS_BAR_WIDTH = 20;

interface LogEntry {
  id: number;
  message: string;
  key?: string;
}

interface TransferProgressPayload {
  downloaded: number;
  total: number | null;
  done: boolean;
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function formatProgressBar(label: string, payload: TransferProgressPayload) {
  const total = payload.total ?? payload.downloaded;
  const ratio = total > 0 ? Math.min(payload.downloaded / total, 1) : 0;
  const filled = payload.done ? PROGRESS_BAR_WIDTH : Math.round(ratio * PROGRESS_BAR_WIDTH);
  const bar = `${'#'.repeat(filled)}${'-'.repeat(PROGRESS_BAR_WIDTH - filled)}`;
  const percent = payload.done ? 100 : Math.round(ratio * 100);
  return `${label} [${bar}] ${percent}% (${formatBytes(payload.downloaded)}/${formatBytes(total)})`;
}

function App() {
  const [phase, setPhase] = useState<Phase>('idle');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [settings, setSettings] = useState<LauncherSettings>({
    executableName: defaultExecutableName,
    closeOnLaunch: false,
  });
  const [detection, setDetection] = useState<DetectionResult | null>(null);
  const [releaseVersion, setReleaseVersion] = useState('Unknown');
  const [launcherUpdate, setLauncherUpdate] = useState<LauncherUpdateState>({
    enabled: false,
    currentVersion: 'unknown',
    available: false,
    version: null,
    notes: null,
  });
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [activeSupportAction, setActiveSupportAction] = useState<string | null>(null);
  const [isInstallingLauncherUpdate, setIsInstallingLauncherUpdate] = useState(false);
  const nextLogId = useRef(1);
  const attemptedLauncherUpdate = useRef(false);

  const appendLog = (message: string) => {
    setLogs((currentLogs) => [...currentLogs, { id: nextLogId.current++, message }]);
    void appendLauncherLog(message).catch(() => undefined);
  };

  const upsertLog = (key: string, message: string) => {
    setLogs((currentLogs) => {
      const idx = currentLogs.findIndex((entry) => entry.key === key);
      if (idx === -1) {
        return [...currentLogs, { id: nextLogId.current++, key, message }];
      }
      const next = [...currentLogs];
      next[idx] = { ...next[idx], message };
      return next;
    });
  };

  const removeLog = (key: string) => {
    setLogs((currentLogs) => currentLogs.filter((entry) => entry.key !== key));
  };

  useEffect(() => {
    let disposed = false;

    const unlistenPromises = [
      listen<TransferProgressPayload>(CLIENT_DOWNLOAD_PROGRESS_EVENT, (event) => {
        if (disposed) return;
        upsertLog('client-progress', formatProgressBar('Client', event.payload));
        if (event.payload.done) {
          setTimeout(() => removeLog('client-progress'), 1200);
        }
      }),
      listen<TransferProgressPayload>(LAUNCHER_UPDATE_PROGRESS_EVENT, (event) => {
        if (disposed) return;
        upsertLog('launcher-progress', formatProgressBar('Launcher', event.payload));
      }),
    ];

    return () => {
      disposed = true;
      void Promise.all(unlistenPromises).then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const initialize = async () => {
      try {
        const [loadedSettings, detectedGame, release, launcherState] = await Promise.all([
          loadSettings(),
          detectSubrosa(),
          getReleaseVersion(configuredLibraryRequest.repo),
          getLauncherUpdateState().catch((error) => {
            appendLog(`Launcher update check failed: ${String(error)}`);
            return {
              enabled: false,
              currentVersion: 'unknown',
              available: false,
              version: null,
              notes: null,
            } satisfies LauncherUpdateState;
          }),
        ]);
        if (cancelled) return;

        setSettings(loadedSettings);
        setDetection(detectedGame);
        setReleaseVersion(release.value);
        setLauncherUpdate(launcherState);
        appendLog(
          detectedGame.gameDir
            ? `Sub Rosa found: ${detectedGame.gameDir}`
            : 'Sub Rosa not detected. Check Steam install and game files.',
        );
        if (launcherState.enabled) {
          appendLog(
            launcherState.available
              ? `Launcher update available: ${launcherState.version}`
              : `Launcher up to date: ${launcherState.currentVersion}`,
          );
        }
      } catch (e) {
        if (cancelled) return;
        appendLog(`Startup failed: ${String(e)}`);
      }
    };

    void initialize();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!launcherUpdate.available || attemptedLauncherUpdate.current) return;

    attemptedLauncherUpdate.current = true;
    setIsInstallingLauncherUpdate(true);
    appendLog(`Installing launcher update: ${launcherUpdate.version}`);
    void installLauncherUpdate().catch((error) => {
      removeLog('launcher-progress');
      appendLog(`Launcher update failed: ${String(error)}`);
      setIsInstallingLauncherUpdate(false);
      attemptedLauncherUpdate.current = false;
    });
  }, [launcherUpdate.available, launcherUpdate.version]);

  useEffect(() => {
    if (phase !== 'running') return;

    const timeoutId = setTimeout(() => {
      setPhase((current) => (current === 'running' ? 'idle' : current));
    }, 5000);

    return () => clearTimeout(timeoutId);
  }, [phase]);

  const handleLaunch = async () => {
    if (phase !== 'idle') return;
    const gameDir = detection?.gameDir;
    if (!gameDir) {
      appendLog('Cannot launch: game path is missing.');
      return;
    }
    try {
      setPhase('downloading');
      appendLog(`Pulling library from GitHub releases: ${configuredLibraryRequest.repo}`);
      const injectionLibraryPath = await downloadInjectionLibrary(configuredLibraryRequest);
      appendLog(`Library cached: ${injectionLibraryPath}`);
      setPhase('launching');
      if (isWin) appendLog('Starting game suspended...');
      else if (isLinux) appendLog('Starting game with LD_PRELOAD...');

      await launchGame(gameDir, settings.executableName, injectionLibraryPath);

      if (isWin) {
        appendLog('Injection complete.');
        appendLog('Game resumed.');
      } else if (isLinux) {
        appendLog('Library preloaded.');
      }

      appendLog(`Process started: ${settings.executableName}`);
      setPhase('running');
      if (settings.closeOnLaunch) {
        await getCurrentWindow().close();
      }
    } catch (e) {
      appendLog(`Launch failed: ${String(e)}`);
      setPhase('idle');
    }
  };

  const runSupportAction = async (actionName: string, action: () => Promise<void>) => {
    if (activeSupportAction) return;

    setActiveSupportAction(actionName);
    try {
      await action();
    } catch (e) {
      appendLog(`${actionName} failed: ${String(e)}`);
    } finally {
      setActiveSupportAction(null);
    }
  };

  const handleSettingsSave = async (nextSettings: LauncherSettings) => {
    setIsSavingSettings(true);
    try {
      await saveSettings(nextSettings);
      setSettings(nextSettings);
      appendLog(`Saved executable override: ${nextSettings.executableName}`);
      setIsSettingsOpen(false);
    } catch (e) {
      appendLog(`Save failed: ${String(e)}`);
    } finally {
      setIsSavingSettings(false);
    }
  };

  const handleOpenLogs = () =>
    void runSupportAction('openLogs', async () => {
      const path = await openLogs();
      appendLog(`Opened logs: ${path}`);
    });

  const handleOpenCacheFolder = () =>
    void runSupportAction('openCacheFolder', async () => {
      const path = await openCacheFolder();
      appendLog(`Opened cache folder: ${path}`);
    });

  const handleForceRedownload = () =>
    void runSupportAction('forceRedownload', async () => {
      const path = await forceRedownload(configuredLibraryRequest.repo);
      appendLog(`Cleared cached library for redownload: ${path}`);
    });

  const handleClearCache = () =>
    void runSupportAction('clearCache', async () => {
      const path = await clearCache();
      appendLog(`Cleared launcher cache: ${path}`);
    });

  const handleCopyDiagnostics = () =>
    void runSupportAction('copyDiagnostics', async () => {
      const diagnostics = await collectDiagnostics(configuredLibraryRequest.repo);

      try {
        await navigator.clipboard.writeText(diagnostics);
      } catch {
        await copyTextToClipboard(diagnostics);
      }

      appendLog('Diagnostics copied to clipboard.');
    });

  return (
    <main className="viewport">
      <div className="noise-overlay" />

      <div className="launcher-shell">
        <img src={logoImage} alt="Sub Rosa logo" className="logo" />
        <p className="version-label">launcher: {launcherUpdate.currentVersion}</p>
        <p className="version-label">client: {releaseVersion}</p>
        <button
          className={`action-btn ${phase !== 'idle' ? 'is-processing' : ''}`}
          onClick={handleLaunch}
          disabled={phase !== 'idle' || isInstallingLauncherUpdate}
        >
          <span className="btn-label">
            {isInstallingLauncherUpdate ? 'Updating Launcher' : phaseLabels[phase]}
          </span>
        </button>
        <button className="action-btn" onClick={() => setIsSettingsOpen(true)}>
          <span className="btn-label">Settings</span>
        </button>
      </div>

      <div className="log-stack-container">
        {logs.slice(-4).map((entry, i, arr) => {
          const d = arr.length - i - 1;
          return (
            <div
              key={entry.id}
              className={`log-entry log-depth-${d} ${d === 0 ? 'log-latest' : ''}`}
            >
              <span className="log-content">{entry.message}</span>
            </div>
          );
        })}
      </div>

      <SettingsPanel
        open={isSettingsOpen}
        saving={isSavingSettings}
        activeSupportAction={activeSupportAction}
        settings={settings}
        executableCandidates={detection?.executableCandidates ?? []}
        onSave={handleSettingsSave}
        onClose={() => setIsSettingsOpen(false)}
        onOpenLogs={handleOpenLogs}
        onOpenCacheFolder={handleOpenCacheFolder}
        onForceRedownload={handleForceRedownload}
        onClearCache={handleClearCache}
        onCopyDiagnostics={handleCopyDiagnostics}
      />
    </main>
  );
}

export default App;
