'use strict';
// Actual Windows processes/pipes, non-authorizing modes only. No input, identity,
// clipboard, dialogs, network service or native resource activation occurs.
const test = require('node:test'), assert = require('node:assert/strict');
const path = require('node:path'), { spawn } = require('node:child_process');
const { once } = require('node:events'), { randomBytes } = require('node:crypto');
const { openLiveChannel, PROTOCOL } = require('../src/assist/live-channel.cjs');
const { createAssistLifecycle } = require('../src/assist/lifecycle.cjs');
const binary = process.env.PALISADE_ASSIST_TEST_BINARY || path.resolve('.assist/Palisade-Assist.exe');
const manifestFile = process.env.PALISADE_ASSIST_TEST_MANIFEST || path.resolve('src/assist/guardian-integrity.json');
const hello = { type: 'hello', protocol: PROTOCOL, version: 1, challenge: randomBytes(32).toString('base64url') };
function frame(command, sequence = 0) {
  const bytes = Buffer.from(typeof command === 'string' ? command : JSON.stringify({ sequence, command }));
  const prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length); return Buffer.concat([prefix, bytes]);
}
function raw({ mode = '--live-probe', pid = process.pid, drain = true } = {}) {
  const child = spawn(binary, [mode, String(pid)], { windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'], shell: false });
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
