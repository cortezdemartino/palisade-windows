'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { ensureNotificationBranding, APP_ID } = require('../src/windows-notification-branding.cjs');
test('packaged notification identity registers only Palisade name and an accessible external logo', async () => {
  const calls = [], files = [];
  await ensureNotificationBranding({ packaged: true, platform: 'win32', resourcesPath: 'C:/Palisade/resources', systemRoot: 'C:/Windows',
    checkFile: async file => files.push(file), run: (file, args, options, done) => { calls.push({ file, args, options }); done(null); } });
  assert.equal(calls.length, 2); assert.match(files[0], /notification-icon.png$/); assert.doesNotMatch(files[0], /app.asar/);
  assert.ok(calls.every(call => call.args[1] === 'HKCU\\Software\\Classes\\AppUserModelId\\' + APP_ID));
  assert.equal(calls[0].args[3], 'DisplayName'); assert.equal(calls[0].args[7], 'Palisade');
  assert.equal(calls[1].args[3], 'IconUri'); assert.equal(calls[1].args[7], files[0]);
  assert.ok(calls.every(call => call.options.shell === false && call.options.windowsHide && call.options.timeout === 10000));
});
test('development runtimes never overwrite production branding and registration failures are surfaced', async () => {
  await ensureNotificationBranding({ packaged: false, platform: 'win32', run: () => assert.fail('No development registration') });
  await assert.rejects(ensureNotificationBranding({ packaged: true, platform: 'win32', resourcesPath: 'C:/Palisade/resources', checkFile: async () => {}, run: (_file, _args, _options, done) => done(new Error('denied')) }), /denied/);
});
