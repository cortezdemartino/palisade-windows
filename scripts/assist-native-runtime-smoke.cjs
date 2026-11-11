'use strict';
// Real Electron isolation and actual compiled monitor/coordinator. Probe mode
// prohibits issuer HTTP, DPAPI identity access, dialogs and OS input.
const { app, BrowserWindow } = require('electron');
const assert = require('node:assert/strict'), path = require('node:path'), fs = require('node:fs');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.join(__dirname, '..');
const { installNativeBridge } = require(path.join(root, 'src/assist/native-bridge.cjs'));
const { createNativeRuntime } = require(path.join(root, 'src/assist/native-runtime.cjs'));
const { createAssistLifecycle } = require(path.join(root, 'src/assist/lifecycle.cjs'));
const { PALISADE_ORIGIN } = require(path.join(root, 'src/security-policy.cjs'));
app.disableHardwareAcceleration();
app.setPath('userData', path.join(__dirname, '../out/assist-native-runtime-profile'));
const lifecycle = createAssistLifecycle();
let win, bridge, runtime;
const deadline = setTimeout(() => { console.error('Native runtime fixture deadline'); app.exit(1); }, 30000);
app.whenReady().then(async () => {
  runtime = createNativeRuntime({ lifecycle, probe: true,
    binary: process.env.PALISADE_ASSIST_TEST_BINARY || (process.env.PALISADE_WINDOW_SMOKE_ASAR
      ? path.join(root, '../assist/Palisade-Assist.exe') : path.join(root, '.assist/Palisade-Assist.exe')),
    manifestFile: path.join(root, 'src/assist/guardian-integrity.json') });
  await runtime.ready;
  win = new BrowserWindow({ show: false, webPreferences: { partition: 'assist-native-runtime-smoke',
    sandbox: true, contextIsolation: true, nodeIntegration: false,
    preload: path.join(root, 'src/chat-notifications-preload.cjs'), additionalArguments: ['--palisade-assist-native', '--palisade-assist-native-origin=' + PALISADE_ORIGIN] } });
  const contents = win.webContents;
  contents.session.protocol.handle('https', () => new Response('<!doctype html><title>Owned runtime fixture</title>'));
  contents.session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  contents.session.setPermissionCheckHandler(() => false);
  bridge = installNativeBridge(contents, { enabled: true, dispatch: runtime.dispatch, stopAll: runtime.stopAll });
  await contents.loadURL(PALISADE_ORIGIN + '/owned-native-runtime-fixture');
  const request = (op, fields = {}) => contents.executeJavaScript(`palisadeAssistNative.request(JSON.stringify({p:'palisade.assist.native',v:1,id:crypto.randomUUID(),op:${JSON.stringify(op)},...${JSON.stringify(fields)}})).then(JSON.parse)`);
  const hello = await request('hello', { versions: [1] });
  assert.equal(hello.ok, true);
  assert.deepEqual(hello.result.features, { capture: false, clipboard: false, files: false, input: false });
  const state = await request('status');
  assert.equal(state.ok, true); assert.equal(state.result.endpoint.enrolled, false);
  assert.equal((await request('enroll.key', { origin: PALISADE_ORIGIN })).error, 'not-enrolled');
  assert.equal(await contents.executeJavaScript('typeof require+":"+typeof process'), 'undefined:undefined');
  // A real renderer crash revokes the native channel; its holder remains until
  // the authenticated endpoint terminal and exact process closure are observed.
  const gone = new Promise(resolve => contents.once('render-process-gone', resolve));
  contents.forcefullyCrashRenderer(); await gone;
  await runtime.stopAll();
  assert.equal(lifecycle.status().holders, 0);
  fs.writeFileSync(path.join(__dirname, '../out/assist-native-runtime-result.json'), JSON.stringify({
    passed: true, electron: process.versions.electron, actualNativeProcess: true,
    isolatedSurface: true, rendererCrashCleanup: true, enrollmentExecuted: false, nativeInput: false,
  }));
  bridge.dispose(); win.destroy(); clearTimeout(deadline); app.exit(0);
}).catch(async error => {
  console.error(error.stack || error); bridge?.dispose();
  try { await runtime?.stopAll(); } catch {}
  win?.destroy(); clearTimeout(deadline); app.exit(1);
});
