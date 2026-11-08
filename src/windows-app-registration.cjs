'use strict';
const path = require('node:path');
const { mkdir, access, unlink } = require('node:fs/promises');
const { execFile } = require('node:child_process');
const { APP_ID } = require('./windows-notification-branding.cjs');

function appDetails({ packaged, platform = process.platform, exe = process.execPath }) {
  if (!packaged || platform !== 'win32') return null;
  if (!path.win32.isAbsolute(exe) || path.win32.basename(exe).toLowerCase() !== 'palisade.exe' || exe.includes('"')) {
    throw new Error('Invalid Palisade executable for Windows registration.');
  }
  return { appId: APP_ID, appIconPath: exe, appIconIndex: 0,
    relaunchCommand: '"' + exe + '"', relaunchDisplayName: 'Palisade' };
}

async function ensureAppRegistration({ packaged, platform = process.platform, exe = process.execPath,
  appData = process.env.APPDATA, commonAppData = process.env.ProgramData,
  systemRoot = process.env.SystemRoot || 'C:\\Windows',
  shell, makeDirectory = mkdir, checkFile = access, deleteFile = unlink, run = execFile }) {
  const details = appDetails({ packaged, platform, exe });
  if (!details) return;
  if (!appData || !path.win32.isAbsolute(appData)) throw new Error('Windows Start menu location is unavailable.');
  await checkFile(exe);
  const directory = path.win32.join(appData, 'Microsoft', 'Windows', 'Start Menu', 'Programs');
  let shortcut = path.win32.join(directory, 'Palisade.lnk');
  const wanted = { target: exe, cwd: path.win32.dirname(exe), args: '', description: 'Palisade',
    icon: exe, iconIndex: 0, appUserModelId: APP_ID };
  let previous;
  try { previous = shell.readShortcutLink(shortcut); } catch { /* Missing or damaged app shortcut. */ }
  // Reuse a correct all-users installation entry rather than creating an extra
  // per-user shortcut that its all-users uninstaller would not own.
  if (!previous && commonAppData && path.win32.isAbsolute(commonAppData)) {
    const commonShortcut = path.win32.join(commonAppData, 'Microsoft', 'Windows', 'Start Menu', 'Programs', 'Palisade.lnk');
    let common;
    try { common = shell.readShortcutLink(commonShortcut); } catch { /* No installed all-users entry. */ }
    if (common && Object.keys(wanted).filter(key => key !== 'description').every(key => common[key] === wanted[key])) {
      shortcut = commonShortcut; previous = common;
    }
  }
  if (previous && previous.appUserModelId !== APP_ID &&
      previous.target?.toLowerCase() !== exe.toLowerCase()) {
    throw new Error('The Palisade Start menu name belongs to another application.');
  }
  if (!previous || Object.keys(wanted).some(key => key !== 'description' && previous[key] !== wanted[key])) {
    await makeDirectory(directory, { recursive: true });
    if (!shell.writeShortcutLink(shortcut, previous ? 'replace' : 'create', wanted)) {
      throw new Error('Windows could not register the Palisade Start menu shortcut.');
    }
    const actual = shell.readShortcutLink(shortcut);
    if (Object.keys(wanted).some(key => actual[key] !== wanted[key])) {
      throw new Error('Windows Start menu registration did not match the installed application.');
    }
  }
  // The preferred Win32 executable lookup registration; never changes PATH or
  // takes file/protocol associations away from other applications.
  const key = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\Palisade.exe';
  for (const args of [
    ['add', key, '/ve', '/t', 'REG_SZ', '/d', exe, '/f'],
    ['add', key, '/v', 'Path', '/t', 'REG_SZ', '/d', path.win32.dirname(exe), '/f'],
  ]) {
    await new Promise((resolve, reject) => run(path.win32.join(systemRoot, 'System32', 'reg.exe'), args,
      { windowsHide: true, shell: false, timeout: 10000 }, error => error ? reject(error) : resolve()));
  }
  // Older development notifications could create Electron.lnk with Palisade's
  // production ID. That duplicate can take precedence in Windows' app catalog.
  // Remove only our misregistered development link, never Electron's own entry.
  const developmentShortcut = path.win32.join(directory, 'Electron.lnk');
  let development;
  try { development = shell.readShortcutLink(developmentShortcut); } catch { /* No readable development entry. */ }
  if (development?.appUserModelId === APP_ID &&
      typeof development.target === 'string' &&
      path.win32.basename(development.target).toLowerCase() === 'electron.exe') {
    await deleteFile(developmentShortcut);
  }
  return { shortcut, appId: APP_ID };
}
module.exports = { appDetails, ensureAppRegistration };
