'use strict';
const { app, BrowserWindow, ipcMain, dialog, shell, nativeTheme, Notification, protocol } = require('electron');
const path = require('node:path');
const fs = require('node:fs/promises');
const os = require('node:os');
const { Settings } = require('./config.cjs');
const windows = require('./windows.cjs');
const updates = require('./updates/service.cjs');
const { createNotesHistory } = require('./updates/notes-history.cjs');
const { compareVersions, verifyDownloadedFile } = require('./updates/release.cjs');
const { resolveAsset } = require('./ui-protocol.cjs');
protocol.registerSchemesAsPrivileged([{ scheme: 'palisade', privileges: { standard: true, secure: true } }]);
app.setAppUserModelId('org.onyxops.palisade.installer');
const locked = app.requestSingleInstanceLock();
if (!locked) app.quit();
let win, settings, key, abort, selected, offer, notesHistory, busy = false, installing = false;
const state = { phase: 'initializing', message: 'Looking for Palisade…', installed: [], current: null,
  available: null, notes: '', releaseHistory: [], source: '', progress: null, error: '', service: 'Not checked', history: [], preferences: {}, recovery: '' };
const icon = path.join(__dirname, '..', 'assets', 'icon.ico');
function push() {
  state.releaseHistory = notesHistory?.snapshot() || [];
  state.preferences = { theme: settings?.value.theme || 'dark', desktopShortcut: settings?.value.desktopShortcut ?? true };
  state.history = settings?.value.history || [];
  if (win && !win.isDestroyed()) win.webContents.send('state', { ...state });
}
async function cacheNotes(candidate) {
  if (candidate.notesPair) {
    try { await notesHistory.add(candidate.notesPair.release, candidate.notesPair.notes); }
    catch { /* Verified in-memory notes still display if the cache cannot be saved. */ }
  }
}
async function log(event) {
  // Only fixed event names are logged: no exception strings, usernames, URLs or profile data.
  const file = path.join(app.getPath('userData'), 'events.jsonl');
  try {
    if ((await fs.stat(file).catch(() => ({ size: 0 }))).size > 512 * 1024) await fs.rename(file, file + '.previous');
    await fs.appendFile(file, JSON.stringify({ at: new Date().toISOString(), event }) + '\n', { mode: 0o600 });
  } catch { /* Diagnostics must not prevent recovery. */ }
}
async function refresh() {
  state.installed = await windows.detectInstallations();
  state.current = selected ? await windows.inspectInstallation(selected, state.installed.find(x => x.path === selected)) :
    state.installed[0] || { status: 'missing', path: windows.defaultInstallPath(), version: null };
  for (const installation of [...state.installed, state.current]) {
    if (!installation.version) continue;
    compareVersions(installation.version, '0.0.0');
    if (compareVersions(installation.version, settings.value.highestVersion) > 0) settings.value.highestVersion = installation.version;
  }
}
async function operation(task) {
  if (busy) throw new Error('Another operation is already running.');
  busy = true; state.busy = true; state.error = ''; state.progress = null; abort = new AbortController(); push();
  try { await task(abort.signal); }
  catch (e) {
    state.phase = abort.signal.aborted ? 'cancelled' : 'error';
    state.message = abort.signal.aborted ? 'Operation cancelled. You can try again.' : 'The operation could not be completed.';
    state.error = abort.signal.aborted ? '' : safeError(e);
    await log(abort.signal.aborted ? 'operation.cancelled' : 'operation.failed');
  } finally { busy = false; installing = false; state.busy = false; state.canCancel = false; state.progress = null; push(); }
}
function safeError(e) {
  if (e.code === 'ENOSPC') return 'The drive is full. Free disk space and retry.';
  if (['EACCES', 'EPERM'].includes(e.code)) return 'Access was denied. Choose a writable folder or check Windows security settings.';
  if (e.code === 'ENOENT') return 'A required file is missing. Select a complete offline bundle or check the installation folder.';
  // Controlled messages only. Do not surface network URLs, filesystem paths or raw OS exceptions.
  if (/^(This package|The signed|The package|The offline|No stable|Download exceeds|Not enough|Choose |Installation |Windows could not|The installer did not|The update service|The selected|Release |Installer |Invalid |Unexpected |Unsupported|Downgrade)/.test(e.message || '') &&
      !/[\\/]/.test(e.message)) return e.message;
  return 'The package could not be retrieved or verified. Check your connection, retry, or choose a signed offline package. No unverified package was run.';
}
async function check(signal) {
  state.phase = 'checking'; state.busy = true; state.canCancel = true; state.message = 'Checking the official release service…'; push();
  try {
    const candidate = await updates.discover(key, signal);
    updates.enforceVersion(candidate.manifest, state.current?.version, settings.value.highestVersion);
    settings.value.highestVersion = candidate.manifest.version; await settings.save();
    offer = candidate;
    await cacheNotes(offer);
    state.available = offer.manifest; state.notes = offer.notes; state.source = offer.source;
    state.service = 'Connected · signed release verified'; state.phase = 'ready';
    state.message = state.current?.version === offer.manifest.version ? 'Palisade is up to date.' : 'A verified release is available.';
    await log('release.verified');
  } catch (e) { state.service = 'Unavailable or verification failed'; throw e; }
}
async function install(signal) {
  if (!offer) throw new Error('The package must be verified before installation.');
  await refresh();
  const manifest = offer.manifest;
  updates.enforceVersion(manifest, state.current?.version, settings.value.highestVersion);
  const directory = windows.validatePath(state.current.path);
  const machine = state.current.machine || windows.installationScope(directory);
  await windows.ensureClientClosed(directory);
  const answer = await dialog.showMessageBox(win, { type: 'question', title: 'Install Palisade',
    message: state.current.status === 'missing' ? 'Install Palisade ' + manifest.version + '?' : 'Update or repair Palisade ' + manifest.version + '?',
    detail: 'Destination: ' + directory + '\nSave your work and close Palisade. This manager will install the verified package. Windows may request administrator permission. Your Palisade profile and sessions will be retained. The selected folder and shortcut preference will be used.',
    buttons: ['Continue', 'Cancel'], defaultId: 1, cancelId: 1, noLink: true });
  if (answer.response !== 0) { state.phase = 'ready'; state.message = 'Installation cancelled.'; return; }
  state.busy = true; state.canCancel = true; state.phase = 'preparing'; state.message = 'Checking the installation folder and disk space…'; push();
  if (state.current.status === 'missing') {
    const entries = await fs.readdir(directory).catch(e => { if (e.code === 'ENOENT') return []; throw e; });
    if (entries.length) throw new Error('Choose an empty folder for a new installation, or locate an existing Palisade installation.');
  }
  await windows.prepareDestination(directory, manifest.size * 4 + 512 * 1024 * 1024, undefined, machine);
  const temp = await fs.mkdtemp(path.join(app.getPath('temp'), 'palisade-setup-'));
  try {
    await windows.prepareDestination(temp, manifest.size + 128 * 1024 * 1024);
    let file;
    if (offer.source === 'offline') { file = path.join(temp, manifest.filename); await fs.copyFile(offer.packagePath, file, require('node:fs').constants.COPYFILE_EXCL); }
    else {
      state.phase = 'downloading'; state.message = 'Downloading Palisade…'; push();
      let last = 0;
      file = await updates.download(manifest, temp, signal, (bytes, total) => {
        if (Date.now() - last < 100 && bytes !== total) return;
        last = Date.now(); state.progress = { bytes, total }; push();
      });
    }
    signal.throwIfAborted();
    state.phase = 'verifying'; state.message = 'Verifying the complete installer…'; push();
    if (!(await verifyDownloadedFile(file, manifest))) throw new Error('The package failed its final integrity check.');
    signal.throwIfAborted();
    state.canCancel = false; installing = true; state.phase = 'installing'; state.progress = null;
    state.message = 'Installing Palisade... Approve the Windows administrator prompt if one appears.'; push();
    settings.value.pending = { version: manifest.version, path: directory };
    settings.record(manifest.version, 'started'); await settings.save(); await log('installation.started');
    try {
      await windows.ensureClientClosed(directory);
      await windows.runInstaller(file, directory, settings.value.desktopShortcut, machine);
      state.phase = 'verifying'; state.message = 'Checking installed application files and version…'; push();
      selected = directory; await refresh();
      if (state.current.status !== 'installed' || state.current.version !== manifest.version) throw new Error('The installer finished, but the expected application version was not found. Locate the installation or retry Repair.');
      settings.record(manifest.version, 'completed'); settings.value.pending = null; await settings.save();
      state.phase = 'complete'; state.message = 'Palisade ' + manifest.version + ' is ready.';
      await log('installation.completed');
      if (Notification.isSupported()) new Notification({ title: 'Palisade is ready', body: 'Installation verified. You can launch Palisade.', icon }).show();
    } catch (e) {
      settings.record(manifest.version, 'failed'); settings.value.pending = null; await settings.save();
      state.recovery = 'Installation did not verify. Refresh detection, locate a custom installation, or retry the same verified package with Repair. Your profile was not removed.';
      throw e;
    }
  } finally { await fs.rm(temp, { recursive: true, force: true }).catch(() => {}); }
}
async function command(name, value) {
  if (name === 'cancel') { if (state.canCancel) abort?.abort(); return; }
  if (busy) return;
  if (name === 'check') return operation(check);
  if (name === 'install') return operation(install);
  if (name === 'refresh') return operation(async () => { await refresh(); state.phase = 'ready'; state.message = 'Installation status refreshed.'; });
  if (name === 'offline') return operation(async () => {
    const result = await dialog.showOpenDialog(win, { title: 'Select the folder containing release.json and its installer', properties: ['openDirectory'] });
    if (result.canceled) return;
    const candidate = await updates.offline(result.filePaths[0], key);
    updates.enforceVersion(candidate.manifest, state.current?.version, settings.value.highestVersion);
    offer = candidate; await cacheNotes(offer); settings.value.highestVersion = offer.manifest.version; await settings.save();
    state.available = offer.manifest; state.notes = offer.notes; state.source = 'offline'; state.phase = 'ready'; state.message = 'Offline package verified.';
  });
  if (name === 'folder' || name === 'locate') return operation(async () => {
    const result = await dialog.showOpenDialog(win, { title: name === 'locate' ? 'Locate the Palisade application folder' : 'Choose installation folder', defaultPath: state.current.path, properties: ['openDirectory', 'createDirectory'] });
    if (result.canceled) return;
    selected = windows.validatePath(result.filePaths[0]);
    await refresh(); state.phase = 'ready'; state.message = 'Installation folder selected.';
  });
  if (name === 'launch') return operation(async () => {
    await refresh(); if (state.current.status !== 'installed') throw new Error('The selected installation is incomplete. Repair it before launching.');
    await windows.launch(state.current.executable); state.message = 'Palisade launched.';
  });
  if (name === 'openFolder') { if (state.current?.path) await shell.openPath(state.current.path); return; }
  if (name === 'releases') { await shell.openExternal('https://github.com/cortezdemartino/palisade-windows/releases'); return; }
  if (name === 'support') { await shell.openExternal('https://github.com/cortezdemartino/palisade-windows/issues'); return; }
  if (name === 'preferences') {
    if (!value || !['dark', 'light', 'system'].includes(value.theme) || typeof value.desktopShortcut !== 'boolean') throw new Error('Invalid preferences.');
    settings.value.theme = value.theme; settings.value.desktopShortcut = value.desktopShortcut; nativeTheme.themeSource = value.theme; await settings.save(); push(); return;
  }
  if (name === 'diagnostics') {
    const result = await dialog.showSaveDialog(win, { title: 'Export diagnostic report', defaultPath: 'Palisade-diagnostics.json', filters: [{ name: 'JSON report', extensions: ['json'] }] });
    if (!result.canceled) await fs.writeFile(result.filePath, JSON.stringify({ utilityVersion: app.getVersion(), platform: process.platform, architecture: process.arch,
      windowsRelease: os.release(), status: state.current?.status, installedVersion: state.current?.version, availableVersion: state.available?.version,
      service: state.service, phase: state.phase, history: settings.value.history }, null, 2), { mode: 0o600 });
  }
}
async function initialize() {
  settings = new Settings(app.getPath('userData')); await settings.load();
  const installationArgument = process.argv.indexOf('--installation');
  if (installationArgument !== -1) selected = windows.validatePath(process.argv[installationArgument + 1]);
  key = await fs.readFile(path.join(__dirname, '..', 'assets', 'update-public-key.pem'), 'utf8');
  notesHistory = createNotesHistory({ directory: path.join(app.getPath('userData'), 'updates'), root: key,
    transitions: require('../assets/update-key-transitions.json') });
  await notesHistory.load();
  nativeTheme.themeSource = settings.value.theme;
  protocol.handle('palisade', async request => {
    const asset = resolveAsset(request.url, request.method);
    if (!asset) return new Response('Not found', { status: 404 });
    try {
      const bytes = await fs.readFile(path.join(__dirname, '..', ...asset.relativePath.split('/')));
      return new Response(bytes, { headers: { 'Content-Type': asset.mime, 'X-Content-Type-Options': 'nosniff' } });
    } catch { return new Response('Resource unavailable', { status: 404 }); }
  });
  win = new BrowserWindow({ width: 1100, height: 790, minWidth: 760, minHeight: 620, backgroundColor: '#0b111c', title: 'Palisade Installer & Update Manager', icon,
    autoHideMenuBar: true, webPreferences: { preload: path.join(__dirname, 'preload.cjs'), sandbox: true, contextIsolation: true, nodeIntegration: false } });
  win.removeMenu();
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  win.webContents.on('will-navigate', e => e.preventDefault());
  win.webContents.session.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
  win.webContents.session.setPermissionCheckHandler(() => false);
  win.on('close', e => { if (installing || busy) { e.preventDefault(); dialog.showMessageBox(win, { type: 'info', message: installing ? 'Wait for installation to finish before closing this utility.' : 'Cancel the active operation before closing.', buttons: ['OK'] }); } });
  ipcMain.handle('snapshot', () => state);
  ipcMain.handle('command', async (event, name, value) => {
    if (event.sender !== win.webContents || event.senderFrame !== win.webContents.mainFrame || typeof name !== 'string') return;
    try { await command(name, value); return { ok: true }; }
    catch (e) { state.error = safeError(e); push(); return { ok: false }; }
  });
  await win.loadURL('palisade://app/src/ui/index.html');
  await operation(async signal => {
    await refresh();
    if (settings.value.pending) {
      const completed = await settings.recoverPending(windows.inspectInstallation);
      state.recovery = completed ? 'The previous installation completed and its version was verified.' : 'The previous installation was interrupted. Check the application folder and retry Repair.';
    }
    await settings.save(); state.phase = 'ready'; state.message = 'Ready when you are.'; state.error = settings.warning;
    const bundled = path.join(app.isPackaged ? process.resourcesPath : path.join(__dirname, '..'), 'offline');
    try {
      await fs.access(path.join(bundled, 'release.json'));
      offer = await updates.offline(bundled, key); updates.enforceVersion(offer.manifest, state.current?.version, settings.value.highestVersion);
      await cacheNotes(offer);
      settings.value.highestVersion = offer.manifest.version; await settings.save();
      state.available = offer.manifest; state.notes = offer.notes; state.source = 'offline'; state.message = 'Bundled offline package verified.';
    } catch (e) { if (e.code !== 'ENOENT') { offer = null; state.error = 'The bundled offline package could not be verified. Check online or select another signed bundle.'; } }
    // Discover the latest signed release on every launch, including fresh installations.
    // Keep a verified offline offer available if the online check fails.
    await check(signal);
  });
}
if (locked) {
  app.on('second-instance', (_event, argv) => {
    if (win) { if (win.isMinimized()) win.restore(); win.show(); win.focus(); }
    const argument = argv.indexOf('--installation');
    if (argument !== -1 && !busy) void operation(async signal => {
      selected = windows.validatePath(argv[argument + 1]);
      await refresh(); await check(signal);
    });
  });
  app.whenReady().then(initialize).catch(async () => { await log('startup.failed'); await dialog.showMessageBox({ type: 'error', message: 'Palisade Installer could not load its interface.', detail: 'Re-download the utility and try again. If the problem continues, contact support. No client installer was run.' }); app.quit(); });
  app.on('window-all-closed', () => app.quit());
}
