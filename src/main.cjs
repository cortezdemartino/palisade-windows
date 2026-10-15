'use strict';

const { app, BrowserWindow, dialog, Menu, nativeImage, session, shell, Tray } = require('electron');
const path = require('node:path');
const { createUpdateController } = require('./updates/updater.cjs');
const {
  PALISADE_URL,
  PALISADE_ORIGIN,
  isPalisadeUrl,
  normaliseExternalUrl,
  mayGrantPermission,
} = require('./security-policy.cjs');

// There is intentionally no preload, IPC interface, Node bridge, or local HTTP server.
// The web client remains responsible for Matrix auth, Rust cryptography, and E2EE.
const PARTITION = 'persist:palisade-desktop';
const ICON = path.join(__dirname, '..', 'assets', 'icon.ico');
let window = null;
let tray = null;
let keepRunningInTray = false;
let quitting = false;
let lastExternalPromptAt = 0;
let updaterController = null;

app.setName('Palisade');
if (process.platform === 'win32') app.setAppUserModelId('org.onyxops.palisade');

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (!window) return;
    window.show();
    if (window.isMinimized()) window.restore();
    window.focus();
  });

  app.on('before-quit', () => {
    quitting = true;
  });

  app.on('web-contents-created', (_event, webContents) => {
    // Remote HTML must never be able to embed a privileged Electron webview.
    webContents.on('will-attach-webview', (event) => event.preventDefault());
  });

  app.whenReady().then(() => {
    const profile = session.fromPartition(PARTITION, { cache: true });
    installSessionPolicy(profile);
    createMainWindow(profile);
    createTray();
    updaterController = createUpdateController(() => window);
    createApplicationMenu();
    updaterController.start();
  });

  app.on('activate', () => {
    if (window) window.show();
  });

  app.on('window-all-closed', () => {
    if (!keepRunningInTray) app.quit();
  });
}

function isMainContents(webContents) {
  return Boolean(window && webContents === window.webContents);
}

function trustedRequester(webContents, details) {
  if (!isMainContents(webContents) || !isPalisadeUrl(webContents.getURL())) return false;
  // Electron may supply one or both fields depending on the permission type.
  const requestedFrom = details?.requestingUrl || details?.securityOrigin;
  return requestedFrom ? isPalisadeUrl(requestedFrom) : true;
}

function installSessionPolicy(profile) {
  // A frame navigated by script cannot escape into an arbitrary top-level website.
  profile.webRequest.onBeforeRequest({ urls: ['*://*/*'] }, (details, callback) => {
    callback({ cancel: details.resourceType === 'mainFrame' && !isPalisadeUrl(details.url) });
  });

  profile.setPermissionRequestHandler((webContents, permission, callback, details) => {
    callback(
      trustedRequester(webContents, details) &&
        mayGrantPermission(permission, webContents.getURL(), details?.mediaTypes),
    );
  });
  profile.setPermissionCheckHandler((webContents, permission, requestingOrigin, details) => {
    if (!trustedRequester(webContents, details)) return false;
    if (requestingOrigin && !isPalisadeUrl(requestingOrigin)) return false;
    // The media request handler above checks audio-only mediaTypes. Permission checks
    // don't consistently carry mediaTypes, so deny every *other* device capability.
    return permission === 'media' || permission === 'notifications';
  });
  profile.setDevicePermissionHandler(() => false);
}

