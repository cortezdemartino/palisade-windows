'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { ClipboardBroker } = require('../src/assist/clipboard-broker.cjs');
function setup() {
  const abort = new AbortController(), events = [], grants = new Set(); let value = 'local secret';
  const broker = new ClipboardBroker({ signal: abort.signal, syncAgreement: () => true,
    clipboard: { readText: () => { events.push('read'); return value; }, writeText: text => { events.push('write'); value = text; } },
    authorize: async capability => { if (!grants.has(capability)) throw new Error('denied'); return true; },
    send: async text => events.push(['sent', text]), schedule: () => 1, cancel: () => {} });
  return { broker, abort, grants, events, get value() { return value; } };
}
test('clipboard stays off, refuses before reading and requires separate directional permission', async () => {
  const t = setup(); assert.equal(t.broker.mode, 'off');
  await assert.rejects(t.broker.userSend()); assert.deepEqual(t.events, []);
  t.grants.add('clipboard_to_guest'); await t.broker.userSend();
  assert.deepEqual(t.events, ['read', ['sent', 'local secret']]);
  await assert.rejects(t.broker.receive('remote')); assert.equal(t.value, 'local secret');
  t.broker.close();
});
test('incoming text waits for a local copy action; sync is explicit and deduplicated', async () => {
  const t = setup(); t.grants.add('clipboard_to_host'); t.grants.add('clipboard_to_guest');
  await t.broker.receive('remote'); assert.equal(t.value, 'local secret'); assert.equal(t.broker.pending.text, 'remote');
  await t.broker.userCopyIncoming(); assert.equal(t.value, 'remote');
  await t.broker.setMode('both'); await t.broker.syncOnce(); assert.deepEqual(t.events, ['write', 'read']);
  await t.broker.receive('new remote'); assert.equal(t.value, 'new remote');
  t.broker.close();
});
test('revocation clears incoming data and timers and never writes or sends afterward', async () => {
  const t = setup(); t.grants.add('clipboard_to_host'); await t.broker.receive('remote private text');
  t.abort.abort(); assert.equal(t.broker.pending, null); assert.equal(t.broker.mode, 'off');
  await assert.rejects(t.broker.userCopyIncoming()); await assert.rejects(t.broker.userSend());
  assert.deepEqual(t.events, []);
});
test('malformed/oversized text is refused without an OS clipboard write', async () => {
  const t = setup(); t.grants.add('clipboard_to_host');
  for (const value of ['bad\0text', '\ud800', 'x'.repeat(65_537)]) await assert.rejects(t.broker.receive(value));
  assert.deepEqual(t.events, []); t.broker.close();
});
test('pause or loss of peer agreement cancels an in-flight automatic clipboard read/send', async () => {
  for (const mode of ['pause','agreement']) {
    const t=setup();t.grants.add('clipboard_to_guest');let agreed=true;
    t.broker.syncAgreement=()=>agreed;await t.broker.setMode('send');
    let resume;t.broker.authorize=()=>new Promise(resolve=>{resume=resolve;});
    const operation=t.broker.syncOnce();await Promise.resolve();
    if(mode==='pause')t.broker.pause();else agreed=false;
    resume(true);await assert.rejects(operation);assert.deepEqual(t.events,[]);t.broker.close();
  }
});
test('helper role uses the opposite direction and capability loss clears staged clipboard data', async () => {
  const stop=new AbortController(),receive=new AbortController(),send=new AbortController();const reads=[],grants=new Set(['clipboard_to_host','clipboard_to_guest']);
  const broker=new ClipboardBroker({role:'helper',signal:stop.signal,sendSignal:send.signal,receiveSignal:receive.signal,
    clipboard:{readText:()=>{reads.push('read');return 'helper text';},writeText:()=>reads.push('write')},
    authorize:async cap=>{reads.push(cap);return grants.has(cap);},send:async()=>{}});
  await broker.userSend();assert(reads.includes('clipboard_to_host'));await broker.receive('publisher text');
  receive.abort();assert.equal(broker.pending,null);await assert.rejects(broker.userCopyIncoming());
  assert.equal(reads.includes('write'),false);await broker.userSend();send.abort();await assert.rejects(broker.userSend());broker.close();
});
