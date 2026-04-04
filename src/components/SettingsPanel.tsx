import { useEffect, useState } from 'react';
import type { LauncherSettings } from '../types/launcher';

interface SettingsPanelProps {
  open: boolean;
  saving: boolean;
  activeSupportAction: string | null;
  settings: LauncherSettings;
  executableCandidates: string[];
  onSave: (next: LauncherSettings) => Promise<void>;
  onClose: () => void;
  onOpenLogs: () => void;
  onOpenCacheFolder: () => void;
  onForceRedownload: () => void;
  onClearCache: () => void;
  onCopyDiagnostics: () => void;
}

function SettingsPanel({
  open,
  saving,
  activeSupportAction,
  settings,
  executableCandidates,
  onSave,
  onClose,
  onOpenLogs,
  onOpenCacheFolder,
  onForceRedownload,
  onClearCache,
  onCopyDiagnostics,
}: SettingsPanelProps) {
  const [executableName, setExecutableName] = useState(settings.executableName);
  const [closeOnLaunch, setCloseOnLaunch] = useState(settings.closeOnLaunch);

  useEffect(() => {
    if (!open) return;
    setExecutableName(settings.executableName);
    setCloseOnLaunch(settings.closeOnLaunch);
  }, [open, settings.executableName, settings.closeOnLaunch]);

  if (!open) return null;

  return (
    <div className="settings-page">
      <div className="settings-top">
        <button className="action-btn" style={{ height: '30px' }} onClick={onClose}>
          <span className="btn-label">Back</span>
        </button>
      </div>
      <div className="settings-body">
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
              onClick={onOpenLogs}
              disabled={activeSupportAction !== null}
            >
              <span className="btn-label">
                {activeSupportAction === 'openLogs' ? 'Working...' : 'Open logs'}
              </span>
            </button>
            <button
              type="button"
              className="action-btn support-btn"
              onClick={onOpenCacheFolder}
              disabled={activeSupportAction !== null}
            >
              <span className="btn-label">
                {activeSupportAction === 'openCacheFolder' ? 'Working...' : 'Open cache folder'}
              </span>
            </button>
            <button
              type="button"
              className="action-btn support-btn"
              onClick={onForceRedownload}
              disabled={activeSupportAction !== null}
            >
              <span className="btn-label">
                {activeSupportAction === 'forceRedownload' ? 'Working...' : 'Force redownload'}
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
              onClick={onCopyDiagnostics}
              disabled={activeSupportAction !== null}
            >
              <span className="btn-label">
                {activeSupportAction === 'copyDiagnostics' ? 'Working...' : 'Copy diagnostics'}
              </span>
            </button>
          </div>
        </section>
      </div>
      <div className="settings-actions">
        <button
          className="action-btn"
          style={{ height: '40px' }}
          onClick={() =>
            onSave({
              executableName: executableName.trim() || settings.executableName,
              closeOnLaunch,
            })
          }
          disabled={saving}
        >
          <span className="btn-label">{saving ? 'Saving...' : 'Save'}</span>
        </button>
      </div>
    </div>
  );
}

export default SettingsPanel;
