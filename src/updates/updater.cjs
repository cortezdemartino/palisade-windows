'use strict';

const { app, dialog } = require('electron');
const { spawn } = require('node:child_process');
const { mkdtemp, rm, readFile } = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { fetchRelease, verifyDownloadedFile } = require('./release.cjs');
const { downloadInstaller } = require('./downloader.cjs');

const CHECK_PERIOD_MS = 6 * 60 * 60 * 1000;
const PUBLIC_KEY_PATH = path.join(__dirname, '..', '..', 'assets', 'update-public-key.pem');

function createUpdateController(getWindow) {
  let busy = false;
  let offered = false;
  let ready = null;

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
    // NSIS per-user installers support /S. Windows still enforces its own policies.
    const processHandle = spawn(download.file, ['/S'], {
      detached: true, windowsHide: false, stdio: 'ignore', shell: false,
    });
    await new Promise((resolve, reject) => {
      processHandle.once('error', reject);
      processHandle.once('spawn', resolve);
    });
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
        detail: 'GitHub Releases provided the installer. Its release signature and file hash were verified. Save your work before restarting. Windows may show an unknown-publisher warning.',
        buttons: ['Install update', 'Later'], defaultId: 1, cancelId: 1,
      });
      if (result.response === 0) await install(download);
    } finally {
      offered = false;
    }
  }

  async function check(manual = false) {
    if (busy || offered) return;
    if (process.platform !== 'win32' || !app.isPackaged) {
      if (manual) await notify({ type: 'info', message: 'Updates are available in installed Windows builds only.', buttons: ['OK'] });
      return;
    }
    busy = true;
    let workingDirectory;
    try {
      const key = await readFile(PUBLIC_KEY_PATH, 'utf8');
      const manifest = await fetchRelease(fetch, key, app.getVersion());
      if (!manifest) {
        if (manual) await notify({ type: 'info', message: 'Palisade Desktop is up to date.', buttons: ['OK'] });
        return;
      }
      if (ready && ready.manifest.version === manifest.version &&
          await verifyDownloadedFile(ready.file, manifest)) {
        await askToInstall(ready);
        return;
      }
      workingDirectory = await mkdtemp(path.join(os.tmpdir(), 'palisade-update-'));
      const file = await downloadInstaller(manifest, workingDirectory);
      if (ready) await rm(ready.directory, { recursive: true, force: true });
      ready = { file, manifest, directory: workingDirectory };
      workingDirectory = undefined;
      await askToInstall(ready);
    } catch (error) {
      if (manual) await notify({ type: 'warning', title: 'Update unavailable',
        message: 'Palisade could not verify or install an update.',
        detail: 'The current version has not been changed. Check your connection and try again. If the problem continues, contact Palisade support.',
        buttons: ['OK'] });
      // Keep update failures out of the message renderer and do not expose untrusted URLs.
    } finally {
      if (workingDirectory) await rm(workingDirectory, { recursive: true, force: true });
      busy = false;
    }
  }

  function start() {
    const first = setTimeout(() => void check(), 15_000);
    const repeat = setInterval(() => void check(), CHECK_PERIOD_MS);
    first.unref?.();
    repeat.unref?.();
    app.on('before-quit', () => { clearTimeout(first); clearInterval(repeat); });
  }

  return { check, start };
}

module.exports = { createUpdateController };
