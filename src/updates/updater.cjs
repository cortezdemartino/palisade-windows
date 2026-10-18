'use strict';
const { app, dialog, powerMonitor } = require('electron');
const { fetchRelease } = require('./release.cjs');
const STARTUP_DELAY = 10_000;
const CHECK_INTERVAL = 6 * 60 * 60 * 1000;
const RETRY_DELAY = 15 * 60 * 1000;
const path = require('node:path');
const fs = require('node:fs/promises');
const { spawn } = require('node:child_process');

// The client verifies release metadata. Downloads and installation stay in the manager.
function createUpdateController(getWindow, onStatus = () => {}, { onAvailable = () => {}, fetchFn = globalThis.fetch, now = Date.now } = {}) {
  let managerBusy = false, checking = false, started = false, stopped = false;
  let startupTimer, intervalTimer, retryTimer, request;
  let lastAttempt = null, failures = 0, notifiedVersion = null;
  const supported = () => app.isPackaged && process.platform === 'win32';
  function notify(options) {
    const win = getWindow();
    return win && !win.isDestroyed()
      ? dialog.showMessageBox(win, { ...options, noLink: true })
      : dialog.showMessageBox({ ...options, noLink: true });
  }
  async function openManager() {
    if (managerBusy || stopped) return;
    managerBusy = true;
    let directory;
    try {
      if (!app.isPackaged || process.platform !== 'win32') {
        await notify({ type: 'info', message: 'The update manager is included in installed Windows builds.', buttons: ['OK'] });
        return;
      }
      const answer = await notify({ type: 'question', title: 'Palisade Installer & Update Manager',
        message: 'Open the manager and exit Palisade?',
        detail: 'Save your work first. The manager checks for releases and handles installation, updates and repairs. Your profile is retained.',
        buttons: ['Open manager and exit', 'Cancel'], defaultId: 1, cancelId: 1 });
      if (answer.response !== 0) return;
      const source = path.join(process.resourcesPath, 'manager', 'Palisade-Manager.exe');
      // Run a copy outside the client folder so updates can replace the bundled manager.
      directory = await fs.mkdtemp(path.join(app.getPath('temp'), 'palisade-manager-'));
      const executable = path.join(directory, 'Palisade-Manager.exe');
      await fs.copyFile(source, executable, require('node:fs').constants.COPYFILE_EXCL);
      const child = spawn(executable, ['--installation', path.dirname(process.execPath)], {
        detached: true, stdio: 'ignore', shell: false, windowsHide: false
      });
      await new Promise((resolve, reject) => { child.once('error', reject); child.once('spawn', resolve); });
      child.unref();
      onStatus('Updates: manager opened');
      app.quit();
    } catch {
      if (directory) await fs.rm(directory, { recursive: true, force: true }).catch(() => {});
      onStatus('Updates: manager unavailable');
      await notify({ type: 'warning', message: 'Palisade could not open its update manager.',
        detail: 'Use the official Palisade Installer & Update Manager to update or repair this installation. Palisade has not been changed.', buttons: ['OK'] });
    } finally { managerBusy = false; }
  }
  async function check(manual = false) {
    if (checking || stopped) return;
    if (!supported()) {
      if (manual) await notify({ type: 'info', message: 'Update checks are available in packaged Windows builds.', buttons: ['OK'] });
      return;
    }
    checking = true;
    lastAttempt = now();
    clearTimeout(retryTimer);
    request = new AbortController();
    const currentRequest = request;
    onStatus('Updates: checking…');
    try {
      const root = path.join(__dirname, '..', '..', 'assets');
      const [key, history] = await Promise.all([
        fs.readFile(path.join(root, 'update-public-key.pem'), 'utf8'),
        fs.readFile(path.join(root, 'update-key-transitions.json'), 'utf8'),
      ]);
      const manifest = await fetchRelease((url, options = {}) => fetchFn(url, {
        ...options, signal: AbortSignal.any([currentRequest.signal, options.signal].filter(Boolean)),
      }), key, app.getVersion(), JSON.parse(history));
      if (stopped || currentRequest.signal.aborted) return;
      failures = 0;
      if (!manifest) {
        onStatus('Updates: up to date');
        if (manual) await notify({ type: 'info', message: 'Palisade is up to date.', buttons: ['OK'] });
        return;
      }
      onStatus('Updates: ' + manifest.version + ' available');
      if (manual) {
        const answer = await notify({ type: 'info', title: 'Palisade update available',
          message: 'Palisade ' + manifest.version + ' is available.',
          detail: (manifest.releaseNotes || 'A signed stable release is available.').slice(0, 6000),
          buttons: ['Open update manager', 'Later'], defaultId: 1, cancelId: 1 });
        if (answer.response === 0 && !stopped) await openManager();
      } else if (notifiedVersion !== manifest.version) {
        notifiedVersion = manifest.version;
        try { onAvailable(manifest, () => { void openManager(); }); } catch { /* Status remains visible if Windows cannot show a notification. */ }
      }
      return manifest;
    } catch {
      if (stopped || currentRequest.signal.aborted) return;
      failures++;
      onStatus('Updates: check failed; retry scheduled');
      if (started) {
        retryTimer = setTimeout(() => { void check(); }, Math.min(CHECK_INTERVAL, RETRY_DELAY * 2 ** Math.min(failures - 1, 5)));
        retryTimer.unref?.();
      }
      if (manual) await notify({ type: 'warning', message: 'Palisade could not verify the latest release.',
        detail: 'No update was installed. Check your connection and try again. Automatic checks will retry.', buttons: ['OK'] });
    } finally {
      checking = false;
      if (request === currentRequest) request = null;
    }
  }
  function resume() {
    if (lastAttempt === null || now() - lastAttempt >= CHECK_INTERVAL) void check();
  }
  function stop() {
    stopped = true; started = false;
    clearTimeout(startupTimer); clearTimeout(retryTimer); clearInterval(intervalTimer);
    request?.abort();
    powerMonitor.removeListener('resume', resume);
    app.removeListener('before-quit', stop);
  }
  function start() {
    if (started || !supported()) return;
    started = true; stopped = false;
    onStatus('Updates: automatic checks enabled');
    startupTimer = setTimeout(() => { void check(); }, STARTUP_DELAY);
    intervalTimer = setInterval(() => { void check(); }, CHECK_INTERVAL);
    startupTimer.unref?.(); intervalTimer.unref?.();
    powerMonitor.on('resume', resume);
    app.once('before-quit', stop);
  }
  return { check, openManager, start, stop };

}
module.exports = { createUpdateController };
