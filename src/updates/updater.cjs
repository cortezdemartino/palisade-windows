'use strict';

const { app, dialog, Notification } = require('electron');
const { spawn } = require('node:child_process');
const { mkdtemp, mkdir, rm, readFile, writeFile } = require('node:fs/promises');
const path = require('node:path');
const { fetchRelease, verifyDownloadedFile } = require('./release.cjs');
const { downloadInstaller } = require('./downloader.cjs');

const CHECK_PERIOD_MS = 6 * 60 * 60 * 1000;
const PUBLIC_KEY_PATH = path.join(__dirname, '..', '..', 'assets', 'update-public-key.pem');

function createUpdateController(getWindow, onStatus = () => {}) {
  let busy = false;
  let offered = false;
  let ready = null;
  let stopping = false;
  const receipt = path.join(app.getPath('userData'), 'update-result.json');
  const cache = path.resolve(app.getPath('userData'), 'palisade-update-cache');
  if (path.dirname(cache) !== path.resolve(app.getPath('userData'))) throw new Error('Invalid update cache path.');
  // Only this dedicated cache is disposable. Browser profile storage is never touched.
  const initialized = rm(cache, { recursive: true, force: true }).catch(() => {}).then(() => mkdir(cache, { recursive: true }));
  void initialized.catch(() => {});
  function status(text) { onStatus(text); }

  function notify(options) {
    const win = getWindow();
    return win && !win.isDestroyed()
      ? dialog.showMessageBox(win, { ...options, noLink: true })
      : dialog.showMessageBox({ ...options, noLink: true });
  }

  async function install(download) {
    if (!await verifyDownloadedFile(download.file, download.manifest)) {
      throw new Error('The update installer failed its integrity check.');
    }
    if (stopping) return;
    // Run outside ASAR after the parent exits, so NSIS can replace locked files.
    const helper = path.join(download.directory, 'install.ps1');
    const handshake = path.join(download.directory, 'helper-ready');
    await rm(handshake, { force: true });
    await writeFile(helper, await readFile(path.join(__dirname, 'install.ps1')));
    await writeFile(receipt, JSON.stringify({ version: download.manifest.version, exitCode: null }));
    const powershell = path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
    const processHandle = spawn(powershell, ['-NoProfile', '-NonInteractive', '-File', helper,
      '-ParentId', String(process.pid), '-Installer', download.file, '-Executable', process.execPath,
      '-Receipt', receipt, '-Version', download.manifest.version, '-Sha256', download.manifest.sha256,
      '-Handshake', handshake], {
      detached: true, windowsHide: true, stdio: 'ignore', shell: false,
    });
    await new Promise((resolve, reject) => {
      processHandle.once('error', reject);
      processHandle.once('spawn', resolve);
    });
    try {
      let started = false;
      for (let attempt = 0; attempt < 100; attempt++) {
        try { started = (await readFile(handshake, 'utf8')).trim() === 'ready'; } catch {}
        if (started) break;
        if (processHandle.exitCode !== null) throw new Error('Installer helper could not start.');
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      if (!started) throw new Error('Installer helper startup timed out.');
    } catch (error) {
      processHandle.kill();
      throw error;
    }
    processHandle.unref();
    app.quit();
  }

  async function askToInstall(download) {
    if (offered) return;
    offered = true;
    try {
      const result = await notify({
        type: 'question', title: 'Palisade update ready',
        message: `Palisade Desktop ${download.manifest.version} is ready to install.`,
        detail: 'The release signature, SHA-256 and size were verified. Save your work before restarting. Windows may show an unknown-publisher warning.\n\nRelease notes from GitHub:\n' + (download.manifest.releaseNotes || 'No release notes provided.'),
        buttons: ['Install update', 'Later'], defaultId: 1, cancelId: 1,
      });
      if (result.response === 0) await install(download);
    } finally {
      offered = false;
    }
  }

  async function check(manual = false) {
    if (busy || offered || stopping) return;
    if (process.platform !== 'win32' || !app.isPackaged) {
      if (manual) await notify({ type: 'info', message: 'Updates are available in installed Windows builds only.', buttons: ['OK'] });
      return;
    }
    busy = true;
    status('Updates: checking');
    let workingDirectory;
    try {
      await initialized;
      const key = await readFile(PUBLIC_KEY_PATH, 'utf8');
      const manifest = await fetchRelease(fetch, key, app.getVersion());
      if (!manifest) {
        status('Updates: up to date');
        if (manual) await notify({ type: 'info', message: 'Palisade Desktop is up to date.', buttons: ['OK'] });
        return;
      }
      if (ready && ready.manifest.version === manifest.version &&
          await verifyDownloadedFile(ready.file, manifest)) {
        status(`Update ${manifest.version} ready`);
        await askToInstall(ready);
        return;
      }
      workingDirectory = await mkdtemp(path.join(cache, 'download-'));
      let lastPercent = -1;
      const file = await downloadInstaller(manifest, workingDirectory, fetch, (bytes, total) => {
        const percent = Math.floor(bytes * 100 / total);
        if (percent === lastPercent) return;
        lastPercent = percent;
        status(`Downloading update: ${percent}%`);
        getWindow()?.setProgressBar(bytes / total);
      });
      if (ready) await rm(ready.directory, { recursive: true, force: true });
      ready = { file, manifest, directory: workingDirectory };
      workingDirectory = undefined;
      getWindow()?.setProgressBar(-1);
      status(`Update ${manifest.version} ready`);
      if (Notification.isSupported()) {
        const notice = new Notification({ title: 'Palisade update ready', body: `Version ${manifest.version} is ready to install.` });
        notice.on('click', () => { getWindow()?.show(); void askToInstall(ready).catch(() => status('Updates: installation failed; retry from menu')); });
        notice.show();
      }
      await askToInstall(ready);
    } catch (error) {
      status('Updates: failed; check again to retry');
      if (manual || ready) await notify({ type: 'warning', title: 'Update unavailable',
        message: 'Palisade could not verify or install an update.',
        detail: 'The current version has not been changed. Check your connection and try again. If the problem continues, contact Palisade support.',
        buttons: ['OK'] });
      // Keep update failures out of the message renderer and do not expose untrusted URLs.
    } finally {
      getWindow()?.setProgressBar(-1);
      if (workingDirectory) await rm(workingDirectory, { recursive: true, force: true }).catch(() => status('Updates: cleanup pending; retry after restart'));
      busy = false;
    }
  }

  function start() {
    void readFile(receipt, 'utf8').then(async (text) => {
      const result = JSON.parse(text.replace(/^\uFEFF/, ''));
      await rm(receipt, { force: true });
      if ((result.exitCode !== 0 && result.exitCode !== null) || result.version !== app.getVersion()) {
        status('Updates: previous installation did not complete');
        await notify({ type: 'warning', message: 'The previous Palisade update did not complete.',
          detail: 'Your profile has been retained. Check for updates to retry, or run the official installer manually.', buttons: ['OK'] });
      }
    }).catch(() => {});
    const first = setTimeout(() => void check(), 15_000);
    const repeat = setInterval(() => void check(), CHECK_PERIOD_MS);
    first.unref?.();
    repeat.unref?.();
    app.on('before-quit', () => { stopping = true; clearTimeout(first); clearInterval(repeat); });
  }

  return { check, start };
}

module.exports = { createUpdateController };
