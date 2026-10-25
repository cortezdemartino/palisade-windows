'use strict';

const { app, desktopCapturer, powerMonitor, screen, clipboard, dialog, Menu, nativeImage, Notification, session, shell, Tray } = require('electron');
const path = require('node:path');
const { APP_ID, ensureNotificationBranding } = require('./windows-notification-branding.cjs');
const { createUpdateController } = require('./updates/updater.cjs');
const { loadSettings, saveSettings } = require('./settings.cjs');
const { installSessionPolicy } = require('./session-policy.cjs');
const { installAssistBridge } = require('./assist/bridge.cjs');
const { createCaptureController } = require('./assist/capture.cjs');
const { chooseSource } = require('./assist/source-picker.cjs');
const { captureEnabled } = require('./assist/build-policy.cjs');
const { installChatNotifications } = require('./chat-notifications.cjs');
const { createDesktopWindow } = require('./desktop-window.cjs');
const {
  PALISADE_URL,
  isPalisadeUrl,
  normaliseExternalUrl,
} = require('./security-policy.cjs');

// The hosted client has an unread observer and the canonical description-only Assist v1 API.
// Only the isolated local window-controls view has a narrowly scoped control bridge.
// The web client remains responsible for Matrix auth, Rust cryptography, and E2EE.
const PARTITION = 'persist:palisade-desktop';
const ICON = path.join(__dirname, '..', 'assets', 'icon.ico');
let window = null;
let mainContents = null;
let assistCapture = null;
let assistBridge = null;
let refreshDeviceSettings = null;
let trayNoticeShown = false;
const activeNotifications = new Set();
let tray = null;
let keepRunningInTray = false;
let quitting = false;
let lastExternalPromptAt = 0;
let updaterController = null;
let updateStatus = 'Updates: idle';
let notificationBranding = Promise.resolve();
let notificationStatus = 'No desktop notifications sent yet.';

