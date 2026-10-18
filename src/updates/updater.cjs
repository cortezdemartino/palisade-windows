'use strict';
const { app, dialog } = require('electron');
const path = require('node:path');
const fs = require('node:fs/promises');
const { spawn } = require('node:child_process');

// Installation, discovery and downloads belong exclusively to the bundled manager.
function createUpdateController(getWindow, onStatus = () => {}) {
  let busy = false;
  function notify(options) {
    const win = getWindow();
    return win && !win.isDestroyed()
      ? dialog.showMessageBox(win, { ...options, noLink: true })
      : dialog.showMessageBox({ ...options, noLink: true });
  }
  async function check(manual = false) {
    if (!manual || busy) return;
    busy = true;
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
    } finally { busy = false; }
  }
  function start() { onStatus('Updates: managed by Palisade Manager'); }
  return { check, start };
}
module.exports = { createUpdateController };
