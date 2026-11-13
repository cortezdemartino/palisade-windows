'use strict';
// Actual Windows processes/pipes, non-authorizing modes only. No input, identity,
// clipboard, dialogs, network service or native resource activation occurs.
const test = require('node:test'), assert = require('node:assert/strict');
const path = require('node:path'), { spawn } = require('node:child_process');
const { once } = require('node:events'), { randomBytes } = require('node:crypto');
const { openLiveChannel, PROTOCOL } = require('../src/assist/live-channel.cjs');
const { createAssistLifecycle } = require('../src/assist/lifecycle.cjs');
const { prepareProcessWatcher } = require('./native-process-watch.cjs');
const binary = process.env.PALISADE_ASSIST_TEST_BINARY || path.resolve('.assist/Palisade-Assist.exe');
const manifestFile = process.env.PALISADE_ASSIST_TEST_MANIFEST || path.resolve('src/assist/guardian-integrity.json');
const hello = { type: 'hello', protocol: PROTOCOL, version: 1, challenge: randomBytes(32).toString('base64url') };
const canonical = (op, fields = {}) => ({ p: 'palisade.assist.native', v: 1, id: randomBytes(12).toString('base64url'), op, ...fields });

test('actual endpoint owns normal executor transport, terminal receipt and mirror across probe generations', async () => {
  const lifecycle = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle, probe: true, ownedTree: true, endpoint: true });
  try {
    let prior = 0;
    for (let generation = 0; generation < 2; generation++) {
      const held = await channel.probeExecutor(false);
      assert.equal(held.released, false);
      assert.notEqual(held.pid, held.coordinator);
      assert.notEqual(held.pid, prior);
      assert.equal((await channel.status()).authority, 'none');
      const released = await channel.probeExecutor(true);
      assert.deepEqual(released, { ...held, released: true });
      prior = held.pid;
    }
  } finally { await channel.close(); }
  assert.equal(lifecycle.status().holders, 0);
});

test('actual endpoint urgent shutdown settles a held fake-input executor before launch completion', async () => {
  const lifecycle = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle, probe: true, ownedTree: true, endpoint: true });
  await channel.probeExecutor(false);
  await channel.close();
  assert.equal(lifecycle.status().holders, 0);
});

test('actual endpoint Stop between creation and ARM settles its retained startup before a successor', async () => {
  const lifecycle = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle, probe: true, ownedTree: true, endpoint: true });
  try {
    const cancelled = await channel.probeExecutorCancel();
    assert.notEqual(cancelled.pid, cancelled.coordinator);
    const fresh = await channel.probeExecutor(false);
    assert.notEqual(fresh.pid, cancelled.pid);
    await channel.probeExecutor(true);
  } finally { await channel.close(); }
  assert.equal(lifecycle.status().holders, 0);
});

test('actual endpoint coordinator loss retains failed app ownership while its probe executor cleans up', async () => {
  const observer = await prepareProcessWatcher(binary);
  const lifecycle = createAssistLifecycle();
  let channel;
  try {
    channel = await openLiveChannel({ binary, manifestFile, lifecycle, probe: true, ownedTree: true, endpoint: true });
    const held = await channel.probeExecutor(false);
    await observer.retain([{ pid: channel.pid, role: 'monitor' }, { pid: held.coordinator, role: 'coordinator' }, { pid: held.pid, role: 'holder' }]);
    // Kill only the disposable no-authority fixture whose original handle and
    // image were retained above. No physical input or personal app is involved.
    process.kill(held.coordinator);
    await assert.rejects(channel.wait(), { code: 'assist-cleanup-incomplete' });
    const result = await observer.finish();
    assert(result.every(item => item.exited));
    assert.equal(result[2].code, 125);
    await assert.rejects(lifecycle.quiesce());
    assert.equal(lifecycle.status().holders, 1);
    // Native mode still requires its missing coordinator mirror. This Probe
    // result cannot satisfy Native cleanup or physical-input acceptance.
  } finally { observer.close(); if (channel) await channel.close().catch(() => {}); }
});

