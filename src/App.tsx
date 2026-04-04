import { useEffect, useState } from 'react';
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
  launchGame,
  loadSettings,
  openCacheFolder,
  openLogs,
  saveSettings,
} from './api/launcher';
import type {
  DetectionResult,
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

function App() {
  const [phase, setPhase] = useState<Phase>('idle');
  const [logs, setLogs] = useState<string[]>([]);
  const [settings, setSettings] = useState<LauncherSettings>({
    executableName: defaultExecutableName,
    closeOnLaunch: false,
  });
  const [detection, setDetection] = useState<DetectionResult | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [activeSupportAction, setActiveSupportAction] = useState<string | null>(null);

  const appendLog = (message: string) => {
    setLogs((currentLogs) => [...currentLogs, message]);
    void appendLauncherLog(message).catch(() => undefined);
  };

  useEffect(() => {
    let cancelled = false;

    const initialize = async () => {
      try {
        const [loadedSettings, detectedGame] = await Promise.all([loadSettings(), detectSubrosa()]);
        if (cancelled) return;

        setSettings(loadedSettings);
        setDetection(detectedGame);
        appendLog(
          detectedGame.gameDir
            ? `Sub Rosa found: ${detectedGame.gameDir}`
            : 'Sub Rosa not detected. Check Steam install and game files.',
        );
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
        <button
          className={`action-btn ${phase !== 'idle' ? 'is-processing' : ''}`}
          onClick={handleLaunch}
          disabled={phase !== 'idle'}
        >
          <span className="btn-label">{phaseLabels[phase]}</span>
        </button>
        <button className="action-btn" onClick={() => setIsSettingsOpen(true)}>
          <span className="btn-label">Settings</span>
        </button>
      </div>

      <div className="log-stack-container">
        {logs.slice(-4).map((msg, i, arr) => {
          const d = arr.length - i - 1;
          return (
            <div
              key={`${i}-${msg}`}
              className={`log-entry log-depth-${d} ${d === 0 ? 'log-latest' : ''}`}
            >
              <span className="log-content">{msg}</span>
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