function createMainWindow(profile) {
  const icon = nativeImage.createFromPath(ICON);
  window = new BrowserWindow({
    title: 'Palisade',
    width: 1240,
    height: 780,
    minWidth: 780,
    minHeight: 540,
    backgroundColor: '#0d0f14',
    icon,
    show: false,
    autoHideMenuBar: true,
    webPreferences: {
      partition: PARTITION,
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInWorker: false,
      webSecurity: true,
      allowRunningInsecureContent: false,
      webviewTag: false,
      javascript: true,
      spellcheck: false,
      devTools: false,
      backgroundThrottling: true,
    },
  });

  const contents = window.webContents;
  contents.setWindowOpenHandler(({ url }) => {
    if (isPalisadeUrl(url)) {
      // In-app enrollment and verification links remain in the one trusted window.
      void contents.loadURL(url);
    } else {
      void askToOpenExternal(url);
    }
    return { action: 'deny' };
  });
  contents.on('will-navigate', (event, url) => {
    if (!isPalisadeUrl(url)) {
      event.preventDefault();
      void askToOpenExternal(url);
    }
  });
  contents.on('will-redirect', (event, url) => {
    if (!isPalisadeUrl(url)) event.preventDefault();
  });
  contents.on('did-fail-load', (_event, code, _description, url, isMainFrame) => {
    // ERR_ABORTED is normal for redirects and canceled loads.
    if (isMainFrame && code !== -3 && isPalisadeUrl(url) && window && !window.isDestroyed()) {
      void dialog.showMessageBox(window, {
        type: 'warning',
        title: 'Palisade is unavailable',
        message: 'Could not load the secure Palisade service.',
        detail: 'Check your internet connection and the service status, then use Ctrl+R to retry.',
        buttons: ['OK'],
        noLink: true,
      });
    }
  });

  window.once('ready-to-show', () => window?.show());
  window.on('close', (event) => {
    if (!quitting && keepRunningInTray) {
      event.preventDefault();
      window?.hide();
    }
  });
  window.on('closed', () => {
    window = null;
  });

  // No production override or insecure localhost URL is allowed by this build.
  void contents.loadURL(PALISADE_URL);
}

async function askToOpenExternal(value) {
  const url = normaliseExternalUrl(value);
  const sourceWindow = window;
  if (!url || !sourceWindow || sourceWindow.isDestroyed()) return;
  // Avoid remote-script-driven prompt storms; opening always needs user approval.
  const now = Date.now();
  if (now - lastExternalPromptAt < 1000) return;
  lastExternalPromptAt = now;
  const result = await dialog.showMessageBox(sourceWindow, {
    type: 'question',
    title: 'Open external website?',
    message: 'Leave Palisade to open this website in your default browser?',
    detail: url,
    buttons: ['Open in browser', 'Cancel'],
    defaultId: 1,
    cancelId: 1,
    noLink: true,
  });
  if (result.response === 0 && !sourceWindow.isDestroyed()) {
    await shell.openExternal(url);
  }
}

function openWindow() {
  if (!window) return;
  if (window.isMinimized()) window.restore();
  window.show();
  window.focus();
}

function createTray() {
  tray = new Tray(ICON);
  tray.setToolTip('Palisade');
  tray.on('double-click', openWindow);
  tray.on('click', openWindow);
  updateTrayMenu();
}

function updateTrayMenu() {
  if (!tray) return;
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Open Palisade', click: openWindow },
      {
        label: 'Keep running when window is closed',
        type: 'checkbox',
        checked: keepRunningInTray,
        click: (item) => {
          keepRunningInTray = item.checked;
          updateTrayMenu();
        },
      },
      { type: 'separator' },
      {
        label: 'Check for updates', click: () => void updaterController?.check(true),
      },
      { type: 'separator' },
      {
        label: 'Quit Palisade',
        click: () => {
          quitting = true;
          app.quit();
        },
      },
    ]),
  );
}

function createApplicationMenu() {
  Menu.setApplicationMenu(
    Menu.buildFromTemplate([
      {
        label: 'Palisade',
        submenu: [
          { label: 'Reload Palisade', accelerator: 'CmdOrCtrl+R', click: () => window?.webContents.reload() },
          { label: 'Check for updates', click: () => void updaterController?.check(true) },
          { type: 'separator' },
          { label: 'Quit', accelerator: 'Alt+F4', click: () => app.quit() },
        ],
      },
      {
        label: 'Edit',
        submenu: [{ role: 'undo' }, { role: 'redo' }, { type: 'separator' }, { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }],
      },
    ]),
  );
}