test('actual persistent endpoint negotiates canonical v1 through monitor/coordinator without native authority', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true, endpoint: true });
  try {
    assert.deepEqual(await channel.request(canonical('hello', { versions: [Number.MAX_SAFE_INTEGER, 1] }), 3),
      { version: 1, features: { capture: false, clipboard: false, files: false, input: false } });
    assert.deepEqual(await channel.request(canonical('hello', { versions: [1] }), 3),
      { version: 1, features: { capture: false, clipboard: false, files: false, input: false } });
    const status = await channel.request(canonical('status'), 3);
    assert.deepEqual(status, { endpoint: { enrolled: false, endpointId: null, pin: 'none' },
      capture: { operation: null, state: 'idle', trackSid: null }, controller: { session: null, state: 'idle' } });
    await assert.rejects(channel.request(canonical('enroll.key', { origin: 'https://attacker.example' }), 3), { code: 'binding-mismatch' });
    await assert.rejects(channel.request(canonical('enroll.key', { origin: 'https://palisade.onyxops.org' }), 3), { code: 'not-enrolled' });
    await assert.rejects(channel.request(canonical('grant.offer', { session: 'session_test', grant: '{}' }), 3), { code: 'not-enrolled' });
    await assert.rejects(channel.request(canonical('capture.start', { operation: 'capture_test', room: '!room:example.test' }), 3), { code: 'capture-unavailable' });
    await assert.rejects(channel.request(canonical('peer.offer', { session: 'session_test', grantId: 'grant_test', sdp: 'not-sdp' }), 3), { code: 'transport-mismatch' });
    assert.deepEqual(await channel.request(canonical('status'), 3), status);
  } finally { await channel.close(); }
  assert.equal(life.status().holders, 0);
});

test('actual endpoint fences replaced documents and does not reconstruct enrollment', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true, endpoint: true });
  try {
    await channel.request(canonical('hello', { versions: [1] }), 0);
    await assert.rejects(channel.request(canonical('status'), 1), { code: 'stopped' });
  } finally { await channel.close(); }
  assert.equal(life.status().holders, 0);
});

test('actual monitor refuses a concurrent endpoint for the same application process', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true, endpoint: true });
  try {
    await assert.rejects(openLiveChannel({ binary, manifestFile, lifecycle: createAssistLifecycle(), probe: true, ownedTree: true, endpoint: true }), { code: 'assist-native-unavailable' });
    assert.equal((await channel.status()).authority, 'none');
  } finally { await channel.close(); }
});

test('application runtime reserves one actual endpoint and retains it through terminal document loss', async () => {
  const { createNativeRuntime } = require('../src/assist/native-runtime.cjs');
  const lifecycle = createAssistLifecycle(), statuses = [];
  const runtime = createNativeRuntime({ binary, manifestFile, lifecycle, probe: true, onStatus: state => statuses.push(state) });
  assert.throws(() => createNativeRuntime({ binary, manifestFile, lifecycle, probe: true }), /already reserved/);
  await runtime.ready;
  const scope = { generation: 0, signal: new AbortController().signal };
  await runtime.dispatch(canonical('hello', { versions: [1] }), scope);
  await assert.rejects(runtime.dispatch(canonical('status'), { generation: 1, signal: scope.signal }), { code: 'stale-operation' });
  await runtime.stopAll();
  await assert.rejects(runtime.dispatch(canonical('status'), scope), { code: 'stopped' });
  assert.equal(lifecycle.status().holders, 0);
  assert(statuses.includes('Cleanup pending'));
  assert(statuses.includes('Session ended'));
});
function frame(command, sequence = 0) {
  const bytes = Buffer.from(typeof command === 'string' ? command : JSON.stringify({ sequence, command }));
  const prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length); return Buffer.concat([prefix, bytes]);
}
function raw({ mode = '--live-probe', pid = process.pid, drain = true, extra = [] } = {}) {
  const child = spawn(binary, [mode, String(pid), ...extra], { windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'], shell: false });
  child.stdin.on('error', () => {});
  if (drain) child.stdout.resume();
  const exit = once(child, 'exit');
  // This harness may reap ONLY non-authorizing probes on a failed test deadline.
  const timer = setTimeout(() => child.kill(), 8500);
  return { child, exit, close() { clearTimeout(timer); child.kill(); } };
}
async function refused(wire, options = {}) {
  const p = raw(options);
  try {
    if (wire) p.child.stdin.write(wire);
    if (options.end) p.child.stdin.end();
    const [code, signal] = await p.exit; assert.equal(signal, null); assert.equal(code, 125);
  } finally { p.close(); }
}

test('actual live channel closes only after typed cleanup proof and process exit, with no native authority', async () => {
  // Repeat adjacent handshake/shutdown to exercise reader metadata publication.
  for (let i = 0; i < 12; i++) {
    const life = createAssistLifecycle(), channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true });
    assert.deepEqual(await channel.status(), { type: 'status', state: 'inactive', authority: 'none' });
    assert.equal(life.status().holders, 1);
    await channel.close(); assert.equal(life.status().holders, 0); assert.equal(channel.signal.aborted, true);
  }
});

