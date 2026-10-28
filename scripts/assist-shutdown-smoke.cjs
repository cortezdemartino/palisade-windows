'use strict';
// Real Electron quit/windows, fake cleanup holder. No native input or account.
const { app, BrowserWindow } = require('electron');
const path = require('node:path'), fs = require('node:fs'), assert = require('node:assert/strict');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.resolve(__dirname, '..');
const { createAssistLifecycle, installQuitBarrier } = require(path.join(root, 'src/assist/lifecycle.cjs'));
app.disableHardwareAcceleration();
app.setPath('userData', path.resolve('out/assist-shutdown-profile'));
app.on('window-all-closed', () => {});
const life = createAssistLifecycle();
let window, release, blocked = 0, revoked = false, ready = false, flushed = 0, prepared = 0, notices = 0;
const cleanup = new Promise(resolve => { release = resolve; });
life.hold({ revoke() { revoked = true; }, wait: () => ready ? cleanup : Promise.reject(Error('cleanup blocked')) });
installQuitBarrier(app, {
  prepare: async () => { prepared++; await life.quiesce(); flushed++; },
  onBlocked() { blocked++; },
  onWaiting() { notices++; }, noticeAfterMs: 25,
});
const pause = time => new Promise(resolve => setTimeout(resolve, time));
const deadline = setTimeout(() => { console.error('Shutdown fixture deadline'); app.exit(1); }, 15000);
app.whenReady().then(async () => {
  window = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true,
    nodeIntegration: false, javascript: false, devTools: false, partition: 'assist-shutdown-fixture' } });
  await window.loadURL('about:blank');
  window.on('close', () => { assert.equal(life.status().state, 'closed'); assert.equal(flushed, 1); });
  app.quit();
  assert.equal(revoked, true); assert.equal(window.isDestroyed(), false);
  await pause(75);
  assert.equal(blocked, 1); assert.equal(flushed, 0); assert.equal(notices, 0); assert.equal(window.isDestroyed(), false);
  ready = true; app.quit(); app.quit();
  await pause(75);
  assert.equal(prepared, 2); assert.equal(window.isDestroyed(), false); assert.equal(flushed, 0); assert.equal(notices, 1);
  release();
}).catch(error => { console.error(error.message); clearTimeout(deadline); app.exit(1); });
app.on('will-quit', () => {
  try {
    assert.equal(life.status().state, 'closed'); assert.equal(life.status().holders, 0);
    assert.equal(flushed, 1); assert.equal(blocked, 1); assert.equal(revoked, true); assert.equal(notices, 1);
    assert.equal(window.isDestroyed(), true);
    fs.writeFileSync(path.resolve('out/assist-shutdown-result.json'), JSON.stringify({ passed: true,
      electron: process.versions.electron, actualQuitBlocked: true, cleanupRetained: true,
      failureRetry: true, pendingStatus: true, fakeHolder: true, inputInjected: false }));
    clearTimeout(deadline); console.log('Actual Electron quit waits for cleanup, blocks failure and supports retry.');
  } catch (error) { console.error(error.message); clearTimeout(deadline); app.exit(1); }
});
