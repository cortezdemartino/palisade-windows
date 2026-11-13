'use strict';

const { app, desktopCapturer, powerMonitor, screen, clipboard, dialog, Menu, nativeImage, Notification, session, shell, Tray } = require('electron');
const path = require('node:path');
const { APP_ID, ensureNotificationBranding } = require('./windows-notification-branding.cjs');
const { appDetails, ensureAppRegistration } = require('./windows-app-registration.cjs');
const { createUpdateController } = require('./updates/updater.cjs');
const { createNativeHelper } = require('./updates/native-helper.cjs');
const { loadSettings, saveSettings } = require('./settings.cjs');
const { installSessionPolicy } = require('./session-policy.cjs');
const { installAssistBridge } = require('./assist/bridge.cjs');
const { createCaptureController } = require('./assist/capture.cjs');
const { chooseSource } = require('./assist/source-picker.cjs');
const { captureEnabled, nativeEndpointEnabled, qualification } = require('./assist/build-policy.cjs');
const { installNativeBridge } = require('./assist/native-bridge.cjs');
const { createNativeRuntime } = require('./assist/native-runtime.cjs');
const { packagedGuardianPaths } = require('./assist/guardian.cjs');
const { createAssistLifecycle, installQuitBarrier } = require('./assist/lifecycle.cjs');
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
const PARTITION = qualification ? 'persist:palisade-assist-qualification-' + qualification.namespace : 'persist:palisade-desktop';
const ICON = path.join(__dirname, '..', 'assets', 'icon.ico');
let window = null;
let mainContents = null;
let assistCapture = null;
let assistBridge = null;
let nativeAssistBridge = null;
let nativeAssistRuntime = null;
let nativeAssistStatus = 'Native endpoint unavailable';
let refreshDeviceSettings = null;
let showDeviceSettings = null;
const activeNotifications = new Set();
let tray = null;
let keepRunningInTray = false;
let desktopSettings;
let quitting = false;
let exitWaiting = false;
let lastExternalPromptAt = 0;
let updaterController = null;
let updateStatus = 'Updates: idle';
let notificationBranding = Promise.resolve();
let notificationStatus = 'No desktop notifications sent yet.';
const assistLifecycle = createAssistLifecycle();
// The live native coordinator must register its own retained child-exit holder
// before admitting authority. This holder covers the existing capture lifecycle.
assistLifecycle.hold({
  revoke: () => { assistCapture?.stop(); },
  wait: async () => { if (assistCapture?.busy()) throw Error('Capture cleanup incomplete'); },
});
async function prepareForExit(reason) {
  await assistLifecycle.quiesce(reason);
  if (app.isReady()) {
    const profile = session.fromPartition(PARTITION);
    profile.flushStorageData();
    await profile.cookies.flushStore();
  }
}

