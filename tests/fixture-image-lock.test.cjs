'use strict';
const test = require('node:test'), assert = require('node:assert/strict'), path = require('node:path');
const { unlocked } = require('../scripts/fixture-image-lock.cjs');
const executable = path.resolve('node_modules/electron/dist/electron.exe');
const failure = extra => Object.assign(Error('fixture open failure'), { code: 'UNKNOWN', errno: -4094, syscall: 'open', path: executable }, extra);
test('fixture image restoration retries only its exact Windows open error', async () => {
  let calls = 0;
  const operation = () => { if (++calls === 1) throw failure(); return 'restored'; };
  if (process.platform === 'win32') {
    assert.equal(await unlocked(operation, executable), 'restored');
    assert.equal(calls, 2);
  } else {
    await assert.rejects(unlocked(operation, executable)); assert.equal(calls, 1);
  }
});
test('unknown errors on another path, another operation or without an exact target are never retried', async () => {
  for (const [error, target] of [[failure(), null], [failure({ path: path.resolve('other.exe') }), executable],
    [failure({ syscall: 'write' }), executable], [failure({ code: 'EIO' }), executable]]) {
    let calls = 0;
    await assert.rejects(unlocked(() => { calls++; throw error; }, target));
    assert.equal(calls, 1);
  }
});
