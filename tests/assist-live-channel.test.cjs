'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const { readFileSync } = require('node:fs'), { EventEmitter } = require('node:events');
const { PassThrough, Writable } = require('node:stream');
const vm = require('node:vm');
const { createAssistLifecycle } = require('../src/assist/lifecycle.cjs');
const source = readFileSync(require.resolve('../src/assist/live-channel.cjs'), 'utf8');
const tick = () => new Promise(resolve => setImmediate(resolve));
const normal = value => JSON.parse(JSON.stringify(value));
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }
function harness({ verify = async () => {}, reply, pressure = false, launchError } = {}) {
  const life = createAssistLifecycle(), child = new EventEmitter(), requests = [];
  let spawns = 0, kills = 0, rawWrite;
  child.pid = 4242; child.stdout = new PassThrough(); child.stderr = new PassThrough();
  child.kill = () => { kills++; throw Error('live channels must never kill cleanup holders'); };
  const sendBytes = bytes => {
    const prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length);
    child.stdout.write(Buffer.concat([prefix, bytes]));
  };
  const send = (sequence, result) => sendBytes(Buffer.from(JSON.stringify({ sequence, result })));
  child.stdin = new Writable({ write(bytes, _encoding, done) {
    const r = JSON.parse(bytes.subarray(4).toString('utf8')); requests.push(r);
    setImmediate(() => {
      if (r.command.type === 'hello') send(r.sequence, { type: 'hello', protocol: r.command.protocol,
        version: 1, challenge: r.command.challenge, mode: 'live-probe', nativeInput: false, cleanupScope: 'channel-only' });
      else reply?.(r, { send, sendBytes, child });
    });
    done();
  } });
  rawWrite = child.stdin.write.bind(child.stdin);
  child.stdin.write = bytes => { const ok = rawWrite(bytes); return pressure && requests.at(-1)?.command.type !== 'hello' ? false : ok; };
  const module = { exports: {} };
  vm.runInNewContext(source, { module, Buffer, AbortController, process,
    setTimeout, clearTimeout, setInterval, clearInterval,
    require(name) {
      if (name === 'node:child_process') return { spawn(_binary, args, options) {
        spawns++; assert.equal(args[0], '--live-probe'); assert.equal(options.shell, false); assert.equal(options.windowsHide, true);
        assert.equal(life.status().holders, 1, 'ownership must precede spawn');
        if (launchError === 'sync') throw Error('spawn fixture');
        if (launchError === 'async') {
          child.pid = undefined;
          setImmediate(() => { child.emit('error', Error('launch fixture')); child.emit('close', -4058, null); });
          return child;
        }
        setImmediate(() => child.emit('spawn')); return child;
      } };
      if (name === './guardian.cjs') return { verifyGuardian: verify };
      return require(name);
    },
  });
  const open = options => module.exports.openLiveChannel({ binary: 'fixture', manifestFile: 'fixture', lifecycle: life, probe: true, ...options });
  const finish = () => {
    const r = requests.find(r => r.command.type === 'shutdown'); assert.ok(r);
    send(r.sequence, { type: 'cleanup-complete', challenge: requests[0].command.challenge, scope: 'channel-only' });
    child.emit('close', 0, null);
  };
  return { life, child, requests, open, send, sendBytes, finish, spawns: () => spawns, kills: () => kills };
}

test('global shutdown during verification forbids spawn and retains ownership until no-child proof', async () => {
  const verification = deferred(), h = harness({ verify: () => verification.promise });
  const opening = h.open(); assert.equal(h.life.status().holders, 1);
  let ended = false; const quit = h.life.quiesce().then(() => { ended = true; });
  await tick(); assert.equal(ended, false); assert.equal(h.spawns(), 0);
  verification.resolve(); await assert.rejects(opening); await quit;
  assert.equal(h.spawns(), 0); assert.deepEqual(h.life.status(), { state: 'closed', holders: 0 });
});

test('integrity rejection and failed launches release ownership only when no process was created', async () => {
  for (const options of [{ verify: async () => { throw Error('integrity fixture'); } }, { launchError: 'sync' }, { launchError: 'async' }]) {
    const h = harness(options);
    await assert.rejects(h.open()); await h.life.quiesce();
    assert.equal(h.life.status().holders, 0); assert.equal(h.kills(), 0);
  }
});

test('live close needs matching cleanup proof AND actual process exit before lifecycle completion', async () => {
  const h = harness(), channel = await h.open();
  assert.equal(channel.request, undefined); assert.equal(channel.sign, undefined);
  const quit = h.life.quiesce(); let completed = false; void quit.then(() => { completed = true; });
  assert.equal(channel.signal.aborted, true); await tick();
  const r = h.requests.find(r => r.command.type === 'shutdown');
  h.send(r.sequence, { type: 'cleanup-complete', challenge: h.requests[0].command.challenge, scope: 'channel-only' });
  await tick(); assert.equal(completed, false); assert.equal(h.life.status().holders, 1);
  h.child.emit('close', 0, null); await quit;
  assert.equal(h.life.status().holders, 0); assert.equal(h.kills(), 0);
});

