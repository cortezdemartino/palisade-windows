'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const { EventEmitter } = require('node:events');

test('Tabler controls reserve a separate strip and reject untrusted window commands', async () => {
  const ipcMain = new EventEmitter();
  const views = [];
  let owner;
  function contents() {
    const session = { protocol: { handle(scheme, handler) { this.scheme = scheme; this.handler = handler; } }, setPermissionRequestHandler(fn) { this.request = fn; }, setPermissionCheckHandler(fn) { this.check = fn; }, setDevicePermissionHandler(fn) { this.device = fn; } };
    return Object.assign(new EventEmitter(), {
      session, mainFrame: { url: '' }, sent: [],
      setWindowOpenHandler(fn) { this.open = fn; },
      isDestroyed() { return this.closed === true; }, close() { this.closed = true; },
      getURL() { return this.mainFrame.url; },
      loadURL(url) { this.mainFrame.url = url; this.emit('did-finish-load'); return Promise.resolve(); },
      send(...args) { this.sent.push(args); }, insertCSS: async () => 'drag-style',
    });
  }
  class Owner extends EventEmitter {
    constructor(options) { super(); owner = this; this.options = options; this.webContents = contents(); this.contentView = { addChildView: (child) => views.push(child) }; }
    getContentSize() { return this.size || [1000, 700]; }
    isFullScreen() { return this.fullScreen === true; }
    isMaximized() { return this.maximized === true; }
    isDestroyed() { return false; }
    minimize() { this.minimized = true; }
    maximize() { this.maximized = true; this.emit('maximize'); }
    unmaximize() { this.maximized = false; this.emit('unmaximize'); }
    close() { this.closeRequested = true; }
    loadURL(url) { return this.webContents.loadURL(url); }
  }
  class View {
    constructor(options) { this.options = options; this.webContents = contents(); }
    setBackgroundColor(color) { this.color = color; }
    setBounds(bounds) { this.bounds = bounds; }
    setVisible(visible) { this.visible = visible; }
  }
  const context = { module: { exports: {} }, URL, Response, __dirname: path.join(__dirname, '../src'), require: (name) => name === 'electron' ? { BrowserWindow: Owner, WebContentsView: View, ipcMain, protocol: { registerSchemesAsPrivileged() {} } } : require(name) };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/desktop-window.cjs'), 'utf8'), context);
  const deviceActions = [];
  const result = context.module.exports.createDesktopWindow({ onSettings: () => ({ keepRunningInTray: true, version: '0.2.3' }), onDeviceAction: action => deviceActions.push(action), partition: 'persist:palisade-desktop', icon: {} });
  const [view, controls] = views;
  assert.equal(owner.options.frame, false); assert.equal(owner.options.titleBarOverlay, undefined);
  assert.equal(owner.options.webPreferences.javascript, false);
  assert.equal(view.options.webPreferences.partition, 'persist:palisade-desktop');
  assert.equal(view.options.webPreferences.backgroundThrottling, false);
  assert.match(view.options.webPreferences.preload, /chat-notifications-preload\.cjs$/);
  assert.match(controls.options.webPreferences.preload, /window-controls-preload\.cjs$/);
  for (const prefs of [owner.options.webPreferences, view.options.webPreferences, controls.options.webPreferences]) {
    assert.equal(prefs.sandbox, true); assert.equal(prefs.contextIsolation, true); assert.equal(prefs.nodeIntegration, false);
  }
  assert.equal(result.contents, view.webContents);
  const localProtocol = controls.webContents.session.protocol;
  assert.equal(localProtocol.scheme, 'palisade-controls');
  const page = await localProtocol.handler({ method: 'GET', url: 'palisade-controls://app/window-chrome.html' });
  assert.equal(page.status, 200);
  assert.match(await page.text(), /id="settings"/);
  for (const url of ['palisade-controls://other/window-chrome.html', 'palisade-controls://app/main.cjs', 'palisade-controls://app/../package.json', 'palisade-controls://app/window-chrome.html?file=main.cjs']) {
    assert.equal((await localProtocol.handler({ method: 'GET', url })).status, 404);
  }
  assert.equal((await localProtocol.handler({ method: 'POST', url: 'palisade-controls://app/window-chrome.html' })).status, 404);
  assert.equal(view.webContents.session.protocol.handler, undefined);
  assert.equal(view.bounds.y, 36); assert.equal(view.bounds.height, 664);
  assert.ok(controls.bounds.y + controls.bounds.height <= view.bounds.y);
  assert.equal(controls.bounds.width, 136); assert.equal(controls.bounds.height, 32); assert.equal(controls.color, '#00000000');
  assert.equal(controls.webContents.open().action, 'deny');
  let granted; controls.webContents.session.request(null, 'notifications', (value) => { granted = value; });
  assert.equal(granted, false); assert.equal(controls.webContents.session.check(), false);
  const emit = (sender, frame, action) => ipcMain.emit('palisade:window-control', { sender, senderFrame: frame }, action);
  emit(view.webContents, view.webContents.mainFrame, 'settings'); assert.equal(deviceActions.length, 0);
  emit(view.webContents, view.webContents.mainFrame, 'close'); assert.equal(owner.closeRequested, undefined);
  emit(controls.webContents, {}, 'close'); assert.equal(owner.closeRequested, undefined);
  const frame = controls.webContents.mainFrame, url = frame.url;
  frame.url = 'https://evil.test/'; emit(controls.webContents, frame, 'close'); assert.equal(owner.closeRequested, undefined); frame.url = url;
  emit(controls.webContents, frame, 'execute'); assert.equal(owner.closeRequested, undefined);
  emit(controls.webContents, frame, 'settings'); assert.equal(controls.webContents.sent.at(-1)[1].settingsOpen, true);
  emit(controls.webContents, frame, 'tray-off'); assert.equal(deviceActions[0], 'tray-off');
  emit(controls.webContents, frame, 'settings'); assert.equal(controls.webContents.sent.at(-1)[1].settingsOpen, false);
  emit(controls.webContents, frame, 'minimize'); assert.equal(owner.minimized, true);
  emit(controls.webContents, frame, 'maximize'); assert.equal(owner.maximized, true);
  assert.equal(controls.webContents.sent.at(-1)[1].maximized, true);
  emit(controls.webContents, frame, 'maximize'); assert.equal(owner.maximized, false);
  emit(controls.webContents, frame, 'close'); assert.equal(owner.closeRequested, true);
  owner.size = [1400, 900]; owner.emit('resize');
  assert.equal(view.bounds.width, 1400); assert.equal(view.bounds.height, 864); assert.equal(controls.bounds.x, 1246);
  owner.fullScreen = true; owner.emit('enter-full-screen'); assert.equal(controls.visible, false); assert.equal(view.bounds.y, 0); assert.equal(view.bounds.height, 900);
  owner.fullScreen = false; owner.emit('leave-full-screen'); assert.equal(controls.visible, true); assert.equal(view.bounds.y, 36); assert.equal(view.bounds.height, 864);
  owner.emit('closed'); assert.equal(view.webContents.closed, true); assert.equal(controls.webContents.closed, true);
  assert.equal(ipcMain.listenerCount('palisade:window-control'), 0);
});

