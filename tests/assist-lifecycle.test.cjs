'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { createAssistLifecycle, installQuitBarrier } = require('../src/assist/lifecycle.cjs');
const flush = () => new Promise(resolve => setImmediate(resolve));
function deferred() { let resolve, reject; const promise = new Promise((done, fail) => { resolve = done; reject = fail; }); return { promise, resolve, reject }; }
test('quiescence revokes every holder synchronously, refuses new work and retains all cleanup', async () => {
  const life = createAssistLifecycle(), first = deferred(), second = deferred(), order = [];
  const a = life.hold({ revoke: () => { order.push('revoke-a'); }, wait: () => { order.push('wait-a'); return first.promise; } });
  const b = life.hold({ revoke: () => { order.push('revoke-b'); }, wait: () => { order.push('wait-b'); return second.promise; } });
  const completion = life.quiesce('update');
  assert.equal(life.quiesce('quit'), completion);
  assert.deepEqual(order, ['revoke-a', 'revoke-b']);
  assert.equal(a.signal.aborted, true); assert.equal(b.signal.aborted, true); assert.equal(life.signal.aborted, true);
  assert.equal(life.accepting(), false); assert.throws(() => life.hold({ revoke() {}, wait: async () => {} }));
  let done = false; void completion.then(() => { done = true; });
  first.resolve(); await flush(); assert.equal(done, false); assert.equal(life.status().holders, 1);
  second.resolve(); await completion;
  assert.deepEqual(life.status(), { state: 'closed', holders: 0 }); assert.equal(done, true);
  await life.quiesce(); assert.deepEqual(order, ['revoke-a', 'revoke-b', 'wait-a', 'wait-b']);
});
test('blocked cleanup retains ownership, permits a proof-only retry and never restores authority', async () => {
  const life = createAssistLifecycle(); let blocked = true, revokes = 0;
  life.hold({ revoke() { revokes++; }, wait: async () => { if (blocked) throw Error('protected desktop'); } });
  await assert.rejects(life.quiesce(), { code: 'assist-cleanup-incomplete' });
  assert.deepEqual(life.status(), { state: 'blocked', holders: 1 }); assert.equal(life.accepting(), false);
  blocked = false; await life.quiesce();
  assert.equal(life.status().state, 'closed'); assert.equal(revokes, 1); assert.equal(life.signal.aborted, true);
});
test('one failed revoker does not prevent other revocation or owned cleanup', async () => {
  const life = createAssistLifecycle(), order = [];
  life.hold({ revoke() { order.push('failed'); throw Error('pipe failed'); }, wait: async () => { order.push('exit-proven'); } });
  life.hold({ revoke() { order.push('other-revoked'); }, wait: async () => { order.push('other-exited'); } });
  const completion = life.quiesce(); assert.deepEqual(order, ['failed', 'other-revoked']);
  await assert.rejects(completion);
  assert.deepEqual(life.status(), { state: 'blocked', holders: 0 });
  await life.quiesce(); assert.equal(life.status().state, 'closed');
});
test('normal holder completion is reentrant and cannot remove ownership before cleanup', async () => {
  const life = createAssistLifecycle(), cleanup = deferred(); let holder, revocations = 0;
  holder = life.hold({ revoke() { revocations++; void holder.finish(); }, wait: () => cleanup.promise });
  const finished = holder.finish(); await flush();
  assert.equal(revocations, 1); assert.equal(life.status().holders, 1); assert.equal(life.accepting(), true);
  cleanup.resolve(); await finished; assert.equal(life.status().holders, 0);
  await life.quiesce(); assert.equal(revocations, 1);
});
test('async revocation and non-void cleanup cannot masquerade as completion', async () => {
  for (const owner of [
    { revoke: async () => {}, wait: async () => {} },
    { revoke() {}, wait: () => undefined },
    { revoke() {}, wait: async () => false },
  ]) {
    const life = createAssistLifecycle(); life.hold(owner);
    await assert.rejects(life.quiesce()); assert.equal(life.status().state, 'blocked');
  }
});
function appHarness() {
  const app = new EventEmitter(); let exits = 0, attempts = 0;
  app.quit = () => {
    attempts++; let prevented = false;
    app.emit('before-quit', { preventDefault() { prevented = true; } });
    if (!prevented) { exits++; app.emit('will-quit'); }
  };
  return { app, exits: () => exits, attempts: () => attempts };
}
test('quit coalesces concurrent requests, revokes immediately and flushes only after cleanup', async () => {
  const h = appHarness(), life = createAssistLifecycle(), cleanup = deferred(), order = [];
  life.hold({ revoke() { order.push('revoke'); }, wait: () => cleanup.promise });
  installQuitBarrier(h.app, { prepare: async () => { await life.quiesce(); order.push('flush'); }, onApproved() { order.push('approved'); } });
  h.app.on('will-quit', () => order.push('exit'));
  h.app.quit(); h.app.quit();
  assert.deepEqual(order, ['revoke']); assert.equal(h.exits(), 0);
  await flush(); assert.equal(h.exits(), 0);
  cleanup.resolve(); await flush();
  assert.deepEqual(order, ['revoke', 'flush', 'approved', 'exit']); assert.equal(h.exits(), 1);
});
test('failed quit preparation or profile flush leaves the app open and supports retry', async () => {
  for (const failure of ['prepare', 'flush']) {
    const h = appHarness(); let fail = true, blocked = 0;
    installQuitBarrier(h.app, { prepare: async () => { if (fail && failure === 'prepare') throw Error('cleanup'); },
      onApproved: () => { if (fail && failure === 'flush') throw Error('profile'); }, onBlocked: () => { blocked++; } });
    h.app.quit(); await flush(); assert.equal(h.exits(), 0); assert.equal(blocked, 1);
    fail = false; h.app.quit(); await flush(); assert.equal(h.exits(), 1);
  }
});
test('synchronous reentrant quit cannot create another preparation or skip the barrier', async () => {
  const h = appHarness(); let preparations = 0;
  installQuitBarrier(h.app, { prepare() { preparations++; h.app.quit(); return Promise.resolve(); } });
  h.app.quit(); await flush(); assert.equal(preparations, 1); assert.equal(h.exits(), 1);
});

test('pending cleanup reports progress once, retains the barrier and clears the notice on completion', async () => {
  const h = appHarness(), cleanup = deferred(); let notices = 0;
  installQuitBarrier(h.app, { prepare: () => cleanup.promise, onWaiting: () => { notices++; }, noticeAfterMs: 5 });
  h.app.quit(); h.app.quit();
  await new Promise(resolve => setTimeout(resolve, 20));
  assert.equal(notices, 1); assert.equal(h.exits(), 0);
  cleanup.resolve(); await flush(); assert.equal(h.exits(), 1);
  await new Promise(resolve => setTimeout(resolve, 10)); assert.equal(notices, 1);
  const quick = appHarness(); let lateNotices = 0;
  installQuitBarrier(quick.app, { prepare: async () => {}, onWaiting: () => { lateNotices++; }, noticeAfterMs: 5 });
  quick.app.quit(); await flush(); await new Promise(resolve => setTimeout(resolve, 10));
  assert.equal(quick.exits(), 1); assert.equal(lateNotices, 0);
});