app.setName('Palisade');
if (process.platform === 'win32') app.setAppUserModelId(APP_ID);

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (!window) return;
    window.show();
    if (window.isMinimized()) window.restore();
    window.focus();
    mainContents?.focus();
  });

  app.on('before-quit', () => {
    quitting = true;
    session.fromPartition(PARTITION).flushStorageData();
  });

  app.on('web-contents-created', (_event, webContents) => {
    // Remote HTML must never be able to embed a privileged Electron webview.
    webContents.on('will-attach-webview', (event) => event.preventDefault());
  });

  app.whenReady().then(() => {
    notificationBranding = ensureNotificationBranding({ packaged: app.isPackaged }).then(() => true).catch(() => {
      notificationStatus = 'Windows notification branding could not be registered.';
      refreshDeviceSettings?.();
      return false;
    });
    keepRunningInTray = loadSettings(app.getPath('userData')).keepRunningInTray;
    const profile = session.fromPartition(PARTITION, { cache: true });
    createMainWindow(profile);
    createTray();
    updaterController = createUpdateController(() => window, (status) => {
      updateStatus = status;
      updateTrayMenu();
      createApplicationMenu();
      refreshDeviceSettings?.();
    }, { onAvailable: (manifest, openManager) => notifyDesktop(
      'Palisade ' + manifest.version + ' is available. Open the update manager to install it.', openManager) });
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

function createMainWindow(profile) {
  const icon = nativeImage.createFromPath(ICON);
  const desktop = createDesktopWindow({ partition: PARTITION, icon,
    onSettings: () => ({ keepRunningInTray, launchOnLogin: app.getLoginItemSettings().openAtLogin,
      loginAvailable: app.isPackaged, version: app.getVersion(), updateStatus, notificationStatus }),
    onDeviceAction: (action) => {
      try {
      if (action === 'tray-on' || action === 'tray-off') trayPreferenceItem().click({ checked: action === 'tray-on' });
      else if ((action === 'login-on' || action === 'login-off') && app.isPackaged) {
        app.setLoginItemSettings({ openAtLogin: action === 'login-on' }); createApplicationMenu();
      } else if (action === 'notification-test') notifyDesktop('Desktop notifications are available while Palisade is open or in the tray.');
      else if (action === 'notification-settings' || action === 'microphone-settings') {
        void shell.openExternal(action === 'notification-settings' ? 'ms-settings:notifications' : 'ms-settings:privacy-microphone')
          .catch(() => dialog.showMessageBox(window, { type: 'error', message: 'Windows settings could not be opened.', buttons: ['OK'] }));
      } else if (action === 'copy-diagnostics') {
        clipboard.writeText([
          'Palisade ' + app.getVersion(), 'Windows desktop · ' + process.platform + ' ' + process.arch,
          'Electron ' + process.versions.electron, 'Tray: ' + keepRunningInTray,
          'Launch on login: ' + app.getLoginItemSettings().openAtLogin, updateStatus,
        ].join('\n'));
      } else if (action === 'reload-client') mainContents?.reload();
      else if (action === 'updates') void updaterController?.openManager();
      else if (action === 'check-updates') void updaterController?.check(true);
      } catch {
        void dialog.showMessageBox(window, { type: 'error', message: 'The device setting could not be applied. Try again.', buttons: ['OK'] });
      }
    },
  });
  refreshDeviceSettings = desktop.refreshSettings;
  window = desktop.window;
  const contents = mainContents = desktop.contents;
  assistCapture = createCaptureController({ enabled: captureEnabled, getContents: () => contents, getWindow: () => window,
    enumerate: options => desktopCapturer.getSources(options), choose: chooseSource, onState: () => updateTrayMenu(),
    revoke: () => { if (!contents.isDestroyed()) { contents.forcefullyCrashRenderer(); contents.reload(); } } });
  installSessionPolicy(profile, () => mainContents);
  assistBridge = installAssistBridge(contents, { desktopVersion: app.getVersion() });
  for (const name of ['minimize', 'hide', 'close', 'unresponsive']) window.on(name, assistCapture.stop);
  contents.on('did-start-navigation', (_event, _url, inPlace, mainFrame) => { if (mainFrame && !inPlace) assistCapture?.stop(); });
  contents.on('render-process-gone', () => assistCapture?.stop());
  const revokeCapture = () => assistCapture?.stop();
  for (const name of ['lock-screen', 'suspend', 'shutdown']) powerMonitor.on(name, revokeCapture);
  for (const name of ['display-added', 'display-removed', 'display-metrics-changed']) screen.on(name, revokeCapture);
  window.once('closed', () => {
    assistBridge?.dispose(); assistCapture?.dispose();
    for (const name of ['lock-screen', 'suspend', 'shutdown']) powerMonitor.removeListener(name, revokeCapture);
    for (const name of ['display-added', 'display-removed', 'display-metrics-changed']) screen.removeListener(name, revokeCapture);
  });
  installChatNotifications(window, contents, { notify: notifyDesktop, openWindow, setUnreadIndicator: image => tray?.setImage(image) });
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

  window.once('ready-to-show', () => { window?.show(); mainContents?.focus(); });
  window.on('minimize', () => {
    if (keepRunningInTray) hideToTray();
  });
  window.on('close', (event) => {
    if (!quitting && keepRunningInTray) {
      event.preventDefault();
      hideToTray();
    }
  });
  window.on('closed', () => {
    window = null;
    mainContents = null;
    refreshDeviceSettings = null;
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
  mainContents?.focus();
}

async function notifyDesktop(body, onClick = openWindow) {
  try {
  if (await notificationBranding === false) return;
  if (!Notification.isSupported()) { notificationStatus = 'Desktop notifications are unavailable on this Windows session.'; refreshDeviceSettings?.(); return; }
  const notification = new Notification({ title: 'Palisade', body,
    icon: app.isPackaged ? path.join(process.resourcesPath, 'notification-icon.png') : ICON, silent: false });
  activeNotifications.add(notification);
  notification.on('click', onClick);
  notification.on('close', () => activeNotifications.delete(notification));
  notification.on('show', () => { notificationStatus = 'Windows accepted the last notification.'; refreshDeviceSettings?.(); });
  notification.on('failed', () => { activeNotifications.delete(notification); notificationStatus = 'Windows could not display the notification. Check Windows notification settings.'; refreshDeviceSettings?.(); });
  notification.show();
  } catch { notificationStatus = 'Windows could not create the notification.'; refreshDeviceSettings?.(); }
}

function hideToTray() {
  window?.hide();
  if (!trayNoticeShown) {
    trayNoticeShown = true;
    notifyDesktop('Palisade is still running in the tray. Messages and notifications remain available.');
  }
}

function createTray() {
  tray = new Tray(ICON);
  tray.setToolTip('Palisade');
  tray.on('double-click', openWindow);
  tray.on('click', openWindow);
  updateTrayMenu();
}

function trayPreferenceItem() {
  return {
  label: 'Minimize and close to tray',
  type: 'checkbox',
  checked: keepRunningInTray,
  click: (item) => {
    try {
      saveSettings(app.getPath('userData'), { keepRunningInTray: item.checked });
      keepRunningInTray = item.checked;
    } catch {
      void dialog.showMessageBox({ type: 'error', message: 'Could not save desktop settings.', buttons: ['OK'] });
    }
    updateTrayMenu();
    createApplicationMenu();
  },
      };
}

function updateTrayMenu() {
  if (!tray) return;
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Open Palisade', click: openWindow },
      { label: 'Stop screen sharing and reload Palisade', enabled: assistCapture?.busy() === true, click: () => assistCapture?.stop() },
      trayPreferenceItem(),
      { type: 'separator' },
      { label: 'Send test notification', click: () => notifyDesktop('Desktop notifications are available while Palisade is open or in the tray.') },
      { label: updateStatus, enabled: false },
      {
        label: 'Check for Updates', click: () => void updaterController?.check(true),
      },
      {
        label: 'Open Installer & Update Manager', click: () => void updaterController?.openManager(),
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
          { label: updateStatus, enabled: false },
          { label: 'Reload Palisade', accelerator: 'CmdOrCtrl+R', click: () => mainContents?.reload() },
          { label: 'Check for Updates', click: () => void updaterController?.check(true) },
          { label: 'Open Installer & Update Manager', click: () => void updaterController?.openManager() },
          trayPreferenceItem(),
          { label: 'Launch on login', type: 'checkbox', enabled: app.isPackaged,
            checked: app.getLoginItemSettings().openAtLogin,
            click: (item) => { app.setLoginItemSettings({ openAtLogin: item.checked }); createApplicationMenu(); } },
          { label: 'About Palisade', click: () => void dialog.showMessageBox({
            type: 'info', title: 'About Palisade', message: `Palisade ${app.getVersion()}`,
            detail: 'Windows x64 desktop client\nCopyright 2026 Onyx Operations Group\nApache-2.0. Palisade trademarks remain with their owner.', buttons: ['OK'],
          }) },
          { type: 'separator' },
          { label: 'Quit', accelerator: 'CmdOrCtrl+Q', click: () => app.quit() },
        ],
      },
      {
        label: 'Edit',
        submenu: [{ role: 'undo' }, { role: 'redo' }, { type: 'separator' }, { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }],
      },
    ]),
  );
}
