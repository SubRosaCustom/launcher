import { useEffect, useState } from 'react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { LauncherSettings } from '../types/launcher';

type SettingsView = 'settings' | 'launchSettings' | 'launcherSupport' | 'clientSupport';

interface SettingsPanelProps {
  open: boolean;
  saving: boolean;
  activeSupportAction: string | null;
  settings: LauncherSettings;
  detectedGameDir: string | null;
  executableCandidates: string[];
  onSave: (next: LauncherSettings) => Promise<void>;
  onClose: () => void;
  onOpenLauncherLogs: () => void;
  onOpenClientCrashlogsFolder: () => void;
  onOpenClientConfigFolder: () => void;
  onOpenCacheFolder: () => void;
  onForceRedownload: () => void;
  onClearCache: () => void;
  onCopyLauncherDiagnostics: () => void;
  onCopyClientDiagnostics: () => void;
}

function parentDirectory(path: string) {
  const slashIndex = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  if (slashIndex < 0) return null;
  if (slashIndex === 0) return path.slice(0, 1);
  if (slashIndex === 2 && /^[A-Za-z]:[\\/]/.test(path)) return path.slice(0, 3);
  return path.slice(0, slashIndex);
}

function SettingsPanel({
  open,
  saving,
  activeSupportAction,
  settings,
  detectedGameDir,
  executableCandidates,
  onSave,
  onClose,
  onOpenLauncherLogs,
  onOpenClientCrashlogsFolder,
  onOpenClientConfigFolder,
  onOpenCacheFolder,
  onForceRedownload,
  onClearCache,
  onCopyLauncherDiagnostics,
  onCopyClientDiagnostics,
}: SettingsPanelProps) {
  const [gameDir, setGameDir] = useState(settings.customGameDir ?? '');
  const [executableName, setExecutableName] = useState(settings.executableName);
  const [closeOnLaunch, setCloseOnLaunch] = useState(settings.closeOnLaunch);
  const [view, setView] = useState<SettingsView>('settings');

  useEffect(() => {
    if (!open) return;
    setGameDir(settings.customGameDir ?? '');
    setExecutableName(settings.executableName);
    setCloseOnLaunch(settings.closeOnLaunch);
    setView('settings');
  }, [open, settings.customGameDir, settings.executableName, settings.closeOnLaunch]);

  if (!open) return null;

  const handleBack = () => {
    if (view !== 'settings') {
      setView('settings');
      return;
    }
    onClose();
  };

  const handleSave = () =>
    onSave({
      customGameDir: gameDir.trim() || null,
      executableName: executableName.trim() || settings.executableName,
      closeOnLaunch,
    });

  const handleSelectGameDir = async () => {
    const selected = await openDialog({
      title: 'Select Sub Rosa folder',
      directory: true,
      multiple: false,
      defaultPath: gameDir.trim() || detectedGameDir || undefined,
    });
    if (selected) {
      setGameDir(selected);
    }
  };

  const handleSelectExecutable = async () => {
    const selected = await openDialog({
      title: 'Select Sub Rosa executable',
      multiple: false,
      defaultPath: executableName.trim() || gameDir.trim() || detectedGameDir || undefined,
    });
    if (selected) {
      setExecutableName(selected);
      setGameDir((current) => current.trim() || parentDirectory(selected) || current);
    }
  };

  return (
    <div className="settings-page">
      <div className="settings-shell">
        <div className="settings-top">
          <button className="action-btn settings-back-btn" onClick={handleBack}>
            <span className="btn-label">{view !== 'settings' ? 'Back to settings' : 'Back'}</span>
          </button>
        </div>

        <div className="settings-body">
          {view === 'settings' ? (
            <section className="settings-section">
              <div className="support-grid">
                <button
                  type="button"
                  className="action-btn support-btn"
                  onClick={() => setView('launchSettings')}
                >
                  <span className="btn-label">Open launch settings</span>
                </button>
                <button
                  type="button"
                  className="action-btn support-btn"
                  onClick={() => setView('launcherSupport')}
                >
                  <span className="btn-label">Open launcher helpers</span>
                </button>
                <button
                  type="button"
                  className="action-btn support-btn"
                  onClick={() => setView('clientSupport')}
                >
                  <span className="btn-label">Open client helpers</span>
                </button>
              </div>
            </section>
          ) : view === 'launchSettings' ? (
            <section className="settings-section">
              <p className="helper-title">launch settings</p>
              <div className="settings-row">
                <input
                  id="gameDir"
                  value={gameDir}
                  onChange={(e) => setGameDir(e.currentTarget.value)}
                  placeholder={detectedGameDir ?? 'Sub Rosa folder'}
                  autoFocus
                />
                <label htmlFor="gameDir">Game folder</label>
                <button type="button" className="action-btn path-picker-btn" onClick={handleSelectGameDir}>
                  <span className="btn-label">Browse</span>
                </button>
              </div>
              <div className="hint">Detected: {detectedGameDir ?? 'not found'}</div>
              <div className="settings-row">
                <input
                  id="exeName"
                  value={executableName}
                  onChange={(e) => setExecutableName(e.currentTarget.value)}
                  placeholder="subrosa.x64 or full executable path"
                />
                <label htmlFor="exeName">Executable</label>
                <button type="button" className="action-btn path-picker-btn" onClick={handleSelectExecutable}>
                  <span className="btn-label">Browse</span>
                </button>
              </div>
              <div className="hint">
                Suggested: {executableCandidates.length > 0 ? executableCandidates.join(', ') : 'none'}
              </div>
              <div className="settings-row">
                <button
                  type="button"
                  className={`toggle-btn ${closeOnLaunch ? 'is-on' : ''}`}
                  onClick={() => setCloseOnLaunch((v) => !v)}
                >
                  <span className="toggle-box">
                    <span className="toggle-fill" />
                  </span>
                  <span>Close after success</span>
                </button>
              </div>
            </section>
          ) : (
            <section className="settings-section">
              <p className="helper-title">
                {view === 'launcherSupport' ? 'launcher helpers' : 'client crashlogs'}
              </p>
              {view === 'launcherSupport' ? (
                <>
                  <div className="support-grid">
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onOpenLauncherLogs}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'openLauncherLogs' ? 'Working...' : 'Open logs'}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onOpenCacheFolder}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'openCacheFolder'
                          ? 'Working...'
                          : 'Open cache folder'}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onForceRedownload}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'forceRedownload'
                          ? 'Working...'
                          : 'Force redownload'}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onClearCache}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'clearCache' ? 'Working...' : 'Clear cache'}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onCopyLauncherDiagnostics}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'copyLauncherDiagnostics'
                          ? 'Working...'
                          : 'Copy diagnostics'}
                      </span>
                    </button>
                  </div>
                </>
              ) : (
                <>
                  <div className="support-grid">
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onOpenClientCrashlogsFolder}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'openClientCrashlogsFolder'
                          ? 'Working...'
                          : 'Open crashlogs'}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onOpenClientConfigFolder}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'openClientConfigFolder'
                          ? 'Working...'
                          : 'Open config folder'}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="action-btn support-btn"
                      onClick={onCopyClientDiagnostics}
                      disabled={activeSupportAction !== null}
                    >
                      <span className="btn-label">
                        {activeSupportAction === 'copyClientDiagnostics'
                          ? 'Working...'
                          : 'Copy diagnostics'}
                      </span>
                    </button>
                  </div>
                </>
              )}
            </section>
          )}
        </div>

        <div className="settings-actions">
          {view === 'launchSettings' ? (
            <button className="action-btn settings-save-btn" onClick={handleSave} disabled={saving}>
              <span className="btn-label">{saving ? 'Saving...' : 'Save'}</span>
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
}

export default SettingsPanel;