test('local preload exposes only fixed controls and strips Electron events from state updates', () => {
  const ipcRenderer = new EventEmitter(); const sent = []; let bridge;
  ipcRenderer.send = (...args) => sent.push(args);
  const context = { require: () => ({ ipcRenderer, contextBridge: { exposeInMainWorld: (name, value) => { assert.equal(name, 'desktopControls'); bridge = value; } } }) };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/window-controls-preload.cjs'), 'utf8'), context);
  assert.equal(Object.keys(bridge).sort().join(','), 'checkForUpdates,close,copyDiagnostics,maximize,microphoneSettings,minimize,notificationSettings,onState,openUpdates,reloadClient,setLogin,setTray,settings,testNotification');
  bridge.minimize(); bridge.maximize(); bridge.close();
  assert.equal(sent.map((args) => args[1]).join(','), 'minimize,maximize,close');
  const received = []; const unsubscribe = bridge.onState((...args) => received.push(args));
  ipcRenderer.emit('palisade:window-state', { sender: 'privileged' }, { maximized: 'invalid' }); assert.equal(received.length, 0);
  ipcRenderer.emit('palisade:window-state', { sender: 'privileged' }, { maximized: true });
  assert.equal(received.length, 1); assert.equal(received[0].length, 1); assert.equal(received[0][0].maximized, true);
  assert.equal(received[0][0].settingsOpen, false);
  assert.equal(received[0][0].version, '');
  bridge.setTray(false); bridge.setLogin(true); bridge.testNotification(); bridge.openUpdates();
  assert.equal(sent.slice(-4).map(args => args[1]).join(','), 'tray-off,login-on,notification-test,updates');
  unsubscribe(); assert.equal(ipcRenderer.listenerCount('palisade:window-state'), 0);
});

