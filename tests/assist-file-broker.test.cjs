'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const { FileBroker } = require('../src/assist/file-broker.cjs');
const bytes = Buffer.from('a selected fixture file');
const metadata = { name: 'fixture.txt', size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
function setup() {
  const abort = new AbortController(), grants = new Set(), events = [], written = [], handles = new Set();
  const files = {
    selectSend: async () => { events.push('source-dialog'); handles.add('send'); return { handle: 'send', metadata }; },
    selectReceive: async () => { events.push('save-dialog'); handles.add('receive'); return 'receive'; },
    read: async (_handle, offset) => bytes.subarray(offset),
    write: async (_handle, offset, chunk) => { assert.equal(offset, Buffer.concat(written).length); written.push(Buffer.from(chunk)); },
    commit: async () => { events.push('commit'); handles.delete('receive'); },
    cancel: async handle => { events.push(['cancel', handle]); handles.delete(handle); },
  };
  const transport = { control: async (kind, value) => events.push([kind, value]), chunk: async (index, value) => events.push(['chunk', index, Buffer.from(value)]) };
  const broker = new FileBroker({ files, transport, signal: abort.signal,
    authorize: async cap => grants.has(cap), onState: value => events.push(['state', value]) });
  return { broker, abort, grants, events, files, transport, handles, written };
}
const turn = () => new Promise(resolve => setImmediate(resolve));
test('outgoing file requires native selection and recipient acceptance before reading', async () => {
  const t = setup(); await assert.rejects(t.broker.userSend()); assert.equal(t.events.includes('source-dialog'), false);
  t.grants.add('file_to_guest'); const sending = t.broker.userSend(); await turn();
  const offer = t.events.find(e => e[0] === 'file-offer')[1]; assert.deepEqual(t.events.filter(e => e[0] === 'chunk'), []);
  await t.broker.receivedControl('file-accept', { transfer: offer.transfer }); await sending;
  assert.equal(t.events.find(e => e[0] === 'chunk')[2].toString(), bytes.toString()); assert.equal(t.handles.size, 0);
  assert(t.events.some(e => e[0] === 'file-complete')); t.broker.close();
});
test('incoming offer cannot create a file without a local accept/save decision', async () => {
  const t = setup(); t.grants.add('file_to_host'); const offer = { transfer: 'transfer_fixture', ...metadata };
  await t.broker.offer(offer); assert.equal(t.events.includes('save-dialog'), false); assert.equal(t.handles.size, 0);
  await t.broker.userAccept(offer.transfer); await t.broker.receiveChunk(0, bytes);
  await t.broker.receivedControl('file-complete', { transfer: offer.transfer, chunks: 1 });
  assert.deepEqual(Buffer.concat(t.written), bytes); assert(t.events.includes('commit')); assert.equal(t.handles.size, 0); t.broker.close();
});
test('tampering, out-of-order chunks and revocation never commit and cancel native handles', async () => {
  for (const mode of ['tamper', 'order', 'revoke']) {
    const t = setup(); t.grants.add('file_to_host'); await t.broker.offer({ transfer: 'transfer_fixture', ...metadata });
    await t.broker.userAccept('transfer_fixture');
    if (mode === 'revoke') { t.grants.clear(); await assert.rejects(t.broker.receiveChunk(0, bytes)); }
    else if (mode === 'order') await assert.rejects(t.broker.receiveChunk(1, bytes));
    else {
      await t.broker.receiveChunk(0, Buffer.alloc(bytes.length));
      await assert.rejects(t.broker.receivedControl('file-complete', { transfer: 'transfer_fixture', chunks: 1 }));
    }
    await turn(); assert.equal(t.events.includes('commit'), false); assert.equal(t.handles.size, 0); t.broker.close();
  }
});
test('session stop cancels stalled transfer, and a cancelled selection cannot leak a handle', async () => {
  const t = setup(); t.grants.add('file_to_guest'); let selected;
  t.files.selectSend = () => new Promise(resolve => { selected = resolve; });
  const sending = t.broker.userSend(); await turn(); await assert.rejects(t.broker.userSend());
  t.abort.abort(); t.handles.add('send'); selected({ handle: 'send', metadata });
  await assert.rejects(sending); assert.equal(t.handles.size, 0);
});
test('metadata, duplicate completion, oversized chunks and cross-direction acceptance fail closed', async () => {
  const t = setup(); t.grants.add('file_to_host');
  await assert.rejects(t.broker.offer({ transfer: 'transfer_fixture', ...metadata, name: '../secret' }));
  await t.broker.offer({ transfer: 'transfer_fixture', ...metadata }); await t.broker.userAccept('transfer_fixture');
  await assert.rejects(t.broker.receiveChunk(0, Buffer.alloc(65537))); assert.equal(t.handles.size, 0);
  await assert.rejects(t.broker.receivedControl('file-accept', { transfer: 'transfer_fixture' }));
  await assert.rejects(t.broker.receivedControl('file-complete', { transfer: 'transfer_fixture', chunks: 0 }));
  t.broker.close();
});
test('directional revocation cancels a pending selection through its operation signal', async () => {
  const t=setup(),send=new AbortController(); t.grants.add('file_to_guest');
  t.broker.sendSignal=send.signal;send.signal.addEventListener('abort',t.broker.sendRevoked,{once:true});
  let resume,operationSignal;t.files.selectSend=({signal})=>{operationSignal=signal;return new Promise(resolve=>{resume=resolve;});};
  const pending=t.broker.userSend();await turn();send.abort();assert.equal(operationSignal.aborted,true);
  t.handles.add('send');resume({handle:'send',metadata});await assert.rejects(pending);assert.equal(t.handles.size,0);t.broker.close();
});
