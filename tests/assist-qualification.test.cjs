'use strict';
const test = require('node:test'), assert = require('node:assert/strict'), fs = require('node:fs'), vm = require('node:vm');
const { validateQualification, qualification } = require('../src/assist/qualification.cjs');
const fixture = { origin: 'https://assist-fixture.example:8443', issuerFingerprint: 'a'.repeat(64), namespace: 'device-acceptance' };
test('qualification build requires an explicit fixed disposable origin, issuer pin and isolated namespace', () => {
  assert.equal(qualification, null);
  assert.deepEqual(validateQualification(fixture), fixture);
  for (const invalid of [null, { ...fixture, approved: true }, { ...fixture, origin: 'https://palisade.onyxops.org' },
    { ...fixture, origin: 'https://palisade.onyxops.org.' }, { ...fixture, origin: 'https://fixture.example:0' },
    { ...fixture, origin: 'http://localhost:5173' }, { ...fixture, origin: 'https://user:pass@fixture.example' },
    { ...fixture, origin: 'https://fixture.example/path' }, { ...fixture, issuerFingerprint: 'unverified' },
    { ...fixture, namespace: '../Palisade' }]) assert.throws(() => validateQualification(invalid));
});
test('qualified renderer trusts only the embedded test origin and still denies display and arbitrary devices', () => {
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(require.resolve('../src/security-policy.cjs'), 'utf8'), {
    module, URL, require: name => { assert.equal(name, './assist/qualification.cjs'); return { qualification: validateQualification(fixture) }; },
  });
  const policy = module.exports;
  assert.equal(policy.PALISADE_URL, fixture.origin + '/');
  assert.equal(policy.isPalisadeUrl(fixture.origin + '/assist'), true);
  assert.equal(policy.isPalisadeUrl('https://palisade.onyxops.org/'), false);
  assert.equal(policy.isPalisadeUrl('https://assist-fixture.example:8443.evil/'), false);
  assert.equal(policy.mayGrantPermission('display-capture', fixture.origin), false);
  assert.equal(policy.mayGrantPermission('media', fixture.origin, ['video']), false);
});
test('production packaging refuses a qualification profile before building or bundling helpers', async () => {
  for (const [env, id] of [[{}, 'org.onyxops.palisade'], [{ PALISADE_ASSIST_QUALIFICATION_BUILD: '1' }, 'org.onyxops.palisade']]) {
    const module = { exports: {} };
    vm.runInNewContext(fs.readFileSync(require.resolve('../scripts/before-pack.cjs'), 'utf8'), {
      module, process: { env }, require(name) {
        if (name === '../src/assist/qualification.cjs') return { qualification: fixture };
        if (name === './bundle-manager.cjs') return () => { throw Error('Unexpected bundle'); };
        if (name === './build-assist.cjs') return { buildAssist() { throw Error('Unexpected build'); } };
        if (name === './build-assist-preload.cjs') return { buildAssistPreload() {} };
        if (name === './build-update.cjs') return { buildUpdate() {} };
        throw Error('Unexpected import');
      },
    });
    await assert.rejects(module.exports({ packager: { appInfo: { id } } }), /separate unsigned qualification build/);
  }
});