if (qualification) app.setPath('userData', path.join(app.getPath('appData'), 'Palisade-Assist-Qualification-' + qualification.namespace));
app.setName(qualification ? 'Palisade Assist Qualification' : 'Palisade');
// Development Electron must never take the installed client's shell identity.
if (process.platform === 'win32') app.setAppUserModelId(qualification ? APP_ID + '.qualification.' + qualification.namespace : app.isPackaged ? APP_ID : APP_ID + '.development');

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

  installQuitBarrier(app, {
    prepare: async reason => {
      await updaterController?.onQuit();
      await prepareForExit(reason);
    },
    onApproved: () => { exitWaiting = false; quitting = true; },
    onWaiting: () => {
      exitWaiting = true;
      tray?.setToolTip('Palisade: stopping assistance before exit');
      updateTrayMenu(); createApplicationMenu();
      void notifyDesktop('Palisade is staying open while assistance and file transfers finish stopping. It will exit when cleanup completes.');
    },
    onBlocked: () => {
      exitWaiting = false; quitting = false;
      tray?.setToolTip('Palisade');
      updateTrayMenu(); createApplicationMenu();
      return dialog.showMessageBox({ type: 'warning', title: 'Palisade is still open',
        message: 'Palisade could not safely finish preparing to exit.',
        detail: 'Assistance cleanup or profile saving has not finished. Keep Palisade open, then try Quit again.',
        buttons: ['Keep open'], noLink: true });
    },
  });

  app.on('web-contents-created', (_event, webContents) => {
    // Remote HTML must never be able to embed a privileged Electron webview.
    webContents.on('will-attach-webview', (event) => event.preventDefault());
  });

  app.whenReady().then(async () => {
    let native;
    if (!qualification && app.isPackaged && process.platform === 'win32') {
      const helper = createNativeHelper({ resources: process.resourcesPath, version: app.getVersion() });
      try {
        const status = await helper.status();
        if (status.state === 'updating') {
          // Leave promptly so the installer's running-client check can succeed.
          app.quit();
          return;
        }
        native = helper;
      } catch {
        updateStatus = 'Updates: built-in updater unavailable. Use Update Manager to repair.';
      }
    }
    void ensureAppRegistration({ packaged: app.isPackaged && !qualification, shell, appData: app.getPath('appData') }).catch(() => {
      notificationStatus = 'Windows app registration could not be repaired. Run the official installer to repair it.';
      refreshDeviceSettings?.();
    });
    notificationBranding = ensureNotificationBranding({ packaged: app.isPackaged && !qualification }).then(() => true).catch(() => {
      notificationStatus = 'Windows notification branding could not be registered.';
      refreshDeviceSettings?.();
      return false;
    });
    if (quitting) return;
    desktopSettings = loadSettings(app.getPath('userData'));
    keepRunningInTray = desktopSettings.keepRunningInTray;
    const profile = session.fromPartition(PARTITION, { cache: true });
    createMainWindow(profile, Boolean(native) || !app.isPackaged || Boolean(qualification));
    createTray();
    if (qualification) { updateStatus = 'Updates unavailable in qualification builds'; createApplicationMenu(); return; }
    updaterController = createUpdateController(() => window, (status) => {
      const changed = updateStatus !== status;
      updateStatus = status;
      if (changed) { updateTrayMenu(); createApplicationMenu(); }
      refreshDeviceSettings?.();
    }, { native, beforeManager: prepareForExit, preferences: () => desktopSettings, canInstallOnQuit: () => !assistCapture?.busy(),
      onAvailable: (manifest) => notifyDesktop('Palisade ' + manifest.version + ' is available.', openUpdateSettings) });
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

function createMainWindow(profile, startupVerified) {
  const icon = nativeImage.createFromPath(ICON);
  const desktop = createDesktopWindow({ partition: PARTITION, icon, nativeAssist: nativeEndpointEnabled && app.isPackaged && startupVerified,
    onSettings: () => ({ keepRunningInTray, launchOnLogin: app.getLoginItemSettings().openAtLogin,
      loginAvailable: app.isPackaged && !qualification, version: app.getVersion(), updateStatus, notificationStatus, nativeAssistStatus,
      ...updaterController?.notesState?.() }),
    onDeviceAction: (action) => {
      try {
      if (action === 'tray-on' || action === 'tray-off') trayPreferenceItem().click({ checked: action === 'tray-on' });
      else if ((action === 'login-on' || action === 'login-off') && app.isPackaged && !qualification) {
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
      else if (action === 'update-download') void updaterController?.downloadUpdate();
      else if (action === 'update-restart') void updaterController?.restart();
      else if (action === 'update-cancel') updaterController?.cancel();
      else if (action === 'update-snooze' || action === 'update-resume') {
        desktopSettings = saveSettings(app.getPath('userData'), { snoozeUntil: action === 'update-snooze' ? Date.now() + 24 * 60 * 60 * 1000 : 0 });
        updaterController?.settingsChanged();
      } else if (/^update-(auto-check|auto-download|install-on-quit)-(on|off)$/.test(action)) {
        const [key, value] = action.slice(7).match(/(.+)-(on|off)$/).slice(1);
        const field = { 'auto-check': 'autoCheck', 'auto-download': 'autoDownload', 'install-on-quit': 'installOnQuit' }[key];
        desktopSettings = saveSettings(app.getPath('userData'), { [field]: value === 'on' });
        updaterController?.settingsChanged();
      }
      } catch {
        void dialog.showMessageBox(window, { type: 'error', message: 'The device setting could not be applied. Try again.', buttons: ['OK'] });
      }
    },
  });
  refreshDeviceSettings = desktop.refreshSettings;
  showDeviceSettings = desktop.showSettings;
  window = desktop.window;
  const identity = appDetails({ packaged: app.isPackaged && !qualification });
  if (identity) window.setAppDetails(identity);
  const contents = mainContents = desktop.contents;
  assistCapture = createCaptureController({ enabled: captureEnabled && startupVerified, getContents: () => contents, getWindow: () => window,
    enumerate: options => desktopCapturer.getSources(options), choose: chooseSource, onState: () => updateTrayMenu(),
    revoke: () => { if (!contents.isDestroyed()) { contents.forcefullyCrashRenderer(); contents.reload(); } } });
  installSessionPolicy(profile, () => mainContents);
  assistBridge = installAssistBridge(contents, { desktopVersion: app.getVersion() });
  if (nativeEndpointEnabled && app.isPackaged && startupVerified) {
    if (!nativeAssistRuntime) nativeAssistRuntime = createNativeRuntime({ lifecycle: assistLifecycle,
      ...packagedGuardianPaths(process.resourcesPath),
      captureOptions: qualification ? { getContents: () => contents, getWindow: () => window, screen,
        enumerate: options => desktopCapturer.getSources(options), choose: chooseSource } : null,
      onStatus: status => { nativeAssistStatus = status; refreshDeviceSettings?.(); updateTrayMenu(); } });
    void nativeAssistRuntime.ready.catch(() => {});
    nativeAssistBridge = installNativeBridge(contents, { enabled: true,
      dispatch: nativeAssistRuntime.dispatch, stopAll: nativeAssistRuntime.stopAll });
  }
  for (const name of ['minimize', 'hide', 'close', 'unresponsive']) window.on(name, assistCapture.stop);
  contents.on('did-start-navigation', (_event, _url, inPlace, mainFrame) => { if (mainFrame && !inPlace) assistCapture?.stop(); });
  contents.on('render-process-gone', () => assistCapture?.stop());
  const revokeCapture = () => { assistCapture?.stop(); void nativeAssistRuntime?.stopAll().catch(() => {}); };
  for (const name of ['lock-screen', 'suspend', 'shutdown']) powerMonitor.on(name, revokeCapture);
  for (const name of ['display-added', 'display-removed', 'display-metrics-changed']) screen.on(name, revokeCapture);
  window.once('closed', () => {
    nativeAssistBridge?.dispose(); assistBridge?.dispose(); assistCapture?.dispose();
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
    if (!quitting) {
      event.preventDefault();
      if (keepRunningInTray) hideToTray();
      else app.quit();
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

function openUpdateSettings() {
  openWindow();
  showDeviceSettings?.('about');
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
      desktopSettings = saveSettings(app.getPath('userData'), { keepRunningInTray: item.checked });
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
      ...(exitWaiting ? [{ label: 'Quit pending: stopping assistance', enabled: false }] : []),
      ...(nativeEndpointEnabled ? [{ label: nativeAssistStatus, enabled: false },
        { label: 'Stop assistance', click: () => { void nativeAssistRuntime?.stopAll().catch(() => {}); } }] : []),
      { label: 'Stop screen sharing and reload Palisade', enabled: assistCapture?.busy() === true, click: () => assistCapture?.stop() },
      trayPreferenceItem(),
      { type: 'separator' },
      { label: 'Send test notification', click: () => notifyDesktop('Desktop notifications are available while Palisade is open or in the tray.') },
      { label: updateStatus, enabled: false },
      { label: "Updates & What's New", click: openUpdateSettings },
      {
        label: 'Check for Updates', click: () => void updaterController?.check(true),
      },
      {
        label: 'Open Update Manager', click: () => void updaterController?.openManager(),
      },
      { type: 'separator' },
      {
        label: 'Quit Palisade',
        click: () => {
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
          { label: "Updates & What's New", click: openUpdateSettings },
          { label: 'Reload Palisade', accelerator: 'CmdOrCtrl+R', click: () => mainContents?.reload() },
          { label: 'Check for Updates', click: () => void updaterController?.check(true) },
          { label: 'Open Update Manager', click: () => void updaterController?.openManager() },
          trayPreferenceItem(),
          { label: 'Launch on login', type: 'checkbox', enabled: app.isPackaged && !qualification,
            checked: app.getLoginItemSettings().openAtLogin,
            click: (item) => { app.setLoginItemSettings({ openAtLogin: item.checked }); createApplicationMenu(); } },
          { label: 'About Palisade', click: () => void dialog.showMessageBox({
            type: 'info', title: 'About Palisade', message: `Palisade ${app.getVersion()}`,
            detail: 'Windows x64 desktop client\nCopyright 2026 Onyx Operations Group\nApache-2.0. Palisade trademarks remain with their owner.', buttons: ['OK'],
          }) },
          { type: 'separator' },
          { label: exitWaiting ? 'Quit pending: stopping assistance' : 'Quit', accelerator: 'CmdOrCtrl+Q', click: () => app.quit() },
        ],
      },
      {
        label: 'Edit',
        submenu: [{ role: 'undo' }, { role: 'redo' }, { type: 'separator' }, { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }],
      },
    ]),
  );
}
