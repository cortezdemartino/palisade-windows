'use strict';
const { app, BrowserWindow } = require('electron');
const path = require('node:path');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, '..');
const { createDesktopWindow } = require(path.join(root, 'src/desktop-window.cjs'));
const { installAssistBridge } = require(path.join(root, 'src/assist/bridge.cjs'));
const { installSessionPolicy } = require(path.join(root, 'src/session-policy.cjs'));
const { chooseSource } = require(path.join(root, 'src/assist/source-picker.cjs'));
app.disableHardwareAcceleration();
app.setPath('userData', path.join(__dirname, '../out/assist-smoke-profile'));
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const deadline = setTimeout(() => { console.error('Assist fixture timed out during', phase); app.exit(1); }, 40000);
let owner, phase = "initialization";
app.whenReady().then(async () => {
  const desktop = createDesktopWindow({ partition: 'assist-smoke' }); owner = desktop.window;
  const contents = desktop.contents, profile = contents.session;
  profile.protocol.handle('https', () => new Response('<!doctype html><title>Assist fixture</title><body style="background:#3574af;color:white">Palisade synthetic capture fixture</body>', { headers: { 'Content-Type': 'text/html' } }));
  installSessionPolicy(profile, () => contents);
  const bridge = installAssistBridge(contents, { desktopVersion: '0.3.0' });
  await contents.loadURL('https://palisade.onyxops.org/assist-fixture');
  phase = "bridge shape";
  const keys = await contents.executeJavaScript('Object.keys(window.palisadeDesktopAssist).sort().join(",")');
  assert.equal(keys, 'getCapabilities,onCapabilitiesChanged,protocol');
  phase = "capability request";
  const report = await contents.executeJavaScript('window.palisadeDesktopAssist.getCapabilities({protocol:"palisade.assist.bridge",versions:[1]})');
  assert.equal(report.capture, 'unavailable'); assert.equal(report.nativeEndpoint, 'absent');
  phase = "subframe";
  await contents.executeJavaScript('new Promise(resolve=>{const frame=document.createElement("iframe");frame.id="child";frame.src="https://palisade.onyxops.org/frame";frame.onload=resolve;document.body.append(frame)})');
  assert.equal(await contents.executeJavaScript('typeof document.getElementById("child").contentWindow.palisadeDesktopAssist'), 'undefined');
  assert.equal(await contents.executeJavaScript('typeof sourcePicker'), 'undefined');
  const controls = owner.contentView.children[1].webContents;
  assert.equal(await controls.executeJavaScript('typeof palisadeDesktopAssist'), 'undefined');
  const sources = [{ id: 'window:11:0', name: '<script>private-title</script>', thumbnail: { isEmpty: () => true } }];
  phase = "picker";
  for (const select of [false, true]) {
    const abort = new AbortController(), result = chooseSource(sources, { parent: owner, signal: abort.signal });
    let picker;
    for (let i = 0; i < 60; i++) {
      picker = BrowserWindow.getAllWindows().find(win => win !== owner && win.webContents.getURL() === 'palisade-controls://app/assist-picker.html');
      if (picker && !picker.webContents.isLoadingMainFrame() && await picker.webContents.executeJavaScript('document.querySelectorAll("input").length')) break;
      await pause(50);
    }
    assert.ok(picker); assert.equal(await picker.webContents.executeJavaScript('document.getElementById("share").disabled'), true);
    assert.equal(await picker.webContents.executeJavaScript('document.querySelectorAll("script").length'), 1);
    if (select) await picker.webContents.executeJavaScript('document.querySelector("input").click();document.getElementById("share").click()');
    else await picker.webContents.executeJavaScript('document.getElementById("cancel").click()');
    assert.equal(await result, select ? 'window:11:0' : null);
  }
  phase = "capture denial and teardown";
  // Electron 44.7.2 represents modern and legacy desktop capture with empty mediaTypes.
  // Never approve that ambiguous permission just to reach the picker.
  // Consume Windows' first-show STARTUPINFO override in a hidden test process.
  owner.show(); owner.hide(); owner.show(); contents.focus(); await pause(100);
  const id = owner.getMediaSourceId();
  for (const expression of ["navigator.mediaDevices.getDisplayMedia({video:true,audio:false})",
    "navigator.mediaDevices.getUserMedia({video:{mandatory:{chromeMediaSource:\"desktop\",chromeMediaSourceId:" + JSON.stringify(id) + "}}})"]) {
    const result = await contents.executeJavaScript("(async()=>{try{const s=await ("+expression+");s.getTracks().forEach(t=>t.stop());return \"GRANTED\"}catch(e){return e.name}})()", true);
    assert.ok(["NotAllowedError", "AbortError"].includes(result));
  }
  const gone = new Promise(resolve => contents.once("render-process-gone", resolve));
  contents.forcefullyCrashRenderer(); await gone;
  fs.writeFileSync(path.join(__dirname, "../out/assist-smoke-result.json"), JSON.stringify({ passed: true,
    canonicalBridge: true, subframeDenied: true, pickerCancelAndChoose: true, modernAndLegacyDenied: true,
    rendererTermination: true, nativeCapture: "blocked-ambiguous-permission", electron: process.versions.electron }));
  bridge.dispose(); owner.destroy(); clearTimeout(deadline); app.exit(0);
}).catch(error => { console.error(phase, error?.stack || String(error)); owner?.destroy(); clearTimeout(deadline); app.exit(1); });