test('actual owned monitor waits for a delayed descendant after coordinator exit', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true });
  assert.deepEqual(await channel.status(), { type: 'status', state: 'inactive', authority: 'none' });
  await channel.probeDescendant(400);
  const start = performance.now();
  await channel.close();
  assert.ok(performance.now() - start >= 350, 'coordinator exit must not complete a retained descendant');
  assert.equal(life.status().holders, 0);
});

test('owned shutdown retains terminal ingress after the native stop event precedes queued frames', async () => {
  for (let i = 0; i < 8; i++) {
    const life = createAssistLifecycle();
    const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true });
    // Monitor signals its native event before forwarding Shutdown. Earlier
    // ordinary frames may still be queued in the independent pipe writer.
    const pending = Promise.allSettled(Array.from({ length: 4 }, () => channel.probeWait(1000)));
    await channel.close();
    assert.equal(life.status().holders, 0);
    assert.ok((await pending).every(result => result.status === 'rejected'));
  }
});

test('actual owned monitor keeps shutdown independent of eight stalled requests', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true });
  const requests = Promise.allSettled(Array.from({ length: 8 }, () => channel.probeWait(10000)));
  await assert.rejects(channel.status());
  const start = performance.now();
  await life.quiesce('update');
  assert.ok(performance.now() - start < 2000);
  assert.ok((await requests).every(r => r.status === 'rejected'));
  assert.equal(life.status().holders, 0);
});

test('actual owned monitor signals a stop-only descendant on coordinator crash and retains unproven cleanup', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true });
  const holder = await channel.probeStopHolder();
  // Both identifiers originate in the verified no-authority probe. No personal
  // process or active native executor is terminated by this test.
  process.kill(holder.coordinator);
  await assert.rejects(channel.wait(), { code: 'assist-cleanup-incomplete' });
  const deadline = Date.now() + 2000;
  let exited = false;
  while (Date.now() < deadline) {
    try { process.kill(holder.pid, 0); }
    catch (error) { if (error.code === 'ESRCH') { exited = true; break; } throw error; }
    await new Promise(resolve => setTimeout(resolve, 10));
  }
  assert.equal(exited, true, 'survivor needs the stop event before monitor waits for job zero');
  await assert.rejects(life.quiesce());
  assert.equal(life.status().holders, 1);
});

test('owned actual pipes refuse tampered images, malformed and replayed frames without authority', async () => {
  const manifest = JSON.parse(require('node:fs').readFileSync(manifestFile, 'utf8'));
  const options = { mode: '--tree-live-probe', extra: [String(manifest.size), manifest.sha256] };
  const greeting = { ...hello, version: 2 };
  await refused(null, { ...options, pid: process.pid + 1 });
  await refused(null, { ...options, extra: [String(manifest.size), '0'.repeat(64)] });
  await refused(null, { ...options, end: true });
  await refused(Buffer.from([10, 0, 0, 0, 123]), { ...options, end: true });
  await refused(frame('{"sequence":0,"sequence":0,"command":{"type":"shutdown"}}'), options);
  await refused(Buffer.concat([frame(greeting), frame({ type: 'pulse' }, 0)]), options);
  await refused(Buffer.concat([frame(greeting), frame({ type: 'input', approved: true }, 1)]), options);
});

test('owned watchdog handles a silent parent and output pressure independently', async () => {
  const manifest = JSON.parse(require('node:fs').readFileSync(manifestFile, 'utf8'));
  const options = { mode: '--tree-live-probe', extra: [String(manifest.size), manifest.sha256] };
  await refused(Buffer.from([10]), options);
  const wire = [frame({ ...hello, version: 2 })];
  for (let i = 1; i <= 8000; i++) wire.push(frame({ type: 'pulse' }, i));
  await refused(Buffer.concat(wire), { ...options, drain: false });
});

