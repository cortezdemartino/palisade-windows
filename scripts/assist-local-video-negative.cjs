'use strict';
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('node:path'),
  assert = require('node:assert/strict'),
  fs = require('node:fs');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.resolve('.');
const { openGuardian } = require(path.join(root, 'src/assist/guardian.cjs'));
const { createLocalVideoHop, PROVIDER } = require(
  path.join(root, 'src/assist/local-video-hop.cjs'),
);
const { installSessionPolicy } = require(path.join(root, 'src/session-policy.cjs'));
app.disableHardwareAcceleration();
app.setPath('userData', path.resolve('out/assist-local-video-negative-profile'));
let guardian, current, keeper;
const passed = [];
const failures = [];
ipcMain.on('palisade:local-video-event', (_event, message) => { if (message?.type === 'failed' && failures.length < 32) failures.push(message.stage || 'unspecified'); });
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const visibleFixtures = () => BrowserWindow.getAllWindows().filter((window) => window !== keeper);
const deadline = setTimeout(() => {
  current?.hop.dispose();
  current?.window.destroy();
  guardian?.close();
  console.error('Local video negative fixture deadline', passed);
  app.exit(1);
}, 90000);
async function eventually(check) {
  for (let i = 0; i < 100; i++) {
    if (await check()) return;
    await pause(25);
  }
  throw Error('Lifecycle transition timed out');
}
async function setup(options = {}) {
  const window = new BrowserWindow({
    show: false,
    webPreferences: {
      partition: 'local-video-negative-' + Math.random(),
      preload: path.join(root, 'src/assist/local-video-preload.cjs'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      webSecurity: true,
      backgroundThrottling: false,
    },
  });
  const contents = window.webContents;
  contents.session.protocol.handle(
    'https',
    () =>
      new Response('<!doctype html><body></body>', {
        headers: { 'Content-Type': 'text/html', 'Content-Security-Policy': "default-src 'none'" },
      }),
  );
  installSessionPolicy(contents.session, () => contents);
  const hop = createLocalVideoHop({
    consumer: contents,
    parent: window,
    inspect: (ports) => guardian.inspectLocalIce(ports),
    enabled: true,
    onState: (state, _operation, reason) => { if (state === 'unavailable' && failures.length < 32) failures.push(reason); },
    ...options,
  });
  current = { window, contents, hop };
  await contents.loadURL('https://palisade.onyxops.org/local-video-negative');
  return current;
}
function close() {
  if (current) {
    current.hop.dispose();
    if (!current.window.isDestroyed()) current.window.destroy();
    current = null;
  }
}
app
  .whenReady()
  .then(async () => {
    keeper = new BrowserWindow({
      show: false,
      webPreferences: {
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        javascript: false,
      },
    });
    guardian = await openGuardian({
      binary: process.env.PALISADE_WINDOW_SMOKE_ASAR
        ? path.join(path.dirname(root), 'assist/Palisade-Assist.exe')
        : path.resolve('.assist/Palisade-Assist.exe'),
      manifestFile: path.join(root, 'src/assist/guardian-integrity.json'),
      probe: true,
    });
    {
      const t = await setup({ enabled: false });
      await assert.rejects(t.hop.start({ kind: 'synthetic' }));
      assert.equal(visibleFixtures().length, 1);
      passed.push('disabled');
      close();
    }
    {
      const t = await setup({ inspect: async () => ({ error: 'ice-port-unowned' }) });
      await assert.rejects(t.hop.start({ kind: 'synthetic' }));
      assert.equal(
        await t.contents.executeJavaScript('document.querySelectorAll("video").length'),
        0,
      );
      passed.push('unowned-sockets');
      close();
    }
    {
      const t = await setup();
      const pending = t.hop.start({ kind: 'synthetic' });
      void pending.catch(() => {});
      t.hop.stop();
      await assert.rejects(pending);
      await pause(150);
      assert.equal(visibleFixtures().length, 1);
      assert.equal(
        await t.contents.executeJavaScript('document.querySelectorAll("video").length'),
        0,
      );
      passed.push('cancel-before-ready-or-gather');
      close();
    }
    {
      const corrupt = (_event, message) => {
        if (message.type === 'offer')
          message.fingerprint = 'sha-256 ' + Array(32).fill('00').join(':');
      };
      ipcMain.prependListener('palisade:local-video-event', corrupt);
      try {
        const t = await setup();
        await assert.rejects(t.hop.start({ kind: 'synthetic' }));
        assert.equal(
          await t.contents.executeJavaScript('document.querySelectorAll("video").length'),
          0,
        );
        passed.push('wrong-certificate');
      } finally {
        ipcMain.removeListener('palisade:local-video-event', corrupt);
        close();
      }
    }
    for (const kind of ['wrong-observed-certificate','unowned-selected-port']) {
      const corrupt = (_event,message) => { if (message.type === 'connected') {
        if (kind === 'wrong-observed-certificate') message.peerFingerprint='sha-256 '+Array(32).fill('00').join(':');
        else message.pair.remotePort=message.pair.remotePort===65535?1:message.pair.remotePort+1;
      } };
      ipcMain.prependListener('palisade:local-video-event',corrupt);
      try { const t=await setup();await assert.rejects(t.hop.start({kind:'synthetic'}));
        assert.equal(await t.contents.executeJavaScript('document.querySelectorAll("video").length'),0);passed.push(kind);
      } finally { ipcMain.removeListener('palisade:local-video-event',corrupt);close(); }
    }
    {
      const t = await setup();
      await t.contents.executeJavaScript(
        `window.peerLeak=false;window.originalSrc=Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype,'srcObject');Object.defineProperty(HTMLMediaElement.prototype,'srcObject',{configurable:true,get(){return null},set(){window.peerLeak=true}});window.RTCPeerConnection=function(){throw Error('page peer must not run')};void 0;`,
      );
      const value = await t.hop.start({ kind: 'synthetic' });
      assert.equal(await t.contents.executeJavaScript(`window.peerLeak`), false);
      await t.contents.executeJavaScript(
        `Object.defineProperty(HTMLMediaElement.prototype,'srcObject',window.originalSrc);const node=document.getElementById(${JSON.stringify(value.elementId)});window.fixtureClone=node.srcObject.clone();node.id='renamed-by-page';node.remove();const replacement=document.createElement('video');replacement.id=${JSON.stringify(value.elementId)};document.body.append(replacement);`,
      );
      t.hop.stop();
      await eventually(() =>
        t.contents.executeJavaScript(
          'window.fixtureClone.getTracks().every(track=>track.readyState==="ended")',
        ),
      );
      assert.equal(
        await t.contents.executeJavaScript(
          `document.getElementById(${JSON.stringify(value.elementId)})!==null`,
        ),
        true,
      );
      const again = await t.hop.start({ kind: 'synthetic' });
      assert.notEqual(again.operation, value.operation);
      t.hop.stop();
      passed.push('hostile-dom-and-repeated-capture');
      close();
    }
    {
      const t = await setup();
      await t.hop.start({ kind: 'synthetic' });
      await t.contents.reload();
      await eventually(() => !t.hop.busy());
      assert.equal(visibleFixtures().length, 1);
      passed.push('consumer-reload');
      close();
    }
    {
      const t = await setup();
      const value = await t.hop.start({ kind: 'synthetic' });
      const provider = BrowserWindow.getAllWindows().find(
        (window) => window.webContents.getURL() === PROVIDER,
      );
      assert(provider);
      provider.webContents.forcefullyCrashRenderer();
      await eventually(() => !t.hop.busy());
      await eventually(() =>
        t.contents.executeJavaScript(
          `document.getElementById(${JSON.stringify(value.elementId)})===null`,
        ),
      );
      passed.push('provider-crash');
      close();
    }
    {
      const t = await setup();
      await t.hop.start({ kind: 'synthetic' });
      void t.contents.executeJavaScript('for(;;){}').catch(() => {});
      await eventually(() => !t.hop.busy());
      assert.equal(visibleFixtures().filter((window) => window !== t.window).length, 0);
      t.contents.forcefullyCrashRenderer();
      passed.push('frozen-consumer-independent-stop');
      close();
    }
    {
      const t = await setup();
      await t.hop.start({ kind: 'synthetic' });
      const provider = BrowserWindow.getAllWindows().find(
        (window) => window.webContents.getURL() === PROVIDER,
      );
      assert(provider);
      void provider.webContents.executeJavaScript('for(;;){}').catch(() => {});
      await eventually(() => !t.hop.busy());
      assert(provider.isDestroyed());
      passed.push('frozen-provider-independent-destruction');
      close();
    }
    {
      let slow = false;
      const t = await setup({
        inspect: async (ports) => {
          if (slow) await pause(1200);
          return guardian.inspectLocalIce(ports);
        },
      });
      await t.hop.start({ kind: 'synthetic' });
      slow = true;
      const originalNow = Date.now;
      Date.now = () => originalNow() - 60 * 60 * 1000;
      try {
        const began = performance.now();
        void t.contents.executeJavaScript('for(;;){}').catch(() => {});
        await eventually(() => !t.hop.busy());
        assert(
          performance.now() - began < 2300,
          'queued reports extended the original liveness deadline',
        );
        assert.equal(visibleFixtures().filter((window) => window !== t.window).length, 0);
        t.contents.forcefullyCrashRenderer();
        passed.push('slow-proof-clock-rollback-frozen-peer');
      } finally {
        Date.now = originalNow;
        close();
      }
    }
    fs.writeFileSync(
      path.resolve('out/assist-local-video-negative-result.json'),
      JSON.stringify({ passed: true, cases: passed, electron: process.versions.electron }),
    );
    console.log('Local video negative/lifecycle fixtures passed:', passed.length);
    guardian.close();
    clearTimeout(deadline);
    app.exit(0);
  })
  .catch((error) => {
    console.error('Local video negatives:', error.message, passed, failures);
    close();
    guardian?.close();
    clearTimeout(deadline);
    app.exit(1);
  });
