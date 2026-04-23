import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import logoImage from './assets/logo.png';
import './App.css';
import SettingsPanel from './components/SettingsPanel';
import {
  appendLauncherLog,
  clearCache,
  collectClientDiagnostics,
  collectLauncherDiagnostics,
  copyTextToClipboard,
  detectSubrosa,
  downloadInjectionLibrary,
  forceRedownload,
  getReleaseHistory,
  getLauncherUpdateState,
  installLauncherUpdate,
  launchGame,
  loadSettings,
  openCacheFolder,
  openClientConfigFolder,
  openClientCrashlogsFolder,
  openLauncherLogs,
  saveSettings,
} from './api/launcher';
import type {
  DetectionResult,
  LauncherUpdateState,
  LibraryDownloadRequest,
  LauncherSettings,
  Phase,
  ReleaseDetails,
} from './types/launcher';

const GH_REPO = 'SubRosaCustom/client_releases';
const LAUNCHER_REPO = 'SubRosaCustom/launcher';
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
const CHANGELOG_PAGE_SIZE = 8;

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

function getMarkdownContent(notes: string | null) {
  if (!notes || !notes.trim()) {
    return 'No changelog available.';
  }

  return notes;
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

function getLauncherReleaseTags(version: string) {
  const trimmed = version.trim();
  if (!trimmed || trimmed === 'unknown') {
    return [];
  }

  return [`launcher-v${trimmed}`, trimmed, `v${trimmed}`];
}

interface ChangelogModalState {
  title: string;
  releases: ReleaseDetails[];
  selectedTagName: string | null;
  fallbackVersion: string;
}

function findReleaseByTags(releases: ReleaseDetails[], tags: string[]) {
  for (const tag of tags) {
    const match = releases.find((release) => release.tagName === tag);
    if (match) {
      return match;
    }
  }

  return releases[0] ?? null;
}

function formatPublishedAt(value: string | null) {
  if (!value) {
    return null;
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return null;
  }

  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  }).format(date);
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
  const [clientReleaseHistory, setClientReleaseHistory] = useState<ReleaseDetails[]>([]);
  const [launcherReleaseHistory, setLauncherReleaseHistory] = useState<ReleaseDetails[]>([]);
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
  const [showLauncherUpdatePrompt, setShowLauncherUpdatePrompt] = useState(false);
  const [changelogModal, setChangelogModal] = useState<ChangelogModalState | null>(null);
  const [changelogPage, setChangelogPage] = useState(0);

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
        const [loadedSettings, detectedGame, clientHistory, launcherState, launcherHistory] =
          await Promise.all([
          loadSettings(),
          detectSubrosa(),
          getReleaseHistory(configuredLibraryRequest.repo).catch((error) => {
            appendLog(`Client changelog lookup failed: ${String(error)}`);
            return [];
          }),
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
          getReleaseHistory(LAUNCHER_REPO).catch((error) => {
            appendLog(`Launcher changelog lookup failed: ${String(error)}`);
            return [];
          }),
        ]);
        if (cancelled) return;

        setSettings(loadedSettings);
        setDetection(detectedGame);
        setClientReleaseHistory(clientHistory);
        setLauncherReleaseHistory(launcherHistory);
        setLauncherUpdate(launcherState);
        setReleaseVersion(clientHistory[0]?.value ?? 'Unknown');
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
    if (!launcherUpdate.available) {
      setShowLauncherUpdatePrompt(false);
      return;
    }

    setShowLauncherUpdatePrompt(true);
  }, [launcherUpdate.available, launcherUpdate.version, launcherUpdate.notes]);

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
      const message = String(e);
      if (message.includes('release_asset_missing:')) {
        appendLog(`Launch failed: client release asset missing in latest release from ${configuredLibraryRequest.repo}.`);
      } else if (message.includes('download_http_status: 404')) {
        appendLog(`Launch failed: client release download returned 404 from ${configuredLibraryRequest.repo}.`);
      } else {
        appendLog(`Launch failed: ${message}`);
      }
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

  const copySupportText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      await copyTextToClipboard(text);
    }
  };

  const handleOpenLauncherLogs = () =>
    void runSupportAction('openLauncherLogs', async () => {
      const path = await openLauncherLogs();
      appendLog(`Opened launcher logs: ${path}`);
    });

  const handleOpenClientCrashlogsFolder = () =>
    void runSupportAction('openClientCrashlogsFolder', async () => {
      const path = await openClientCrashlogsFolder();
      appendLog(`Opened client crashlogs: ${path}`);
    });

  const handleOpenClientConfigFolder = () =>
    void runSupportAction('openClientConfigFolder', async () => {
      const path = await openClientConfigFolder();
      appendLog(`Opened client config folder: ${path}`);
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

  const handleCopyLauncherDiagnostics = () =>
    void runSupportAction('copyLauncherDiagnostics', async () => {
      const diagnostics = await collectLauncherDiagnostics(configuredLibraryRequest.repo);
      await copySupportText(diagnostics);
      appendLog('Launcher diagnostics copied to clipboard.');
    });

  const handleCopyClientDiagnostics = () =>
    void runSupportAction('copyClientDiagnostics', async () => {
      const diagnostics = await collectClientDiagnostics();
      await copySupportText(diagnostics);
      appendLog('Client diagnostics copied to clipboard.');
    });

  const handleInstallLauncherUpdate = async () => {
    if (!launcherUpdate.available || isInstallingLauncherUpdate) return;

    setIsInstallingLauncherUpdate(true);
    setShowLauncherUpdatePrompt(false);
    appendLog(`Installing launcher update: ${launcherUpdate.version}`);
    try {
      await installLauncherUpdate();
    } catch (error) {
      removeLog('launcher-progress');
      appendLog(`Launcher update failed: ${String(error)}`);
      setIsInstallingLauncherUpdate(false);
      setShowLauncherUpdatePrompt(true);
    }
  };

  const handleDismissLauncherUpdate = () => {
    setShowLauncherUpdatePrompt(false);
    appendLog(`Skipped launcher update: ${launcherUpdate.version}`);
  };

  const openClientChangelog = () => {
    const selectedRelease = clientReleaseHistory[0] ?? null;
    setChangelogModal({
      title: 'Client changelog',
      releases: clientReleaseHistory,
      selectedTagName: selectedRelease?.tagName ?? null,
      fallbackVersion: releaseVersion,
    });
    setChangelogPage(0);
  };

  const openLauncherChangelog = () => {
    const selectedRelease = findReleaseByTags(
      launcherReleaseHistory,
      getLauncherReleaseTags(launcherUpdate.currentVersion),
    );
    const selectedIndex =
      selectedRelease == null
        ? 0
        : launcherReleaseHistory.findIndex((release) => release.tagName === selectedRelease.tagName);
    setChangelogModal({
      title: 'Launcher changelog',
      releases: launcherReleaseHistory,
      selectedTagName: selectedRelease?.tagName ?? null,
      fallbackVersion: launcherUpdate.currentVersion,
    });
    setChangelogPage(selectedIndex < 0 ? 0 : Math.floor(selectedIndex / CHANGELOG_PAGE_SIZE));
  };

  const selectedRelease =
    changelogModal == null
      ? null
      : changelogModal.releases.find((release) => release.tagName === changelogModal.selectedTagName) ??
        changelogModal.releases[0] ??
        null;
  const changelogPageCount =
    changelogModal == null ? 1 : Math.max(1, Math.ceil(changelogModal.releases.length / CHANGELOG_PAGE_SIZE));
  const visibleReleases =
    changelogModal == null
      ? []
      : changelogModal.releases.slice(
          changelogPage * CHANGELOG_PAGE_SIZE,
          changelogPage * CHANGELOG_PAGE_SIZE + CHANGELOG_PAGE_SIZE,
        );

  return (
    <main className="viewport">
      <div className="noise-overlay" />

      <div className="launcher-shell">
        <img src={logoImage} alt="Sub Rosa logo" className="logo" />
        <div className="release-info">
          <button
            className="version-label version-button"
            onClick={openLauncherChangelog}
            title="Click to see changelogs"
            type="button"
          >
            launcher: {launcherUpdate.currentVersion}
          </button>
          <button
            className="version-label version-button"
            onClick={openClientChangelog}
            title="Click to see changelogs"
            type="button"
          >
            client: {releaseVersion}
          </button>
        </div>
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
        onOpenLauncherLogs={handleOpenLauncherLogs}
        onOpenClientCrashlogsFolder={handleOpenClientCrashlogsFolder}
        onOpenClientConfigFolder={handleOpenClientConfigFolder}
        onOpenCacheFolder={handleOpenCacheFolder}
        onForceRedownload={handleForceRedownload}
        onClearCache={handleClearCache}
        onCopyLauncherDiagnostics={handleCopyLauncherDiagnostics}
        onCopyClientDiagnostics={handleCopyClientDiagnostics}
      />

      {showLauncherUpdatePrompt ? (
        <div className="update-modal-backdrop">
          <div className="update-modal">
            <p className="update-modal-title">Launcher update available</p>
            <p className="hint">Install version {launcherUpdate.version}</p>
            <div className="update-notes">
              <div className="markdown-body">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {getMarkdownContent(launcherUpdate.notes)}
                </ReactMarkdown>
              </div>
            </div>
            <div className="update-modal-actions">
              <button className="action-btn" onClick={handleInstallLauncherUpdate}>
                <span className="btn-label">Install</span>
              </button>
              <button className="action-btn" onClick={handleDismissLauncherUpdate}>
                <span className="btn-label">Not Now</span>
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {changelogModal ? (
        <div className="update-modal-backdrop" onClick={() => setChangelogModal(null)}>
          <div className="update-modal changelog-modal" onClick={(event) => event.stopPropagation()}>
            <div className="changelog-header">
              <div className="changelog-header-copy">
                <p className="update-modal-title">{changelogModal.title}</p>
              </div>
            </div>
            <div className="changelog-layout">
              <div className="changelog-sidebar">
                <div className="changelog-sidebar-header">
                  <span className="hint">
                    Releases {changelogModal.releases.length > 0 ? changelogModal.releases.length : 0}
                  </span>
                  <span className="hint">
                    Page {Math.min(changelogPage + 1, changelogPageCount)}/{changelogPageCount}
                  </span>
                </div>
                <div className="changelog-release-list">
                  {visibleReleases.length > 0 ? (
                    visibleReleases.map((release) => (
                      <button
                        key={release.tagName}
                        className={`changelog-release-button ${
                          selectedRelease?.tagName === release.tagName ? 'is-selected' : ''
                        }`}
                        onClick={() =>
                          setChangelogModal((current) =>
                            current == null
                              ? current
                              : { ...current, selectedTagName: release.tagName },
                          )
                        }
                        type="button"
                      >
                        <span className="changelog-release-version">
                          {release.value}
                          {release.tagName === changelogModal.releases[0]?.tagName ? (
                            <span className="changelog-release-latest"> latest</span>
                          ) : null}
                        </span>
                        <span className="changelog-release-date">
                          {formatPublishedAt(release.publishedAt) ?? 'Undated'}
                        </span>
                      </button>
                    ))
                  ) : (
                    <p className="hint">No release history available.</p>
                  )}
                </div>
                <div className="changelog-sidebar-actions">
                  <button
                    className="action-btn changelog-nav-btn"
                    disabled={changelogPage === 0}
                    onClick={() => setChangelogPage((current) => Math.max(0, current - 1))}
                    type="button"
                  >
                    <span className="btn-label">Previous</span>
                  </button>
                  <button
                    className="action-btn changelog-nav-btn"
                    disabled={changelogPage >= changelogPageCount - 1}
                    onClick={() =>
                      setChangelogPage((current) => Math.min(changelogPageCount - 1, current + 1))
                    }
                    type="button"
                  >
                    <span className="btn-label">Next</span>
                  </button>
                </div>
              </div>
              <div className="changelog-content">
                <div className="changelog-markdown">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {getMarkdownContent(selectedRelease?.notes ?? null)}
                  </ReactMarkdown>
                </div>
              </div>
            </div>
            <div className="update-modal-actions changelog-footer">
              <button className="action-btn changelog-close-btn" onClick={() => setChangelogModal(null)} type="button">
                <span className="btn-label">Close</span>
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </main>
  );
}

export default App;
