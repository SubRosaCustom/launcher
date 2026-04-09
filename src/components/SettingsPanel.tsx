import { useEffect, useState } from 'react';
import type { LauncherSettings } from '../types/launcher';

type SettingsView = 'settings' | 'launcherSupport' | 'clientSupport';

interface SettingsPanelProps {
  open: boolean;
  saving: boolean;
  activeSupportAction: string | null;
  settings: LauncherSettings;
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

function SettingsPanel({
  open,
  saving,
  activeSupportAction,
  settings,
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
  const [executableName, setExecutableName] = useState(settings.executableName);
  const [closeOnLaunch, setCloseOnLaunch] = useState(settings.closeOnLaunch);
  const [view, setView] = useState<SettingsView>('settings');

  useEffect(() => {
    if (!open) return;
    setExecutableName(settings.executableName);
    setCloseOnLaunch(settings.closeOnLaunch);
    setView('settings');
  }, [open, settings.executableName, settings.closeOnLaunch]);

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
      executableName: executableName.trim() || settings.executableName,
      closeOnLaunch,
    });

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
            <>
              <section className="settings-section">
                <div className="settings-row">
                  <input
                    id="exeName"
                    value={executableName}
                    onChange={(e) => setExecutableName(e.currentTarget.value)}
                    placeholder="subrosa.x64.exe"
                    autoFocus
                  />
                  <label htmlFor="exeName">Executable name</label>
                </div>
                <div className="hint">Suggested: {executableCandidates.join(', ')}</div>
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

              <section className="settings-section">
                <div className="support-grid">
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
            </>
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
          {view === 'settings' ? (
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
