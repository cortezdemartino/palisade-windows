'use strict';
// Fixed no-authority executables and private OS handles only. No physical input,
// identity, clipboard, files, production service or endpoint activation.
const test = require('node:test'), assert = require('node:assert/strict');
const path = require('node:path'), { spawn } = require('node:child_process');
const { once } = require('node:events');
const { verifyGuardian } = require('../src/assist/guardian.cjs');
const { prepareProcessWatcher } = require('./native-process-watch.cjs');
const binary = process.env.PALISADE_ASSIST_TEST_BINARY || path.resolve('.assist/Palisade-Assist.exe');
const manifestFile = process.env.PALISADE_ASSIST_TEST_MANIFEST || path.resolve('src/assist/guardian-integrity.json');
function frame(value) {
  const bytes = Buffer.from(JSON.stringify(value)), prefix = Buffer.alloc(4);
  prefix.writeUInt32LE(bytes.length); return Buffer.concat([prefix, bytes]);
}
async function start(scenario, corrupt = false) {
  const manifest = await verifyGuardian(binary, manifestFile);
  const child = spawn(binary, ['--custody-monitor-probe', String(process.pid), String(manifest.size),
    corrupt ? '0'.repeat(64) : manifest.sha256, scenario], {
    detached: true, windowsHide: true, shell: false, stdio: ['pipe', 'pipe', 'pipe'],
  });
  child.stdin.on('error', () => {});
  let errors = ''; child.stderr.on('data', bytes => { errors = (errors + bytes).slice(-2048); });
  const messages = [], waiters = [];
  let buffer = Buffer.alloc(0), malformed = false;
  child.stdout.on('data', bytes => {
    buffer = Buffer.concat([buffer, bytes]);
    while (buffer.length >= 4) {
      const size = buffer.readUInt32LE(0);
      if (!size || size > 4096) { malformed = true; return; }
      if (buffer.length < size + 4) return;
      try {
        const text = buffer.subarray(4, size + 4).toString('utf8'), value = JSON.parse(text);
        assert.equal(JSON.stringify(value), text); messages.push(value);
        waiters.splice(0).forEach(resolve => resolve());
      } catch { malformed = true; }
      buffer = buffer.subarray(size + 4);
    }
  });
  const closed = once(child, 'close');
  void closed.catch(() => {});
  // Only this known non-authorizing fixture is reaped on a harness failure.
  const timer = setTimeout(() => child.kill(), 12_000);
  return {
    child, closed,
    async spawned() {
      while (!messages.length) {
        await Promise.race([new Promise(resolve => waiters.push(resolve)), closed.then(([code]) => {
          throw Error('Custody probe ended before registration: ' + code + ' ' + errors);
        })]);
      }
      const first = messages[0];
      assert.equal(first.protocol, 'palisade.assist.custody-probe');
      assert.equal(first.version, 1); assert.equal(first.stage, 'spawned');
      assert.equal(first.authority, 'none'); assert.equal(first.mode, 'probe');
      assert.equal(first.monitor, child.pid);
      assert.equal(new Set([first.monitor, first.coordinator, first.child]).size, 3);
      return first;
    },
    async finish() {
      child.stdin.end(frame({ type: 'continue' }));
      const [code, signal] = await closed;
      assert.equal(code, 0, errors); assert.equal(signal, null);
      assert.equal(malformed, false); assert.equal(buffer.length, 0); assert.equal(messages.length, 2);
      const result = messages[1];
      assert.equal(result.protocol, 'palisade.assist.custody-probe'); assert.equal(result.version, 1);
      assert.equal(result.stage, 'complete'); assert.equal(result.authority, 'none');
      assert.equal(result.mode, 'probe'); assert.equal(result.nativeComplete, false);
      assert.equal(result.processesExited, true); assert.equal(result.scenario, scenario);
      return result;
    },
    close() { clearTimeout(timer); child.stdin.destroy(); if (child.exitCode == null) child.kill(); },
  };
}
test('actual custody actor separates live mirror, child recovery and process exit with no input authority', async () => {
  for (const scenario of ['mirror', 'child-recovery', 'hold-after-receipt', 'blocked-output', 'lost-arm', 'partial-arm', 'cancel-before-arm']) {
    const probe = await start(scenario);
    try {
      await probe.spawned(); const result = await probe.finish();
      assert.equal(result.probeComplete, true, scenario); assert.equal(result.receiptAccepted, true);
      if (scenario === 'hold-after-receipt') assert.equal(result.receiptBeforeExit, true);
      assert.equal(result.outputPressure, scenario === 'blocked-output');
      if (scenario !== 'cancel-before-arm') assert.equal(result.beforeMirror, false);
      assert.equal(result.armAccepted, !['lost-arm', 'partial-arm', 'cancel-before-arm'].includes(scenario));
    } finally { probe.close(); }
  }
});
test('actual custody actor refuses missing, malformed, wrong-scope and duplicate receipts even after job zero', async () => {
  for (const scenario of ['missing', 'truncated', 'wrong-ticket', 'wrong-mode', 'duplicate']) {
    const probe = await start(scenario);
    try {
      await probe.spawned(); const result = await probe.finish();
      assert.equal(result.receiptAccepted, false); assert.equal(result.probeComplete, false);
    } finally { probe.close(); }
  }
  const probe = await start('mirror', true);
  try { assert.equal((await probe.closed)[0], 125); } finally { probe.close(); }
});
test('custody monitor loss stops exact retained coordinator and child without a signaled native event', async () => {
  // Prewarm the observer before bounded fixture processes, then retain the
  // original handles before terminating only our no-authority monitor.
  const observer = await prepareProcessWatcher(binary);
  let probe;
  try {
    probe = await start('mirror'); const first = await probe.spawned();
    await observer.retain([
      { pid: first.monitor, role: 'monitor' }, { pid: first.coordinator, role: 'coordinator' },
      { pid: first.child, role: 'holder' },
    ]);
    probe.child.kill(); await probe.closed;
    const result = await observer.finish(4000);
    assert.ok(result.every(entry => entry.exited), JSON.stringify(result));
    assert.equal(result[2].code, 125, 'Unarmed child must fail closed on actual monitor death');
  } finally { observer.close(); probe?.close(); }
});
