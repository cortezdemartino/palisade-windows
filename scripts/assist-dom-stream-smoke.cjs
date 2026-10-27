'use strict';
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('node:path'),
  assert = require('node:assert/strict'),
  fs = require('node:fs');
app.disableHardwareAcceleration();
app.setPath('userData', path.join(__dirname, '../out/assist-dom-stream-profile'));
let window;
const timeout = setTimeout(() => app.exit(1), 15000);
app
  .whenReady()
  .then(async () => {
    window = new BrowserWindow({
      show: false,
      webPreferences: {
        partition: 'assist-dom-stream-fixture',
        preload: path.join(__dirname, 'fixtures/assist-dom-stream-preload.cjs'),
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        webSecurity: true,
      },
    });
    window.webContents.session.setPermissionCheckHandler(() => false);
    window.webContents.session.setPermissionRequestHandler((_contents, _permission, done) =>
      done(false),
    );
    window.webContents.session.protocol.handle(
      'https',
      () =>
        new Response('<!doctype html><title>Owned synthetic stream</title><body></body>', {
          headers: { 'Content-Type': 'text/html', 'Content-Security-Policy': "default-src 'none'" },
        }),
    );
    const ready = new Promise((resolve) =>
      ipcMain.once('palisade:test-stream-ready', (event) => {
        assert.equal(event.sender, window.webContents);
        assert.equal(event.senderFrame, window.webContents.mainFrame);
        resolve();
      }),
    );
    await window.loadURL('https://palisade.onyxops.org/synthetic-stream');
    await ready;
    const value = await window.webContents.executeJavaScript(
      `(()=>{const stream=document.getElementById('owned-synthetic-stream').srcObject;return {stream:stream instanceof MediaStream,tracks:stream.getVideoTracks().length,state:stream.getTracks()[0].readyState,node:typeof require,ipc:typeof ipcRenderer}})()`,
    );
    assert.deepEqual(value, {
      stream: true,
      tracks: 1,
      state: 'live',
      node: 'undefined',
      ipc: 'undefined',
    });
    fs.writeFileSync(
      path.resolve('out/assist-dom-stream-result.json'),
      JSON.stringify({
        passed: true,
        isolatedDomStream: true,
        nodeUnavailable: true,
        electron: process.versions.electron,
      }),
    );
    window.destroy();
    clearTimeout(timeout);
    console.log(
      'Owned synthetic stream crosses isolated DOM through srcObject without a native bridge.',
    );
    app.exit(0);
  })
  .catch((error) => {
    console.error(error.message);
    window?.destroy();
    clearTimeout(timeout);
    app.exit(1);
  });
