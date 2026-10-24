'use strict';
const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, '..');
const { createDesktopWindow } = require(path.join(root, 'src/desktop-window.cjs'));
const { installSessionPolicy } = require(path.join(root, 'src/session-policy.cjs'));
app.disableHardwareAcceleration();
// Synthetic devices only. Permission UI and permission handlers are not bypassed.
app.commandLine.appendSwitch('use-fake-device-for-media-stream');
app.setPath('userData', path.join(__dirname, '../out/permission-smoke-profile'));
let owner, other;
const results = {};
const deadline = setTimeout(() => { console.error('Permission smoke timed out'); app.exit(1); }, 30000);
app.whenReady().then(async () => {
  const desktop = createDesktopWindow({ partition: 'permission-smoke' });
  owner = desktop.window;
  const contents = desktop.contents;
  const profile = contents.session;
  // Test-only HTTPS fixture interception: no real website, credentials or TLS override.
  profile.protocol.handle('https', () => new Response('<!doctype html><title>Permission fixture</title><body>Permission fixture</body>', {
    headers: { 'Content-Type': 'text/html' },
  }));
  installSessionPolicy(profile, () => contents);
  await contents.loadURL('https://palisade.onyxops.org/permission-fixture');
  const denied = async (target, expression, label) => {
    const name = await target.executeJavaScript('(async () => { try { const stream = await (' + expression + '); stream.getTracks().forEach(track => track.stop()); return "GRANTED"; } catch (error) { return error.name; } })()', true);
    assert.ok(['NotAllowedError', 'AbortError', 'SecurityError'].includes(name), label + ': ' + name);
    results[label] = 'denied';
  };
  await denied(contents, 'navigator.mediaDevices.getUserMedia({video:true})', 'camera');
  await denied(contents, 'navigator.mediaDevices.getUserMedia({audio:true,video:true})', 'mixedMedia');
  await denied(contents, 'navigator.mediaDevices.getDisplayMedia({video:true,audio:false})', 'screen');
  const microphone = await contents.executeJavaScript('(async () => { const stream = await navigator.mediaDevices.getUserMedia({audio:true}); const kinds=stream.getTracks().map(track=>track.kind); stream.getTracks().forEach(track=>track.stop()); return kinds; })()', true);
  assert.deepEqual(microphone, ['audio']); results.syntheticMicrophone = 'allowed-and-stopped';
  assert.equal(await contents.executeJavaScript('Notification.requestPermission()', true), 'granted');
  results.notifications = 'permission-granted-no-toast';
  await contents.executeJavaScript('new Promise(resolve => { const frame=document.createElement("iframe"); frame.id="child"; frame.src="https://palisade.onyxops.org/frame-fixture"; frame.onload=resolve; document.body.appendChild(frame); })');
  await denied(contents, 'document.getElementById("child").contentWindow.navigator.mediaDevices.getUserMedia({audio:true})', 'sameOriginSubframe');
  other = new BrowserWindow({ show: false, webPreferences: { partition: 'permission-smoke', sandbox: true, contextIsolation: true, nodeIntegration: false } });
  await other.loadURL('https://palisade.onyxops.org/other-fixture');
  await denied(other.webContents, 'navigator.mediaDevices.getUserMedia({audio:true})', 'wrongContents');
  const exposed = await contents.executeJavaScript('[typeof require, typeof process, typeof window.desktopControls]');
  assert.deepEqual(exposed, ['undefined', 'undefined', 'undefined']); results.noPrivilegedPageBridge = true;
  await assert.rejects(contents.loadURL('https://example.test/blocked'));
  results.externalNavigation = 'blocked';
  fs.writeFileSync(path.join(__dirname, '../out/session-policy-result.json'), JSON.stringify({ passed: true, runtime: process.versions.electron, ...results }));
  other.destroy(); owner.destroy(); clearTimeout(deadline); app.exit(0);
}).catch(error => { console.error(error); other?.destroy(); owner?.destroy(); clearTimeout(deadline); app.exit(1); });
