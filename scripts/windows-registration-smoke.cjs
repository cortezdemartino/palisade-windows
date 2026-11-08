'use strict';
const { app, shell, BrowserWindow } = require('electron');
const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, '..');
const { ensureAppRegistration, appDetails } = require(path.join(root, 'src/windows-app-registration.cjs'));
const { APP_ID } = require(path.join(root, 'src/windows-notification-branding.cjs'));
const fixture = path.join(__dirname, '../out/windows-shell-fixture');
const result = path.join(__dirname, '../out/windows-registration-result.json');
app.setName('Palisade shell fixture');
app.setAppUserModelId(APP_ID + '.registration-fixture');
app.setPath('userData', path.join(fixture, 'profile'));
// Closing the temporary window must not exit before the evidence write ends.
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => { console.error('Windows registration fixture timed out'); app.exit(1); }, 30000);
app.whenReady().then(async () => {
  await fs.mkdir(fixture, { recursive: true });
  const exe = process.env.PALISADE_WINDOW_SMOKE_ASAR
    ? path.resolve(root, '../../Palisade.exe') : path.join(fixture, 'Palisade.exe');
  if (!process.env.PALISADE_WINDOW_SMOKE_ASAR) await fs.copyFile(process.execPath, exe);
  const appData = path.join(fixture, 'roaming');
  const programs = path.join(appData, 'Microsoft/Windows/Start Menu/Programs');
  await fs.mkdir(programs, { recursive: true });
  const legacy = path.join(programs, 'Electron.lnk');
  assert(shell.writeShortcutLink(legacy, 'create', { target: process.execPath, appUserModelId: APP_ID }));
  const commands = [];
  const options = { packaged: true, exe, appData, shell,
    run: (file, args, config, done) => { commands.push({ file, args, config }); done(null); } };
  const registered = await ensureAppRegistration(options);
  const actual = shell.readShortcutLink(registered.shortcut);
  assert.equal(actual.target, exe); assert.equal(actual.appUserModelId, APP_ID);
  assert.equal(actual.icon, exe); assert.equal(actual.args, '');
  assert.equal(actual.cwd, path.dirname(exe));
  await assert.rejects(fs.access(legacy));
  assert.equal(commands.length, 2); assert(commands.every(c => c.config.shell === false));
  assert(shell.writeShortcutLink(legacy, 'create', { target: process.execPath, appUserModelId: 'org.electron.app' }));
  await ensureAppRegistration(options);
  assert.equal(shell.readShortcutLink(legacy).appUserModelId, 'org.electron.app');
  const window = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false } });
  // Exercise native property setters without claiming the real app ID on this
  // development process or adding anything to the user's Start menu.
  window.setAppDetails({ ...appDetails({ packaged: true, exe }), appId: APP_ID + '.registration-fixture' });
  window.destroy();
  await fs.writeFile(result, JSON.stringify({ passed: true, electron: process.versions.electron,
    nativeShortcutRoundtrip: true, windowRelaunchMetadata: true, legacyConflictRemoved: true,
    foreignShortcutPreserved: true, registryCommandsIntercepted: true, personalStartMenuModified: false }));
  clearTimeout(deadline); app.exit(0);
}).catch(error => { console.error(error.stack || String(error)); clearTimeout(deadline); app.exit(1); });
