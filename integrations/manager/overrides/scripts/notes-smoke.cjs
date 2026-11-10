'use strict';
// Isolated UI fixture: no service calls, installer execution or installed profile access.
const { app, BrowserWindow, protocol, ipcMain } = require('electron');
const fs = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { randomUUID } = require('node:crypto');
app.disableHardwareAcceleration();
app.setPath('userData', path.join(app.getPath('temp'), `palisade-manager-notes-${randomUUID()}`));
protocol.registerSchemesAsPrivileged([{ scheme: 'palisade', privileges: { standard: true, secure: true } }]);
const base = process.env.PALISADE_MANAGER_SMOKE_ASAR || path.join(__dirname, '..');
const { resolveAsset } = require(path.join(base, 'src/ui-protocol.cjs'));
const deadline = setTimeout(() => { console.error('Manager notes fixture timed out.'); app.exit(1); }, 20000);
app.whenReady().then(async () => {
  protocol.handle('palisade', async (request) => {
    const asset = resolveAsset(request.url, request.method);
    if (!asset) return new Response(null, { status: 404 });
    return new Response(await fs.readFile(path.join(base, asset.relativePath)), { headers: { 'Content-Type': asset.mime } });
  });
  const note = { version: '0.4.0', locale: 'en', publishedAt: '2026-11-10T00:00:00.000Z',
    title: "What's New in Palisade 0.4.0", summary: 'UI fixture only.',
    sections: [{ title: 'Fixed', items: ['Text only: <img src=x onerror=alert(1)>'] }], compatibility: ['Windows x64.'] };
  const state = { phase: 'ready', current: { status: 'missing', path: 'C:\\fixture\\Palisade' },
    message: 'Isolated UI fixture.', notes: note.summary, releaseHistory: [note], history: [], preferences: { theme: 'dark' } };
  const win = new BrowserWindow({ show: false, width: 1050, height: 780, webPreferences: {
    preload: path.join(base, 'src/preload.cjs'), sandbox: true, contextIsolation: true, nodeIntegration: false } });
  win.webContents.session.webRequest.onBeforeRequest((details, callback) => callback({ cancel: !details.url.startsWith('palisade://app/') }));
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  ipcMain.handle('snapshot', (event) => { assert.equal(event.sender, win.webContents); return state; });
  ipcMain.handle('command', () => { throw Error('Commands are disabled in the UI fixture.'); });
  await win.loadURL('palisade://app/src/ui/index.html');
  const js = (script) => win.webContents.executeJavaScript(script);
  for (let attempt = 0; attempt < 50 && await js("document.getElementById('release-version').options.length") !== 1; attempt++)
    await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(await js('typeof window.require'), 'undefined');
  await js("document.querySelector('[data-page=release]').click()");
  assert.equal(await js("document.querySelector('#release-notes h2').textContent"), note.title);
  assert.equal(await js("document.querySelector('#release-notes img') === null"), true);
  assert.equal(await js("document.querySelector('#release-notes li').textContent"), note.sections[0].items[0]);
  await js("document.querySelector('#release-notes details summary').click()");
  assert.equal(await js("document.querySelector('#release-notes details').open"), true);
  if (process.argv[2]) {
    win.showInactive();
    await new Promise((resolve) => setTimeout(resolve, 300));
    await fs.writeFile(path.resolve(process.argv[2]), (await win.webContents.capturePage(undefined, { stayAwake: true })).toPNG());
  }
  console.log('Manager notes UI: version selection, text-only details, sandbox and local-only loading passed (UI fixture).');
  win.destroy(); clearTimeout(deadline); app.exit(0);
}).catch((error) => { console.error(error.message); clearTimeout(deadline); app.exit(1); });