test('owned monitor crash cannot manufacture a successful lifecycle proof', async () => {
  const life = createAssistLifecycle();
  const channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true, ownedTree: true });
  process.kill(channel.pid); // Verified non-authorizing monitor fixture only.
  await assert.rejects(channel.wait(), { code: 'assist-cleanup-incomplete' });
  await assert.rejects(life.quiesce());
  assert.equal(life.status().holders, 1);
});

test('owned parent death revokes the monitor and its coordinator without ordinary dispatch', async () => {
  for (const forced of [false, true]) {
  // PowerShell startup must precede the bounded event-only survivor. A cold CI
  // shell can take longer than that probe's six-second lifetime. Keep both the
  // native timeout and the four-second post-parent-death assertion unchanged.
  const observer = await prepareProcessWatcher(binary);
  const script = String.raw`const {openLiveChannel}=require(process.argv[3]);
    const {createAssistLifecycle}=require(process.argv[4]);
    openLiveChannel({binary:process.argv[1],manifestFile:process.argv[2],lifecycle:createAssistLifecycle(),probe:true,ownedTree:true})
      .then(async channel=>{
        const holder=await channel.probeStopHolder();
        process.stdin.once('data',()=>process.exit(0));
        process.stdout.write(JSON.stringify([channel.pid,holder.coordinator,holder.pid])+'\n');
      })
      .catch(error=>process.stderr.write(String(error.stack||error),()=>process.exit(2)));`;
  const parent = spawn(process.execPath, ['-e', script, binary, manifestFile,
    require.resolve('../src/assist/live-channel.cjs'), require.resolve('../src/assist/lifecycle.cjs')],
    { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  let diagnostic = '';
  parent.stderr.on('data', bytes => { diagnostic = (diagnostic + bytes).slice(-4096); });
  parent.stdin.on('error', () => {});
  const lines = require('node:readline').createInterface({ input: parent.stdout });
  const exited = once(parent, 'exit');
  const timer = setTimeout(() => parent.kill(), 10_000);
  try {
    const [text] = await Promise.race([once(lines, 'line'), exited.then(([code]) => {
      throw Error('Fixture parent ended before process registration: ' + code + ' ' + diagnostic);
    })]);
    const pids = JSON.parse(text);
    assert.equal(new Set(pids).size, 3);
    assert.ok(pids.every(pid => Number.isInteger(pid) && pid > 0));
    await observer.retain(pids.map((pid, index) => ({ pid, role: ['monitor', 'coordinator', 'holder'][index] })));
    if (forced) parent.kill(); // Only this disposable no-authority Node fixture.
    else parent.stdin.end('exit\n');
    const [code, signal] = await exited;
    if (!forced) { assert.equal(code, 0); assert.equal(signal, null); }
    const result = await observer.finish(4000);
    assert.ok(result.every(item => item.exited), 'Original process survived parent death: ' + JSON.stringify(result));
    assert.deepEqual(result.map(item => item.code), [125, 125, 0], 'Parent-loss cleanup must execute, not be killed by the parent job: ' + JSON.stringify(result));
  } finally { clearTimeout(timer); observer?.close(); lines.close(); parent.kill(); }
  }
});

test('actual urgent shutdown interrupts eight stalled ordinary requests and is joined by app cleanup', async () => {
  const life = createAssistLifecycle(), channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true });
  const pending = Array.from({ length: 8 }, () => channel.probeWait(10_000));
  const results = Promise.allSettled(pending);
  await assert.rejects(channel.status());
  const started = performance.now(); const quit = life.quiesce('update');
  assert.equal(channel.signal.aborted, true);
  await quit; assert.ok(performance.now() - started < 2000, 'shutdown must not wait for ordinary dispatch');
  assert.ok((await results).every(r => r.status === 'rejected')); assert.equal(life.status().holders, 0);
});

test('actual native liveness stays independent while ordinary dispatch waits beyond the watchdog period', async () => {
  const life = createAssistLifecycle(), channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true });
  try {
    assert.deepEqual(await channel.probeWait(5500), { type: 'probe-wait' });
    assert.equal((await channel.status()).authority, 'none');
  } finally { await channel.close(); }
});

