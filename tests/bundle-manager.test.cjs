'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { mkdtemp, writeFile, rm } = require('node:fs/promises');
const { createHash } = require('node:crypto');
const path = require('node:path');
const os = require('node:os');
const { verifyManager } = require('../scripts/bundle-manager.cjs');
test('manager dependency requires the pinned size and SHA-256 and rejects substitution', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'palisade-manager-test-'));
  try {
    const filename = path.join(directory, 'manager.exe');
    const bytes = Buffer.from('verified manager');
    const expected = { size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
    await writeFile(filename, bytes);
    await verifyManager(filename, expected);
    await writeFile(filename, Buffer.alloc(bytes.length));
    await assert.rejects(verifyManager(filename, expected), /SHA-256/);
    await writeFile(filename, 'partial');
    await assert.rejects(verifyManager(filename, expected), /size/);
    await assert.rejects(verifyManager(path.join(directory, 'missing'), expected), /missing/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