test('eight stalled ordinary calls cannot occupy the shutdown slot; revoke is synchronous', async () => {
  const h = harness(), channel = await h.open();
  const calls = Array.from({ length: 8 }, () => channel.status());
  const results = Promise.allSettled(calls);
  await assert.rejects(channel.status());
  const ending = channel.close(); assert.equal(channel.signal.aborted, true);
  assert.equal(h.requests.filter(r => r.command.type === 'shutdown').length, 1);
  assert.ok((await results).every(r => r.status === 'rejected'));
  // Late ordinary results remain typed but can never reactivate their callers.
  for (const r of h.requests.filter(r => r.command.type === 'status')) h.send(r.sequence, { type: 'status', state: 'inactive', authority: 'none' });
  h.finish(); await ending; assert.equal(h.kills(), 0);
});

test('unexpected exit, substituted challenge/scope, nonzero exit and output after proof retain a failed holder', async () => {
  for (const mode of ['exit-only', 'challenge', 'scope', 'nonzero', 'after-proof']) {
    const h = harness(), channel = await h.open(), ending = channel.close();
    void ending.catch(() => {});
    const r = h.requests.find(r => r.command.type === 'shutdown');
    if (mode !== 'exit-only') h.send(r.sequence, { type: 'cleanup-complete',
      challenge: mode === 'challenge' ? 'A'.repeat(43) : h.requests[0].command.challenge,
      scope: mode === 'scope' ? 'all-descendants' : 'channel-only' });
    if (mode === 'after-proof') h.send(r.sequence, { type: 'pulse' });
    h.child.emit('close', mode === 'nonzero' ? 125 : 0, null);
    await assert.rejects(ending, { code: 'assist-cleanup-incomplete' });
    await assert.rejects(h.life.quiesce()); assert.equal(h.life.status().holders, 1); assert.equal(h.kills(), 0);
  }
});

test('fragmented/coalesced replies are bounded and preserve exact request correlation', async () => {
  const h = harness(), channel = await h.open();
  const a = channel.status(), b = channel.status();
  const pending = h.requests.filter(r => r.command.type === 'status');
  const wire = Buffer.concat(pending.map(r => {
    const bytes = Buffer.from(JSON.stringify({ sequence: r.sequence, result: { type: 'status', state: 'inactive', authority: 'none' } }));
    const prefix = Buffer.alloc(4); prefix.writeUInt32LE(bytes.length); return Buffer.concat([prefix, bytes]);
  }));
  for (const part of [wire.subarray(0, 1), wire.subarray(1, 7), wire.subarray(7)]) h.child.stdout.write(part);
  assert.deepEqual(normal(await a), { type: 'status', state: 'inactive', authority: 'none' }); await b;
  const end = channel.close(); h.finish(); await end;
});

test('malformed, oversized, invalid UTF-8, duplicate, unknown and replayed native replies revoke without kill', async () => {
  for (const mode of ['oversize', 'utf8', 'duplicate', 'unknown', 'replay', 'bad-json']) {
    const h = harness(), channel = await h.open();
    const status = channel.status(); void status.catch(() => {});
    const id = h.requests.at(-1).sequence;
    if (mode === 'oversize') h.child.stdout.write(Buffer.from([1, 0, 1, 0]));
    if (mode === 'utf8') h.sendBytes(Buffer.from([0xff, 0xfe]));
    if (mode === 'duplicate') h.sendBytes(Buffer.from(`{"sequence":${id},"sequence":${id},"result":{"type":"status","state":"inactive","authority":"none"}}`));
    if (mode === 'unknown') h.send(id, { type: 'status', state: 'inactive', authority: 'native' });
    if (mode === 'replay') h.send(0, { type: 'pulse' });
    if (mode === 'bad-json') h.sendBytes(Buffer.from('{'));
    assert.equal(channel.signal.aborted, true); await assert.rejects(status);
    h.child.emit('close', 125, null); await assert.rejects(channel.wait());
    assert.equal(h.life.status().holders, 1); assert.equal(h.kills(), 0);
  }
});

test('backpressure closes the input pipe and keeps output/cleanup ownership alive', async () => {
  const h = harness({ pressure: true }), channel = await h.open();
  const waiting = channel.status(); void waiting.catch(() => {});
  channel.revoke(); assert.equal(h.child.stdin.destroyed, true); assert.equal(h.child.stdout.destroyed, false);
  await assert.rejects(waiting); h.child.emit('close', 125, null);
  await assert.rejects(channel.close()); assert.equal(h.life.status().holders, 1); assert.equal(h.kills(), 0);
});

test('external cancellation revokes and invalid probe requests cannot create arbitrary native commands', async () => {
  const h = harness(), cancel = new AbortController(), channel = await h.open({ signal: cancel.signal });
  await assert.rejects(channel.probeWait(0)); await assert.rejects(channel.probeWait(10001));
  assert.equal(h.requests.length, 1);
  cancel.abort(); assert.equal(channel.signal.aborted, true); h.finish(); await channel.wait(); await tick();
  assert.equal(h.life.status().holders, 0);
});