test('live actual pipes reject malformed/oversized/incomplete frames, wrong parents, replay and sensitive commands', async () => {
  await refused(null, { pid: process.pid + 1 });
  await refused(null, { mode: '--live' }); // Node is not the packaged Palisade parent.
  await refused(Buffer.from([1, 0, 1, 0]));
  await refused(Buffer.from([10, 0, 0, 0, 123]), { end: true });
  await refused(frame('{"sequence":0,"sequence":0,"command":{"type":"shutdown"}}'));
  await refused(frame({ type: 'status' }));
  await refused(Buffer.concat([frame(hello), frame({ type: 'pulse' }, 0)]));
  for (const command of [{ type: 'sign', data: 'arbitrary' }, { type: 'input', approved: true },
    { type: 'status', consent: true }, { type: 'probe-wait', milliseconds: 10001 }]) {
    await refused(Buffer.concat([frame(hello), frame(command, 1)]));
  }
  await refused(null, { end: true });
});

test('full ordinary native queue fails closed instead of blocking the urgent reader', async () => {
  const wire = [frame(hello)];
  for (let i = 1; i <= 12; i++) wire.push(frame({ type: 'probe-wait', milliseconds: 10000 }, i));
  const started = performance.now(); await refused(Buffer.concat(wire));
  assert.ok(performance.now() - started < 2000);
});

test('independent live watchdog rejects a silent/partial parent and bounded output saturation', async () => {
  await refused(null);
  await refused(Buffer.from([10]));
  const wire = [frame(hello)];
  for (let i = 1; i <= 8000; i++) wire.push(frame({ type: 'pulse' }, i));
  await refused(Buffer.concat(wire), { drain: false });
});

test('actual live EOF and guardian crash cannot masquerade as lifecycle cleanup proof', async () => {
  const life = createAssistLifecycle(), channel = await openLiveChannel({ binary, manifestFile, lifecycle: life, probe: true });
  // Only this no-authority test process is terminated. A production broker has
  // no kill path. Lifecycle ownership must remain after an unproven exit.
  process.kill(channel.pid);
  await assert.rejects(channel.wait(), { code: 'assist-cleanup-incomplete' });
  await assert.rejects(life.quiesce()); assert.equal(life.status().holders, 1);
});

test('actual live parent death revokes without waiting for an ordinary request', async () => {
  const script = `const {spawn}=require('node:child_process');
    const child=spawn(process.argv[1],['--live-probe',String(process.pid)],{windowsHide:true,stdio:['pipe','pipe','ignore']});
    child.stdin.on('error',()=>{});child.on('exit',()=>process.exit(2));
    child.stdout.once('data',()=>process.stdout.write(String(child.pid),()=>process.exit(0)));
    child.stdin.write(Buffer.from(process.argv[2],'base64'));`;
  const parent = spawn(process.execPath, ['-e', script, binary, frame(hello).toString('base64')], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let text = ''; parent.stdout.on('data', chunk => { text += chunk; }); parent.stderr.resume();
  const timer = setTimeout(() => parent.kill(), 5000);
  try { assert.equal((await once(parent, 'exit'))[0], 0); } finally { clearTimeout(timer); }
  const pid = Number(text); assert.ok(Number.isInteger(pid) && pid > 0);
  const deadline = Date.now() + 4000;
  while (Date.now() < deadline) {
    try { process.kill(pid, 0); } catch (error) { if (error.code === 'ESRCH') return; throw error; }
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  // Reap only a known non-authorizing fixture after the failed assertion.
  try { process.kill(pid); } catch {}
  assert.fail('live probe survived parent death');
});
test('actual helper build contains cleanup panic, keeps failure sticky and joins later work', async () => {
  const p = raw({ mode: '--cleanup-probe', drain: false });
  const closed = once(p.child, 'close');
  const chunks = []; p.child.stdout.on('data', chunk => { chunks.push(chunk); });
  try {
    const [code, signal] = await p.exit; await closed; assert.equal(signal, null); assert.equal(code, 0);
    const bytes = Buffer.concat(chunks); assert.ok(bytes.length > 4 && bytes.length <= 1024);
    assert.equal(bytes.readUInt32LE(0), bytes.length - 4);
    assert.deepEqual(JSON.parse(bytes.subarray(4).toString('utf8')), { protocol: 'palisade.assist.cleanup-probe',
      version: 1, authority: 'none', synthetic: true, panic: 'retained-failure', retry: 'retained-failure', later: 'joined' });
  } finally { p.close(); }
});
