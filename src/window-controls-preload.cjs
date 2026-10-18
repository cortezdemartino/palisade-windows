'use strict';
const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('desktopControls', {
  settings: () => ipcRenderer.send('palisade:window-control', 'settings'),
  setTray: (enabled) => { if (typeof enabled === 'boolean') ipcRenderer.send('palisade:window-control', enabled ? 'tray-on' : 'tray-off'); },
  setLogin: (enabled) => { if (typeof enabled === 'boolean') ipcRenderer.send('palisade:window-control', enabled ? 'login-on' : 'login-off'); },
  testNotification: () => ipcRenderer.send('palisade:window-control', 'notification-test'),
  notificationSettings: () => ipcRenderer.send('palisade:window-control', 'notification-settings'),
  microphoneSettings: () => ipcRenderer.send('palisade:window-control', 'microphone-settings'),
  copyDiagnostics: () => ipcRenderer.send('palisade:window-control', 'copy-diagnostics'),
  reloadClient: () => ipcRenderer.send('palisade:window-control', 'reload-client'),
  openUpdates: () => ipcRenderer.send('palisade:window-control', 'updates'),
  minimize: () => ipcRenderer.send('palisade:window-control', 'minimize'),
  maximize: () => ipcRenderer.send('palisade:window-control', 'maximize'),
  close: () => ipcRenderer.send('palisade:window-control', 'close'),
  onState: (callback) => {
    if (typeof callback !== 'function') return () => {};
    const listener = (_event, state) => {
      if (typeof state?.maximized === 'boolean') callback({ maximized: state.maximized, settingsOpen: state.settingsOpen === true,
        keepRunningInTray: state.keepRunningInTray === true, launchOnLogin: state.launchOnLogin === true,
        loginAvailable: state.loginAvailable === true, version: typeof state.version === 'string' ? state.version.slice(0, 64) : '',
        updateStatus: typeof state.updateStatus === 'string' ? state.updateStatus.slice(0, 160) : '' });
    };
    ipcRenderer.on('palisade:window-state', listener);
    return () => ipcRenderer.removeListener('palisade:window-state', listener);
  },
});
