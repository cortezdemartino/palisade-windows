'use strict';
// App-owned WebRTC peers and a fake native sink only. No OS input, clipboard,
// account enrollment, service grant, production key or personal source access.
const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('node:path'), fs = require('node:fs'), assert = require('node:assert/strict');
const root = process.env.PALISADE_WINDOW_SMOKE_ASAR || path.resolve(__dirname, '..');
const { createOwnedInputPeer } = require(path.join(root, 'src/assist/remote-input-peer.cjs'));
const { installControlsProtocol } = require(path.join(root, 'src/desktop-window.cjs'));
app.disableHardwareAcceleration();
app.on('window-all-closed', () => {});
app.setPath('userData', path.resolve('out/assist-remote-input-profile'));
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const poll = async (predicate, timeout = 8000) => {
  const start = performance.now();
  while (!(await predicate())) { if (performance.now() - start >= timeout) throw Error('Peer fixture deadline'); await pause(25); }
};
const packet = sequence => JSON.stringify({ p: 'palisade.assist.control', v: 2, session: 'session_fixture_0001',
  grant: 'grant_fixture_0001', gen: 1, seq: sequence, kind: 'release-all' });
const script = value => JSON.stringify(value);
let phase = 'startup', number = 0;
const cases = [], blocked = [];
const deadline = setTimeout(() => { console.error('Remote peer fixture timeout:', phase); app.exit(1); }, 110000);
async function fixture({ label = 'control', nativeWait = false, cleanupWait = false, stallPacket = false, fingerprintOverride, corruptOffer = false } = {}) {
  const helper = new BrowserWindow({ show: false, webPreferences: { partition: 'assist-remote-helper-' + (++number),
    sandbox: true, contextIsolation: true, nodeIntegration: false, webSecurity: true, devTools: false,
    backgroundThrottling: false } });
  const profile = helper.webContents.session;
  installControlsProtocol(profile);
  profile.setPermissionRequestHandler((_contents, _permission, done) => done(false));
  profile.setPermissionCheckHandler(() => false);
  profile.webRequest.onBeforeRequest({ urls: ['*://*/*'] }, (details, done) => {
    const allowed = details.url === 'palisade-controls://app/assist-input.html';
    if (!allowed) blocked.push('request');
    done({ cancel: !allowed });
  });
  await helper.loadURL('palisade-controls://app/assist-input.html');
  const run = source => helper.webContents.executeJavaScript(source);
  const own = await run(`(async()=>{
    const certificate=await RTCPeerConnection.generateCertificate({name:'ECDSA',namedCurve:'P-256'});
    const pc=new RTCPeerConnection({iceServers:[],certificates:[certificate]});
    const channel=pc.createDataChannel(${script(label)},{ordered:true});
    const f=window.fixture={pc,channel,candidates:[],pending:[],open:false};
    channel.onopen=()=>f.open=true;
    pc.onicecandidate=e=>f.candidates.push({candidate:e.candidate?.candidate??'',mid:e.candidate?.sdpMid??null});
    f.add=async c=>{if(!pc.remoteDescription){f.pending.push(c);return;}await pc.addIceCandidate(c.candidate===''?null:{candidate:c.candidate,sdpMid:c.mid});};
    f.answer=async sdp=>{await pc.setRemoteDescription({type:'answer',sdp});for(const c of f.pending)await f.add(c);f.pending=[];};
    await pc.setLocalDescription(await pc.createOffer());
    return {sdp:pc.localDescription.sdp,fingerprint:'sha-256 '+certificate.getFingerprints().find(f=>f.algorithm==='sha-256').value.toUpperCase()};
  })()`);
  const received = [], observations = [];
  const cancellation = new AbortController();
  let closed = false, reason = null, nativeResolve, cleanupResolve, packetResolve, packetSignal;
  const nativeDecision = new Promise(resolve => { nativeResolve = resolve; });
  const cleanup = new Promise(resolve => { cleanupResolve = resolve; });
  const packetWork = new Promise(resolve => { packetResolve = resolve; });
  const peer = await createOwnedInputPeer({ enabled: true, session: 'session_fixture_0001',
    signal: cancellation.signal,
    helperTransport: fingerprintOverride ?? own.fingerprint,
    onObserved: async (fingerprint, signal) => { observations.push(fingerprint); if (nativeWait) await nativeDecision; return !signal.aborted; },
    onPacket: async (text, signal) => { packetSignal = signal; received.push(text); if (stallPacket) await packetWork; assert.equal(typeof signal.aborted, 'boolean'); },
    onCandidate: candidate => run(`fixture.add(${script(candidate)})`),
    onClosed: why => { closed = true; reason = why; if (cleanupWait) return cleanup; } });
  const ownedWindow = BrowserWindow.getAllWindows().find(value => value !== helper && value.getTitle() === 'Palisade Assist input transport');
  assert.ok(ownedWindow);
  const isolation = await ownedWindow.webContents.executeJavaScript(`({node:typeof require,ipc:typeof ipcRenderer,bridge:typeof palisadeAssistNative,privatePeer:typeof peer})`);
  assert.deepEqual(isolation, { node: 'undefined', ipc: 'undefined', bridge: 'undefined', privatePeer: 'undefined' });
  let pumping = false;
  const pump = setInterval(async () => {
    if (closed || pumping || helper.isDestroyed()) return;
    pumping = true;
    try { const candidates = await run('fixture.candidates.splice(0)'); for (const c of candidates) { if (closed) break; await peer.candidate(c.candidate, c.mid); } }
    catch { /* Expected only after deliberate close/refusal. Assertions below prove outcome. */ }
    finally { pumping = false; }
  }, 25);
  const dispose = async () => {
    clearInterval(pump); nativeResolve(); cleanupResolve(); packetResolve();
    await peer.close(); if (!helper.isDestroyed()) helper.destroy();
  };
  try {
    const offer = corruptOffer ? own.sdp.replace(own.fingerprint, fingerprintOverride) : own.sdp;
    const answer = await peer.offer(offer);
    await run(`fixture.answer(${script(answer.sdp)})`);
    return { peer, helper, ownedWindow, run, received, observations, dispose, nativeResolve, cleanupResolve, packetResolve,
      cancellation, packetSignal: () => packetSignal, closed: () => closed, reason: () => reason };
  } catch (error) { await dispose(); throw error; }
}
app.whenReady().then(async () => {
  phase = 'disabled';
  const before = BrowserWindow.getAllWindows().length;
  await assert.rejects(() => createOwnedInputPeer());
  const cancelled = new AbortController(); cancelled.abort();
  await assert.rejects(() => createOwnedInputPeer({ enabled: true, signal: cancelled.signal }));
  for (const session of [undefined, 12345678, { toString: () => 'session_fixture_0001' }])
    await assert.rejects(() => createOwnedInputPeer({ enabled: true, session,
      helperTransport: 'sha-256 ' + Array(32).fill('AB').join(':'),
      onObserved() {}, onPacket() {}, onCandidate() {}, onClosed() {} }));
  assert.equal(BrowserWindow.getAllWindows().length, before);
  cases.push(phase);

  phase = 'observed-certificate-native-admission-and-cleanup';
  let f = await fixture({ nativeWait: true, cleanupWait: true });
  try {
    await poll(() => f.observations.length === 1);
    assert.equal(f.peer.verified(), false);
    await f.run(`fixture.channel.send(${script(packet(1))})`);
    await pause(150); assert.equal(f.received.length, 0);
    f.nativeResolve(); await poll(() => f.peer.verified()); await pause(50);
    await f.run(`fixture.channel.send(${script(packet(2))})`);
    await poll(() => f.received.length === 1);
    assert.equal(f.received[0], packet(2));
    // Another contents cannot impersonate the private gateway IPC sender.
    ipcMain.emit('palisade:owned-input-event', { sender: f.helper.webContents, senderFrame: f.helper.webContents.mainFrame }, { type: 'packet', text: packet(3) });
    await pause(50); assert.equal(f.received.length, 1);
    let finished = false;
    const closing = f.peer.close().then(() => { finished = true; });
    assert.equal(f.closed(), true); assert.equal(f.peer.verified(), false);
    await pause(50); assert.equal(finished, false);
    f.cleanupResolve(); await closing;
    await assert.rejects(() => f.peer.candidate('', null));
    assert.equal(f.received.length, 1); cases.push(phase);
  } finally { await f.dispose(); }

  phase = 'bounded-before-native-admission';
  f = await fixture({ nativeWait: true });
  try {
    await poll(() => f.observations.length === 1);
    await f.run('fixture.channel.send(new Uint8Array([1,2,3]))');
    await poll(f.closed); assert.equal(f.received.length, 0); cases.push(phase);
  } finally { await f.dispose(); }

  phase = 'cancel-pending-native-admission';
  f = await fixture({ nativeWait: true });
  try {
    await poll(() => f.observations.length === 1);
    f.cancellation.abort(); assert.equal(f.closed(), true);
    f.nativeResolve(); await pause(50);
    assert.equal(f.peer.verified(), false); assert.equal(f.received.length, 0); cases.push(phase);
  } finally { await f.dispose(); }

  phase = 'cancel-stalled-dispatch';
  f = await fixture({ stallPacket: true });
  try {
    await poll(() => f.peer.verified()); await pause(50);
    await f.run(`fixture.channel.send(${script(packet(5))})`);
    await poll(() => f.received.length === 1);
    f.cancellation.abort(); assert.equal(f.closed(), true); assert.equal(f.packetSignal().aborted, true);
    f.packetResolve(); await f.peer.finished;
    assert.equal(f.received.length, 1); cases.push(phase);
  } finally { await f.dispose(); }

  phase = 'wrong-approved-fingerprint';
  await assert.rejects(() => fixture({ fingerprintOverride: 'sha-256 ' + Array(32).fill('AB').join(':') }));
  cases.push(phase);

  phase = 'actual-certificate-mismatch';
  f = await fixture({ fingerprintOverride: 'sha-256 ' + Array(32).fill('CD').join(':'), corruptOffer: true });
  try { await poll(f.closed, 18000); assert.equal(f.observations.length, 0); assert.equal(f.received.length, 0); cases.push(phase); }
  finally { await f.dispose(); }

  phase = 'wrong-channel';
  f = await fixture({ label: 'files' });
  try { await poll(f.closed); assert.equal(f.received.length, 0); cases.push(phase); }
  finally { await f.dispose(); }

  for (const kind of ['extra-channel', 'binary', 'oversized', 'stalled-native-queue', 'renderer-crash', 'renderer-freeze']) {
    phase = kind; f = await fixture({ stallPacket: kind === 'stalled-native-queue' });
    try {
      await poll(() => f.peer.verified()); await pause(50);
      if (kind === 'extra-channel') await f.run(`fixture.pc.createDataChannel('files')`);
      if (kind === 'binary') await f.run('fixture.channel.send(new Uint8Array([1,2,3]))');
      if (kind === 'oversized') await f.run(`fixture.channel.send('x'.repeat(2049))`);
      if (kind === 'stalled-native-queue') await f.run(`for(let i=0;i<12;i++)fixture.channel.send(${script(packet(4))})`);
      if (kind === 'renderer-crash') f.ownedWindow.webContents.forcefullyCrashRenderer();
      if (kind === 'renderer-freeze') void f.ownedWindow.webContents.executeJavaScript('while(true){}').catch(() => {});
      await poll(f.closed, 5000);
      assert.equal(f.peer.verified(), false);
      assert.ok(f.received.length <= (kind === 'stalled-native-queue' ? 1 : 0));
      f.packetResolve(); await f.peer.finished; cases.push(phase);
    } finally { await f.dispose(); }
  }
  assert.deepEqual(blocked, []);
  fs.writeFileSync(path.resolve('out/assist-remote-input-result.json'), JSON.stringify({ passed: true, electron: process.versions.electron,
    cases, fakeNativeSink: true, actualDtls: true, inputInjected: false }));
  clearTimeout(deadline); console.log('Owned remote input transport fixtures passed:', cases.length); app.exit(0);
}).catch(error => {
  console.error('Remote peer fixture failed:', phase, error.message);
  for (const window of BrowserWindow.getAllWindows()) window.destroy();
  clearTimeout(deadline); app.exit(1);
});
