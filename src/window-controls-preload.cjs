'use strict';
const { contextBridge, ipcRenderer } = require('electron');
function boundedNotes(value) {
  if (!Array.isArray(value) || value.length > 12) return [];
  const string = (s, max) => typeof s === 'string' && s.length <= max;
  return value.filter((note) => note && string(note.version, 64) && string(note.publishedAt, 24) &&
    string(note.locale, 12) && string(note.title, 160) && string(note.summary, 600) &&
    Array.isArray(note.sections) && note.sections.length <= 5 && note.sections.every((section) =>
      section && string(section.title, 32) && Array.isArray(section.items) && section.items.length <= 12 &&
      section.items.every((item) => string(item, 600))) && Array.isArray(note.compatibility) &&
    note.compatibility.length <= 6 && note.compatibility.every((item) => string(item, 400)))
    .map((note) => ({ version: note.version, publishedAt: note.publishedAt, locale: note.locale,
      title: note.title, summary: note.summary,
      sections: note.sections.map((section) => ({ title: section.title, items: [...section.items] })),
      compatibility: [...note.compatibility] }));
}
contextBridge.exposeInMainWorld('desktopControls', {
  settings: () => ipcRenderer.send('palisade:window-control', 'settings'),
  setTray: (enabled) => { if (typeof enabled === 'boolean') ipcRenderer.send('palisade:window-control', enabled ? 'tray-on' : 'tray-off'); },
  setLogin: (enabled) => { if (typeof enabled === 'boolean') ipcRenderer.send('palisade:window-control', enabled ? 'login-on' : 'login-off'); },
  testNotification: () => ipcRenderer.send('palisade:window-control', 'notification-test'),
  notificationSettings: () => ipcRenderer.send('palisade:window-control', 'notification-settings'),
  microphoneSettings: () => ipcRenderer.send('palisade:window-control', 'microphone-settings'),
  copyDiagnostics: () => ipcRenderer.send('palisade:window-control', 'copy-diagnostics'),
  reloadClient: () => ipcRenderer.send('palisade:window-control', 'reload-client'),
  checkForUpdates: () => ipcRenderer.send('palisade:window-control', 'check-updates'),
  openUpdates: () => ipcRenderer.send('palisade:window-control', 'updates'),
  downloadUpdate: () => ipcRenderer.send('palisade:window-control', 'update-download'),
  restartForUpdate: () => ipcRenderer.send('palisade:window-control', 'update-restart'),
  cancelUpdate: () => ipcRenderer.send('palisade:window-control', 'update-cancel'),
  setUpdatesPaused: (paused) => { if (typeof paused === 'boolean') ipcRenderer.send('palisade:window-control', paused ? 'update-snooze' : 'update-resume'); },
  setUpdatePreference: (name, enabled) => {
    if (['auto-check', 'auto-download', 'install-on-quit'].includes(name) && typeof enabled === 'boolean')
      ipcRenderer.send('palisade:window-control', 'update-' + name + (enabled ? '-on' : '-off'));
  },
  minimize: () => ipcRenderer.send('palisade:window-control', 'minimize'),
  maximize: () => ipcRenderer.send('palisade:window-control', 'maximize'),
  close: () => ipcRenderer.send('palisade:window-control', 'close'),
  onState: (callback) => {
    if (typeof callback !== 'function') return () => {};
    const listener = (_event, state) => {
      if (typeof state?.maximized === 'boolean') callback({ maximized: state.maximized, settingsOpen: state.settingsOpen === true,
        keepRunningInTray: state.keepRunningInTray === true, launchOnLogin: state.launchOnLogin === true,
        loginAvailable: state.loginAvailable === true, version: typeof state.version === 'string' ? state.version.slice(0, 64) : '',
        nativeAssistStatus: typeof state.nativeAssistStatus === 'string' ? state.nativeAssistStatus.slice(0, 200) : 'Native endpoint unavailable',
        notificationStatus: typeof state.notificationStatus === 'string' ? state.notificationStatus.slice(0, 200) : '',
        updateStatus: typeof state.updateStatus === 'string' ? state.updateStatus.slice(0, 160) : '',
        latestVersion: typeof state.latestVersion === 'string' ? state.latestVersion.slice(0, 64) : '',
        notesStatus: typeof state.notesStatus === 'string' ? state.notesStatus.slice(0, 200) : '',
        releaseHistory: boundedNotes(state.releaseHistory), settingsOpenTo: state.settingsOpenTo === 'about' ? 'about' : '',
        updaterAvailable: state.updaterAvailable === true, updateBusy: state.updateBusy === true,
        updatePhase: typeof state.updatePhase === 'string' ? state.updatePhase.slice(0, 32) : '',
        updateProgress: Number.isInteger(state.updateProgress) && state.updateProgress >= 0 && state.updateProgress <= 100 ? state.updateProgress : 0,
        updateCancelable: state.updateCancelable === true, updateCanDownload: state.updateCanDownload === true,
        updateCanRestart: state.updateCanRestart === true, autoCheck: state.autoCheck === true,
        autoDownload: state.autoDownload === true, installOnQuit: state.installOnQuit === true,
        updatesPaused: state.updatesPaused === true,
        lastUpdateCheck: Number.isSafeInteger(state.lastUpdateCheck) && state.lastUpdateCheck > 0 &&
          state.lastUpdateCheck <= 8640000000000000 ? state.lastUpdateCheck : 0,
        updateHistory: Array.isArray(state.updateHistory) ? state.updateHistory.slice(0, 12).filter(item =>
          typeof item?.version === 'string' && item.version.length <= 64 && Number.isSafeInteger(item.at) &&
          item.at > 0 && item.at <= 8640000000000000 && ['installed', 'interrupted'].includes(item.result))
          .map(item => ({ version: item.version, at: item.at, result: item.result })) : [] });
    };
    ipcRenderer.on('palisade:window-state', listener);
    return () => ipcRenderer.removeListener('palisade:window-state', listener);
  },
});
