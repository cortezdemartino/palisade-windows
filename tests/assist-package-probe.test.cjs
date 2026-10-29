'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const { verifiedCleanupReport } = require('../scripts/verify-assist-package.cjs');
const name = 'actual helper build contains cleanup panic, keeps failure sticky and joins later work';
const passed = `TAP version 13\n# Subtest: ${name}\nok 1 - ${name}\n1..1\n# tests 1\n# pass 1\n# fail 0\n# cancelled 0\n# skipped 0\n`;
test('packaged panic verification requires the exact named successful probe, not runner exit/pass count', () => {
  assert.equal(verifiedCleanupReport(passed), true);
  assert.equal(verifiedCleanupReport(passed.replaceAll('\n', '\r\n')), true);
  // A zero-match Node runner may report its file as one successful test.
  assert.equal(verifiedCleanupReport(passed.replaceAll(name, 'tests/assist-live-process.native.cjs')), false);
});
test('skipped, renamed, failed, cancelled, empty and contradictory probe reports cannot publish', () => {
  for (const report of [undefined, '', passed.replace(`ok 1 - ${name}`, `ok 1 - ${name} # SKIP`),
    passed.replace(`ok 1 - ${name}`, `not ok 1 - ${name}`), passed.replace('# pass 1', '# pass 0'),
    passed.replace('# fail 0', '# fail 1'), passed.replace('# cancelled 0', '# cancelled 1'),
    passed.replace('# Subtest: ' + name, '# Subtest: renamed probe'), passed.replace(`ok 1 - ${name}`, 'ok 1 - renamed probe')]) {
    assert.equal(verifiedCleanupReport(report), false);
  }
});
