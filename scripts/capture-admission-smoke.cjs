'use strict';
const { app, BrowserWindow, session } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, '..');
const { installSessionPolicy } = require(path.join(root, 'src/session-policy.cjs'));

app.disableHardwareAcceleration();
app.on('window-all-closed', () => {});
app.commandLine.appendSwitch('use-fake-device-for-media-stream');
app.setPath('userData', path.join(__dirname, '../out/capture-admission-smoke-profile'));
const reportPath = process.env.PALISADE_CAPTURE_REPORT || path.join(__dirname, '../out/capture-admission-result.json');
const cases = [], failures = [];
let owner, contents, activeCase, controllerAvailable = false;
function writeReport() {
  const report = { passed: failures.length === 0, electron: process.versions.electron,
    fixtureCaptureGrantsBlocked: true, cases, failures };
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  return report;
}
const deadline = setTimeout(() => {
  failures.push('fixture deadline exceeded'); writeReport();
  console.error('Capture admission fixture timed out'); app.exit(1);
}, 90000);

app.whenReady().then(async () => {
  const profile = session.fromPartition('capture-admission-smoke');
  profile.protocol.handle('https', () => new Response('<!doctype html><title>Synthetic capture admission fixture</title>',
    { headers: { 'Content-Type': 'text/html' } }));

  // Observe the production policy without letting a failed negative test capture
  // the desktop. Record an unexpected grant as a failure before refusing it.
  const setRequest = profile.setPermissionRequestHandler.bind(profile);
  profile.setPermissionRequestHandler = handler => setRequest((requester, permission, callback, details) => {
    handler(requester, permission, allowed => {
      if (activeCase) activeCase.permissions.push({ permission, mediaTypes: details?.mediaTypes, allowed });
      const permittedFixtureRequest = activeCase?.expectMicrophone === true && permission === 'media';
      callback(allowed && permittedFixtureRequest);
    }, details);
  });
  async function exercise(name, expression, expectMicrophone = false) {
    // Invalid legacy tab requests can cause Chromium to terminate their renderer.
    // Give each case its own renderer and record that outcome explicitly.
    owner = new BrowserWindow({ show: false, width: 640, height: 360,
      webPreferences: { partition: 'capture-admission-smoke', sandbox: true, contextIsolation: true, nodeIntegration: false } });
    contents = owner.webContents;
    await contents.loadURL('https://palisade.onyxops.org/capture-admission-fixture');
    activeCase = { name, controllerAvailable, expectMicrophone, permissions: [] };
    cases.push(activeCase);
    if (typeof expression === 'function') expression = expression(owner, contents);
    let timer, gone;
    const terminated = new Promise(resolve => {
      gone = (_event, details) => resolve({ outcome: 'rendererRejected', reason: details.reason });
      contents.once('render-process-gone', gone);
    });
    const request = contents.executeJavaScript(`(async () => {
      try {
        const stream = await (${expression});
        const kinds = stream.getTracks().map(track => track.kind);
        stream.getTracks().forEach(track => track.stop());
        return { outcome: 'granted', kinds };
      } catch (error) { return { outcome: 'denied', error: error.name }; }
    })()`, true).catch(() => terminated);
    const result = await Promise.race([request, terminated, new Promise(resolve => {
      timer = setTimeout(() => resolve({ outcome: 'timeout' }), 6000);
    })]);
    clearTimeout(timer); contents.removeListener('render-process-gone', gone);
    activeCase.result = result;
    if (expectMicrophone) {
      if (result.outcome !== 'granted' || result.kinds.length !== 1 || result.kinds[0] !== 'audio') failures.push(name + ': synthetic microphone failed');
    } else {
      if (activeCase.permissions.some(event => event.allowed)) failures.push(name + ': production policy admitted capture');
      if (!['denied', 'rendererRejected'].includes(result.outcome)) failures.push(name + ': request was not denied');
    }
    owner.destroy(); owner = null; contents = null;
    activeCase = null;
  }

  for (controllerAvailable of [false, true]) {
    installSessionPolicy(profile, () => contents, controllerAvailable ? {
      available: () => true,
      handle: () => { throw new Error('Availability must not authorize display capture.'); },
    } : null);
    await exercise('microphone', 'navigator.mediaDevices.getUserMedia({audio:true})', true);
    await exercise('camera', 'navigator.mediaDevices.getUserMedia({video:true})');
    await exercise('microphoneAndCamera', 'navigator.mediaDevices.getUserMedia({audio:true,video:true})');
    await exercise('displayVideo', 'navigator.mediaDevices.getDisplayMedia({video:true,audio:false})');
    await exercise('displayWithAudio', 'navigator.mediaDevices.getDisplayMedia({video:true,audio:true})');
    for (const source of ['desktop', 'tab']) {
      for (const audio of [false, true]) {
        await exercise('legacy' + source + (audio ? 'WithMicrophone' : 'Video'),
          (window, page) => 'navigator.mediaDevices.getUserMedia(' + JSON.stringify({ audio,
            video: { mandatory: { chromeMediaSource: source,
              chromeMediaSourceId: source === 'desktop' ? window.getMediaSourceId() : page.getMediaSourceId(page) } } }) + ')');
      }
    }
    for (const source of ['desktop', 'screen', 'tab']) {
      await exercise('legacy' + source + 'MissingIdWithMicrophone',
        'navigator.mediaDevices.getUserMedia(' + JSON.stringify({ audio: true, video: { mandatory: { chromeMediaSource: source } } }) + ')');
      await exercise('legacy' + source + 'InvalidIdWithMicrophone',
        'navigator.mediaDevices.getUserMedia(' + JSON.stringify({ audio: true, video: { mandatory: { chromeMediaSource: source, chromeMediaSourceId: 'invalid-source' } } }) + ')');
    }
  }
  const report = writeReport();
  console.log(JSON.stringify({ passed: report.passed, electron: report.electron, cases: cases.length, failures }));
  assert.equal(failures.length, 0, failures.join('; '));
  clearTimeout(deadline); app.exit(0);
}).catch(error => {
  failures.push(error.message); writeReport(); console.error(error.stack || error.message);
  owner?.destroy(); clearTimeout(deadline); app.exit(1);
});
