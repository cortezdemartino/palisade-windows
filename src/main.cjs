'use strict';

const { app, crashReporter, desktopCapturer, globalShortcut, powerMonitor, screen, clipboard, dialog, Menu, nativeImage, Notification, session, shell, Tray } = require('electron');
const path = require('node:path');
const { APP_ID, ensureNotificationBranding } = require('./windows-notification-branding.cjs');
const { appDetails, ensureAppRegistration } = require('./windows-app-registration.cjs');
const { createUpdateController } = require('./updates/updater.cjs');
const { createNativeHelper } = require('./updates/native-helper.cjs');
const { loadSettings, saveSettings } = require('./settings.cjs');
const { openMediaSettings, ACTIONS, defaults: mediaDefaults, userError: mediaUserError } = require('./media-settings.cjs');
const { createMediaShortcuts } = require('./media-shortcuts.cjs');
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
const { createStore: createDiagnosticsStore } = require('./diagnostics/store.cjs');
const { createDiagnostics, promptRecovery } = require('./diagnostics/monitor.cjs');
const { installDiagnosticsBridge } = require('./diagnostics/bridge.cjs');
const { createTelemetry } = require('./diagnostics/telemetry.cjs');
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
let mediaSettings, mediaShortcuts;
let quitting = false;
let exitWaiting = false;
let lastExternalPromptAt = 0;
let updaterController = null;
let updateStatus = 'Updates: idle';
let notificationBranding = Promise.resolve();
let notificationStatus = 'No desktop notifications sent yet.';
let deviceFeedback = '', deviceFeedbackError = false;
// Testing Center crash detection (src/diagnostics): local only; started after the instance lock.
let diagnostics = null, diagnosticsStore = null, diagnosticsBridge = null, desktopTelemetry = null, lastUpdatePhase = '';
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
  try {
    diagnosticsStore = createDiagnosticsStore({ dir: path.join(app.getPath('userData'), 'diagnostics') });
    diagnostics = createDiagnostics({ app, crashReporter, store: diagnosticsStore });
    diagnostics.start();
    desktopTelemetry = createTelemetry({ dir: path.join(app.getPath('userData'), 'diagnostics'), identity: diagnostics.identity, sessionId: diagnostics.sessionId, app });
    desktopTelemetry.onLaunch({ previous: diagnostics.recovery().previous });
    app.on('render-process-gone', diagnostics.onRenderGone);
    app.on('child-process-gone', diagnostics.onChildGone);
    app.on('will-quit', () => { desktopTelemetry?.lifecycle('quit'); desktopTelemetry?.stopSampling(); diagnostics?.markCleanExit(); });
  } catch {
    // Diagnostics are optional: Palisade starts without them.
    diagnostics = null;
  }
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
    mediaSettings = openMediaSettings(app.getPath('userData'));
    keepRunningInTray = desktopSettings.keepRunningInTray;
    const profile = session.fromPartition(PARTITION, { cache: true });
    createMainWindow(profile, Boolean(native) || !app.isPackaged || Boolean(qualification));
    createTray();
    if (qualification) { updateStatus = 'Updates unavailable in qualification builds'; createApplicationMenu(); return; }
    updaterController = createUpdateController(() => window, (status) => {
      const changed = updateStatus !== status;
      updateStatus = status;
      const phase = updaterController?.notesState?.().updatePhase || '';
      if (phase === 'failed' && lastUpdatePhase !== 'failed') {
        diagnostics?.updateFailed('UPDATE_FAILED');
        desktopTelemetry?.update(['downloading', 'verifying'].includes(lastUpdatePhase) ? 'download' : 'install', 'failure', 'UPDATE_FAILED');
      } else if (phase !== lastUpdatePhase) {
        if (phase === 'available') desktopTelemetry?.update('check', 'success');
        if (phase === 'staged') desktopTelemetry?.update('download', 'success');
        if (phase === 'installing') desktopTelemetry?.update('install', 'success');
      }
      lastUpdatePhase = phase;
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
      architecture: process.arch, electronVersion: process.versions.electron,
      managerAvailable: app.isPackaged && !qualification,
      buildKind: qualification ? 'qualification' : app.isPackaged ? 'installed' : 'development', deviceFeedback, deviceFeedbackError,
      media: mediaSettings?.snapshot(), mediaError: mediaSettings?.error() || '', mediaShortcutFailures: mediaShortcuts?.failures() || [],
      ...updaterController?.notesState?.() }),
    onMediaSettings: (request) => {
      try {
        if (!request || Array.isArray(request) || !Number.isSafeInteger(request.revision)) throw Error('Invalid media setting.');
        const previous = mediaSettings.snapshot(); let patch;
        if (request.op === 'background' && Object.keys(request).sort().join(',') === 'enabled,op,revision' && typeof request.enabled === 'boolean') patch = { backgroundShortcuts: request.enabled };
        else if (request.op === 'reset' && Object.keys(request).sort().join(',') === 'op,revision') patch = { keybinds: mediaDefaults().keybinds, backgroundShortcuts: false };
        else if (request.op === 'binding' && Object.keys(request).sort().join(',') === 'accelerator,action,enabled,op,revision' && ACTIONS.includes(request.action))
          patch = { keybinds: { ...previous.keybinds, [request.action]: { accelerator: request.accelerator, enabled: request.enabled } } };
        else throw Error('Unsupported media setting.');
        const next = mediaSettings.update(patch, request.revision); mediaShortcuts?.configure(next);
        deviceFeedback = 'Shortcut settings saved on this PC.'; deviceFeedbackError = false;
      } catch (error) { deviceFeedback = mediaUserError(error); deviceFeedbackError = true; }
    },
    onDeviceAction: (action) => {
      deviceFeedback = ''; deviceFeedbackError = false;
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
        deviceFeedback = 'Troubleshooting information copied.';
      } else if (action === 'reload-client') void requestReload();
      else if (action === 'help-guide' || action === 'help-issues') {
        const destination = action === 'help-guide'
          ? 'https://github.com/cortezdemartino/palisade-windows/blob/main/docs/USER-GUIDE.md'
          : 'https://github.com/cortezdemartino/palisade-windows/issues';
        void shell.openExternal(destination).catch(() => {
          deviceFeedback = 'The support page could not be opened. Try again.'; deviceFeedbackError = true; refreshDeviceSettings?.();
        });
      }
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
        deviceFeedback = 'The action could not be completed. Try again.'; deviceFeedbackError = true;
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
  mediaShortcuts = createMediaShortcuts({ globalShortcut,
    available: action => action === 'device-settings', focused: () => window?.isFocused?.() === true,
    dispatch: () => { openWindow(); showDeviceSettings?.('permissions'); },
    onStatus: () => refreshDeviceSettings?.(),
  });
  mediaShortcuts.configure(mediaSettings.snapshot());
  contents.on('before-input-event', mediaShortcuts.input);
  contents.on('blur', mediaShortcuts.blur);
  contents.on('focus', mediaShortcuts.focus);
  window.on('blur', mediaShortcuts.blur);
  window.on('focus', mediaShortcuts.focus);
  app.on('will-quit', mediaShortcuts.stop);
  contents.on('render-process-gone', mediaShortcuts.blur);
  window.once('closed', mediaShortcuts.stop);
  window.once('closed', () => { showDeviceSettings = null; });
  contents.on('destroyed', mediaShortcuts.stop);
  assistCapture = createCaptureController({ enabled: captureEnabled && startupVerified, getContents: () => contents, getWindow: () => window,
    enumerate: options => desktopCapturer.getSources(options), choose: chooseSource, onState: () => updateTrayMenu(),
    revoke: () => { if (!contents.isDestroyed()) { diagnostics?.expectRendererExit(); contents.forcefullyCrashRenderer(); contents.reload(); } } });
  installSessionPolicy(profile, () => mainContents);
  assistBridge = installAssistBridge(contents, { desktopVersion: app.getVersion() });
  if (nativeEndpointEnabled && app.isPackaged && startupVerified) {
    if (!nativeAssistRuntime) nativeAssistRuntime = createNativeRuntime({ lifecycle: assistLifecycle,
      ...packagedGuardianPaths(process.resourcesPath),
      captureOptions: qualification ? { getContents: () => contents, getWindow: () => window, screen,
        enumerate: options => desktopCapturer.getSources(options), choose: chooseSource } : null,
      onStatus: status => {
        nativeAssistStatus = status; refreshDeviceSettings?.(); updateTrayMenu();
        if (status === 'Cleanup failed') diagnostics?.helperFailed('ASSIST_CLEANUP_FAILED');
        else if (status === 'Native endpoint unavailable') diagnostics?.helperFailed('ASSIST_ENDPOINT_UNAVAILABLE');
      } });
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
  if (diagnostics && diagnosticsStore) {
    diagnostics.watchContents(contents);
    diagnosticsBridge = installDiagnosticsBridge(contents, { diagnostics, store: diagnosticsStore, telemetry: desktopTelemetry });
  }
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

  window.once('ready-to-show', () => {
    window?.show(); mainContents?.focus();
    diagnostics?.markWindowReady();
    // Process start to a shown window: the desktop cold start.
    const coldStart = Math.round(process.uptime() * 1000);
    desktopTelemetry?.lifecycle('ready', { durationMs: Math.min(coldStart, 3600000) });
    desktopTelemetry?.measure('cold-start', coldStart, 'ms', 'startup');
    desktopTelemetry?.startSampling(() => window?.isFocused?.() === true);
    // After an unexpected end, offer (never force) a report. The page opens Report a bug.
    if (diagnostics) {
      void promptRecovery({ dialog, window, recovery: diagnostics.recovery() }).then(report => {
        if (report) diagnosticsBridge?.requestRecovery();
      }).catch(() => {});
    }
  });
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
  openDeviceSettings('updates');
}
function openDeviceSettings(section) { openWindow(); showDeviceSettings?.(section); }
let reloadPrompt;
function requestReload() {
  if (reloadPrompt) return reloadPrompt;
  reloadPrompt = dialog.showMessageBox(window, { type: 'question', title: 'Reload Palisade?',
    message: 'Reload Palisade?', detail: 'Calls and screen sharing may disconnect. Your account data stays on this PC.',
    buttons: ['Reload', 'Cancel'], defaultId: 1, cancelId: 1, noLink: true }).then(answer => {
      if (answer?.response === 0 && !mainContents?.isDestroyed()) mainContents?.reload();
    }).catch(() => {
      deviceFeedback = 'Reload could not be started. Try again.'; deviceFeedbackError = true; refreshDeviceSettings?.();
    }).finally(() => { reloadPrompt = null; });
  return reloadPrompt;
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
      deviceFeedback = 'The tray preference could not be saved. Your previous choice is unchanged.'; deviceFeedbackError = true;
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
      ...(assistCapture?.busy() === true ? [{ label: 'Stop screen sharing and reload Palisade', click: () => assistCapture?.stop() }] : []),
      { type: 'separator' },
      ...(['available', 'staged', 'downloading', 'verifying', 'failed', 'awaiting-safe-restart', 'deferred', 'installing'].includes(updaterController?.notesState?.().updatePhase)
        ? [{ label: updateStatus, enabled: false }] : []),
      { label: 'Updates', click: openUpdateSettings },
      { label: 'Settings', click: () => openDeviceSettings() },
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
          { label: 'Settings', click: () => openDeviceSettings() },
          { label: 'Updates', click: openUpdateSettings },
          { label: "What's new", click: () => openDeviceSettings('whats-new') },
          { label: 'Reload Palisade', accelerator: 'CmdOrCtrl+R', click: () => void requestReload() },
          { label: 'Check for updates', click: () => { openUpdateSettings(); void updaterController?.check(true); } },
          { label: 'About Palisade', click: () => openDeviceSettings('about') },
          { type: 'separator' },
          { label: exitWaiting ? 'Quit pending: stopping assistance' : 'Quit Palisade', accelerator: 'CmdOrCtrl+Q', click: () => app.quit() },
        ],
      },
      {
        label: 'Edit',
        submenu: [{ role: 'undo' }, { role: 'redo' }, { type: 'separator' }, { role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'selectAll' }],
      },
    ]),
  );
}
