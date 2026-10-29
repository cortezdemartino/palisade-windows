'use strict';
const { app, ipcMain, nativeImage } = require('electron');
const path = require('node:path');
const fs = require('node:fs');
const assert = require('node:assert/strict');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, '..');
const { createDesktopWindow } = require(path.join(root, 'src/desktop-window.cjs'));
app.setPath('userData', path.join(__dirname, '../out/chat-smoke-profile'));
let owner;
const deadline = setTimeout(() => { console.error('Chat observer test timed out'); app.exit(1); }, 20000);
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
app.whenReady().then(async () => {
  const desktop = createDesktopWindow({ partition: 'chat-notifications-test' }); owner = desktop.window;
  const snapshots = [];
  async function waitForCount(count) {
    const until = Date.now() + 3000;
    while (snapshots.at(-1)?.count !== count && Date.now() < until) await pause(50);
    assert.equal(snapshots.at(-1)?.count, count);
  }
  ipcMain.on('palisade:unread-state', (event, value) => { if (event.sender === desktop.contents) snapshots.push(value); });
  await desktop.contents.loadURL('data:text/html,' + encodeURIComponent('<nav aria-label="Workspaces"><button aria-label="Create a workspace"></button><button class="rail__item" id="dm" aria-label="Direct messages"></button></nav>'));
  await waitForCount(0); assert.equal(snapshots.at(-1).baseline, true);
  assert.equal(await desktop.contents.executeJavaScript('typeof window.desktopControls'), 'undefined');
  await pause(5000);
  await desktop.contents.executeJavaScript('document.getElementById("dm").innerHTML = "<span class=rail__badge>2</span>"; document.getElementById("dm").setAttribute("aria-label", "Direct messages, 2 unread")');
  await waitForCount(2); assert.equal(snapshots.at(-1).baseline, false);
  await desktop.contents.executeJavaScript('document.getElementById("dm").setAttribute("aria-label", "Direct messages")');
  await waitForCount(0);
  const unread = nativeImage.createFromPath(path.join(root, 'assets/icon-unread.png'));
  if (process.env.PALISADE_WINDOW_SMOKE_ASAR) {
    const brandingIcon = nativeImage.createFromPath(path.join(path.dirname(root), 'notification-icon.png'));
    assert.equal(brandingIcon.isEmpty(), false, 'Windows notification logo must exist outside the application archive');
  }
  assert.equal(unread.isEmpty(), false); owner.setIcon(nativeImage.createFromPath(path.join(root, 'assets/icon.ico')));
  const pixels = unread.toBitmap();
  let red = 0; for (let i = 0; i < pixels.length; i += 4) if (pixels[i+2] > 200 && pixels[i+1] < 120 && pixels[i] < 120 && pixels[i+3] > 0) red++;
  assert.ok(red > 100, 'Unread icon must contain bright red markings');
  fs.writeFileSync(path.join(__dirname, '../out/chat-notifications-result.json'), JSON.stringify({ passed: true, baseline: true, unreadAndRead: true, noPageBridge: true, redMarkings: true }));
  owner.destroy(); clearTimeout(deadline); app.exit(0);
}).catch(error => { console.error(error); owner?.destroy(); clearTimeout(deadline); app.exit(1); });
