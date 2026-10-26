'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { stripTypeScriptTypes } = require('node:module');
const directory = path.join(__dirname, 'fixtures');
const fixtures = require('./fixtures/assist-control-v2.json');
const provenance = require('./fixtures/assist-control-v2.provenance.json');
const url = text => 'data:text/javascript;base64,' + Buffer.from(stripTypeScriptTypes(text, { mode: 'transform' })).toString('base64');
const legacy = url(fs.readFileSync(path.join(directory, 'assist-control-common.web.ts'), 'utf8'));
const source = fs.readFileSync(path.join(directory, 'assist-control-v2.web.ts'), 'utf8');
const api = import(url(source.replace("from './protocol'", 'from ' + JSON.stringify(legacy))));
const key = Uint8Array.from(Buffer.from(fixtures.issuerPublicKey, 'base64url'));
test('stream v2 source and fixtures are exact pinned companion Git blobs', () => {
  assert.equal(provenance.commit, 'c0778935a6a08303bf7a9d7de8f2fa75aa8ef536');
  for (const file of provenance.files) {
    const bytes = Buffer.from(fs.readFileSync(path.join(directory, file.local), 'utf8').replace(/\r\n/g, '\n'));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), file.sha256);
    assert.equal(createHash('sha1').update('blob ' + bytes.length + '\0').update(bytes).digest('hex'), file.gitBlob);
  }
});
test('canonical v2 grant and lease verifiers agree with every shared positive and negative fixture', async () => {
  const contract = await api;
  for (const [name, method] of [['grants', 'verifyStreamGrant'], ['leases', 'verifyStreamLease']]) {
    for (const entry of fixtures[name].valid) await contract[method](entry.signed, key, { ...fixtures.expect, ...entry.expect });
    for (const entry of fixtures[name].invalid) await assert.rejects(contract[method](entry.signed, key, { ...fixtures.expect, ...entry.expect }), error => error.code === entry.error, entry.name);
  }
});
test('canonical stream message parser accepts only versioned bounded messages', async () => {
  const contract = await import(legacy);
  for (const entry of fixtures.messages.valid) assert.deepEqual(contract.parseControlMessage(entry, 2), entry);
  for (const entry of fixtures.messages.invalid) assert.throws(() => contract.parseControlMessage(entry.message, 2), error => error.code === entry.error, entry.name);
});