test('tray close keeps the session alive, notification click restores the app, and Quit really exits', async () => {
  const notices = []; const externalSettings = []; let diagnosticText;
  let hidden = 0, shown = 0, focused = 0, appMenu, openSettings, deviceAction;
  const remote = Object.assign(new EventEmitter(), { isDestroyed: () => false, getURL: () => 'https://palisade.onyxops.org/', setWindowOpenHandler() {}, loadURL: async () => {}, reload() {}, focus: () => focused++ });
  const window = Object.assign(new EventEmitter(), { getContentSize: () => [1000, 700], hide: () => hidden++, show: () => shown++, focus() {}, isMinimized: () => false, isDestroyed: () => false });
  const profile = { webRequest: { onBeforeRequest() {} }, setPermissionRequestHandler(fn) { this.request = fn; }, setPermissionCheckHandler(fn) { this.check = fn; }, setDevicePermissionHandler() {}, setDisplayMediaRequestHandler(fn) { this.display = fn; }, flushStorageData() {} };
  const app = Object.assign(new EventEmitter(), { setName() {}, setAppUserModelId() {}, requestSingleInstanceLock: () => true,
    whenReady: () => Promise.resolve(), isReady: () => true, getPath: () => 'fixture-profile', getLoginItemSettings: () => ({ openAtLogin: false }), getVersion: () => '0.2.2', quit: () => {
      let prevented = false;
      app.emit('before-quit', { preventDefault() { prevented = true; } });
      if (!prevented) app.emit('will-quit');
    } });
  class Notice extends EventEmitter { static isSupported() { return true; } constructor(options) { super(); this.options = options; } show() { notices.push(this); } }
  class Tray extends EventEmitter { setToolTip() {} setContextMenu(menu) { this.menu = menu; } }
  const electron = { app, powerMonitor: new EventEmitter(), screen: new EventEmitter(), clipboard: { writeText: text => { diagnosticText = text; } }, Notification: Notice, Tray, nativeImage: { createFromPath: () => ({}) }, session: { fromPartition: () => profile },
    Menu: { getApplicationMenu: () => ({ items: [{ submenu: { popup: (options) => { settingsPopup = options; } } }] }), buildFromTemplate: (value) => value, setApplicationMenu: (value) => { appMenu = value; } }, dialog: { showMessageBox: async () => ({ response: 1 }) }, shell: { openExternal: async url => { externalSettings.push(url); } } };
  const context = { module: { exports: {} }, __dirname: path.join(__dirname, '../src'), process: { platform: 'win32', arch: 'x64', versions: { electron: '44.7.2' } }, require(name) {
    if (name === 'electron') return electron;
    if (name === './desktop-window.cjs') return { createDesktopWindow: ({ onSettings, onDeviceAction }) => { openSettings = onSettings; deviceAction = onDeviceAction; return { window, contents: remote }; } };
    if (name === './windows-notification-branding.cjs') return { APP_ID: 'org.onyxops.palisade', ensureNotificationBranding: async () => {} };
    if (name === './chat-notifications.cjs') return { installChatNotifications() {} };
    if (name === './session-policy.cjs') return require('../src/session-policy.cjs');
    if (name === './assist/bridge.cjs') return { installAssistBridge: () => ({ dispose() {} }) };
    if (name === './assist/source-picker.cjs') return { chooseSource() {} };
    if (name === './assist/capture.cjs') return require('../src/assist/capture.cjs');
    if (name === './assist/build-policy.cjs') return require('../src/assist/build-policy.cjs');
    if (name === './assist/lifecycle.cjs') return require('../src/assist/lifecycle.cjs');
    if (name === './settings.cjs') return { loadSettings: () => ({ keepRunningInTray: true }), saveSettings() {} };
    if (name === './updates/updater.cjs') return { createUpdateController: () => ({ start() {}, check() {} }) };
    if (name === './security-policy.cjs') return require('../src/security-policy.cjs');
    return require(name);
  } };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/main.cjs'), 'utf8'), context);
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(openSettings().version, '0.2.2');
  deviceAction('tray-off');
  assert.equal(openSettings().keepRunningInTray, false);
  deviceAction('tray-on');
  deviceAction('notification-settings'); deviceAction('microphone-settings');
  assert.deepEqual(externalSettings, ['ms-settings:notifications', 'ms-settings:privacy-microphone']);
  deviceAction('copy-diagnostics');
  assert.match(diagnosticText, /Palisade 0.2.2/);
  assert.doesNotMatch(diagnosticText, /fixture-profile|palisade.onyxops.org|token|session/i);
  assert.ok(appMenu[0].submenu.some((item) => item.label === 'Minimize and close to tray'));
  let prevented = false;
  window.emit('close', { preventDefault: () => { prevented = true; } });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(prevented, true); assert.equal(hidden, 1); assert.equal(notices.length, 1);
  notices[0].emit('click'); assert.equal(shown, 1); assert.equal(focused, 1);
  window.emit('minimize'); assert.equal(hidden, 2); assert.equal(notices.length, 1);
  let granted;
  profile.request(remote, 'notifications', (value) => { granted = value; }, { requestingUrl: remote.getURL(), isMainFrame: true });
  assert.equal(granted, true);
  profile.request({}, 'notifications', (value) => { granted = value; }, { requestingUrl: remote.getURL() });
  assert.equal(granted, false);
  profile.request(remote, 'notifications', (value) => { granted = value; }, { requestingUrl: 'https://evil.test/', isMainFrame: true });
  assert.equal(granted, false);
  const quit = appMenu[0].submenu.find((item) => item.label === 'Quit');
  assert.equal(quit.accelerator, 'CmdOrCtrl+Q'); quit.click();
  await new Promise(resolve => setImmediate(resolve));
  prevented = false; window.emit('close', { preventDefault: () => { prevented = true; } });
  assert.equal(prevented, false);
});
