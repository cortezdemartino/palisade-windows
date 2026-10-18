'use strict';
const path = require('node:path');
const { readFile } = require('node:fs/promises');
const { BrowserWindow, WebContentsView, ipcMain, protocol } = require('electron');
protocol.registerSchemesAsPrivileged([{ scheme: 'palisade-controls', privileges: { standard: true, secure: true } }]);
const CONTROLS_URL = 'palisade-controls://app/window-chrome.html';
function installControlsProtocol(session) {
  const types = { '/window-chrome.html': 'text/html', '/window-chrome.css': 'text/css', '/window-chrome.js': 'text/javascript' };
  session.protocol.handle('palisade-controls', async (request) => {
    const url = new URL(request.url);
    const type = types[url.pathname];
    if (request.method !== 'GET' || url.host !== 'app' || url.username || url.password || url.search || !type) {
      return new Response(null, { status: 404 });
    }
    try {
      return new Response(await readFile(path.join(__dirname, url.pathname.slice(1))), {
        headers: { 'Content-Type': type + '; charset=utf-8', 'X-Content-Type-Options': 'nosniff', 'Cache-Control': 'no-store' },
      });
    } catch { return new Response(null, { status: 404 }); }
  });
}
const CONTROLS_WIDTH = 136;
const CONTROLS_HEIGHT = 32;
const CONTROLS_STRIP_HEIGHT = 36;
const CONTROL_CHANNEL = 'palisade:window-control';
const STATE_CHANNEL = 'palisade:window-state';
const SECURE_WEB_PREFERENCES = Object.freeze({
  sandbox: true, contextIsolation: true, nodeIntegration: false,
  nodeIntegrationInWorker: false, webSecurity: true, allowRunningInsecureContent: false,
  webviewTag: false, spellcheck: false, devTools: false,
});

function lockLocalContents(contents) {
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
  contents.on('will-navigate', (event) => event.preventDefault());
  contents.on('will-redirect', (event) => event.preventDefault());
  contents.session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  contents.session.setPermissionCheckHandler(() => false);
  contents.session.setDevicePermissionHandler(() => false);
}

function createDesktopWindow({ partition, icon, onSettings = () => ({}), onDeviceAction = () => {} }) {
  const window = new BrowserWindow({
    title: 'Palisade', width: 1240, height: 780, minWidth: 780, minHeight: 540,
    backgroundColor: '#0d0f14', icon, show: false, autoHideMenuBar: true,
    frame: false, titleBarStyle: 'hidden',
    webPreferences: { ...SECURE_WEB_PREFERENCES, partition: 'palisade-window-chrome', javascript: false },
  });
  const view = new WebContentsView({ webPreferences: {
    ...SECURE_WEB_PREFERENCES, partition, javascript: true, backgroundThrottling: false,
    preload: path.join(__dirname, 'chat-notifications-preload.cjs'),
  } });
  const controls = new WebContentsView({ webPreferences: {
    ...SECURE_WEB_PREFERENCES, partition: 'palisade-window-controls', javascript: true,
    preload: path.join(__dirname, 'window-controls-preload.cjs'),
  } });
  view.setBackgroundColor('#0d0f14');
  controls.setBackgroundColor('#00000000');
  window.contentView.addChildView(view);
  window.contentView.addChildView(controls);
  const controlsUrl = CONTROLS_URL;
  installControlsProtocol(window.webContents.session);
  installControlsProtocol(controls.webContents.session);
  let settingsOpen = false;
  const layout = () => {
    const [width, height] = window.getContentSize();
    // Reserve desktop controls space independently of the hosted app layout.
    const top = window.isFullScreen() ? 0 : CONTROLS_STRIP_HEIGHT;
    view.setBounds({ x: 0, y: top, width, height: Math.max(0, height - top) });
    const panelWidth = settingsOpen ? 400 : CONTROLS_WIDTH;
    controls.setBounds({ x: Math.max(0, width - panelWidth - 18), y: 4, width: panelWidth, height: settingsOpen ? Math.min(560, height - 8) : CONTROLS_HEIGHT });
    controls.setVisible(!window.isFullScreen());
  };
  const publishState = () => {
    if (!controls.webContents.isDestroyed()) controls.webContents.send(STATE_CHANNEL, { ...onSettings(window), maximized: window.isMaximized(), settingsOpen });
  };
  const onControl = (event, action) => {
    // No commands are accepted from the remote page, subframes, or a navigated local view.
    if (window.isDestroyed() || event.sender !== controls.webContents ||
        event.senderFrame !== controls.webContents.mainFrame || event.senderFrame?.url !== controlsUrl ||
        controls.webContents.getURL() !== controlsUrl) return;
    if (action === 'settings') { settingsOpen = !settingsOpen; layout(); publishState(); }
    else if (['tray-on', 'tray-off', 'login-on', 'login-off', 'notification-test', 'updates', 'notification-settings', 'microphone-settings', 'copy-diagnostics', 'reload-client', 'check-updates'].includes(action)) {
      onDeviceAction(action); publishState();
    }
    else if (action === 'minimize') window.minimize();
    else if (action === 'maximize') {
      if (window.isMaximized()) window.unmaximize(); else window.maximize();
    } else if (action === 'close') window.close();
  };
  ipcMain.on(CONTROL_CHANNEL, onControl);
  controls.webContents.on('did-finish-load', publishState);
  window.on('maximize', publishState);
  window.on('unmaximize', publishState);
  layout();
  window.on('resize', layout);
  window.on('enter-full-screen', layout);
  window.on('leave-full-screen', layout);
  window.on('closed', () => {
    ipcMain.removeListener(CONTROL_CHANNEL, onControl);
    for (const child of [view, controls]) if (!child.webContents.isDestroyed()) child.webContents.close();
  });
  lockLocalContents(window.webContents);
  lockLocalContents(controls.webContents);
  // Only the local empty strip is draggable; hosted page elements remain interactive.
  void window.loadURL('data:text/html;charset=utf-8,' + encodeURIComponent('<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src &#39;none&#39;; style-src &#39;unsafe-inline&#39;"><style>html,body{margin:0;width:100%;height:36px;background:#0d0f14;app-region:drag;-webkit-app-region:drag}</style>'));
  void controls.webContents.loadURL(controlsUrl);
  return { window, contents: view.webContents, refreshSettings: publishState };
}
module.exports = { createDesktopWindow, CONTROLS_WIDTH, CONTROLS_HEIGHT };
