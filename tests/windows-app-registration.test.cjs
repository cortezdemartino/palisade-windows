'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { appDetails, ensureAppRegistration } = require('../src/windows-app-registration.cjs');
const exe = 'C:\\Program Files\\Palisade\\Palisade.exe';
function fixture(previous) {
  let link = previous;
  const writes = [], registry = [];
  const options = { packaged: true, platform: 'win32', exe, appData: 'C:\\Users\\Fixture\\AppData\\Roaming',
    checkFile: async () => {}, makeDirectory: async () => {},
    shell: { readShortcutLink() { if (!link) throw Error('missing'); return link; },
      writeShortcutLink(file, operation, value) { writes.push({ file, operation, value }); link = value; return true; } },
    run(file, args, config, done) { registry.push({ file, args, config }); done(null); } };
  return { options, writes, registry };
}
test('only packaged Palisade gets explicit window relaunch identity', async () => {
  assert.equal(appDetails({ packaged: false }), null);
  assert.equal(appDetails({ packaged: true, platform: 'linux' }), null);
  for (const target of ['Palisade.exe', 'C:\\Electron.exe', 'C:\\Palisade-Assist.exe', 'C:\\bad"name\\Palisade.exe']) {
    assert.throws(() => appDetails({ packaged: true, platform: 'win32', exe: target }));
  }
  const details = appDetails({ packaged: true, platform: 'win32', exe });
  assert.equal(details.appId, 'org.onyxops.palisade');
  assert.equal(details.relaunchCommand, '"' + exe + '"');
  assert.equal(details.relaunchDisplayName, 'Palisade');
  const f = fixture();
  await ensureAppRegistration({ ...f.options, packaged: false });
  assert.equal(f.writes.length, 0); assert.equal(f.registry.length, 0);
});
test('repair creates a complete Start entry and App Paths without shell invocation or pinning', async () => {
  const f = fixture();
  const result = await ensureAppRegistration(f.options);
  assert.match(result.shortcut, /Microsoft\\Windows\\Start Menu\\Programs\\Palisade\.lnk$/);
  assert.equal(f.writes.length, 1); assert.equal(f.writes[0].operation, 'create');
  assert.equal(f.writes[0].value.target, exe); assert.equal(f.writes[0].value.icon, exe);
  assert.equal(f.writes[0].value.args, ''); assert.equal(f.writes[0].value.appUserModelId, result.appId);
  assert.equal(f.registry.length, 2);
  for (const call of f.registry) {
    assert.match(call.args[1], /^HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\Palisade\.exe$/);
    assert.equal(call.config.shell, false); assert.equal(call.config.windowsHide, true);
    assert.equal(call.config.timeout, 10000);
  }
  await ensureAppRegistration(f.options);
  assert.equal(f.writes.length, 1, 'correct shortcuts remain untouched');
});
test('older own shortcuts are retargeted, while another app is preserved', async () => {
  const f = fixture({ appUserModelId: 'org.onyxops.palisade', target: 'C:\\Old\\Palisade.exe', args: '--old' });
  await ensureAppRegistration(f.options);
  assert.equal(f.writes[0].operation, 'replace'); assert.equal(f.writes[0].value.args, '');
  const other = fixture({ appUserModelId: 'other.app', target: 'C:\\Other.exe' });
  await assert.rejects(ensureAppRegistration(other.options), /another application/);
  assert.equal(other.writes.length, 0); assert.equal(other.registry.length, 0);
});
test('failed shortcut writes, mismatched readback and registry errors are reported', async () => {
  const failed = fixture(); failed.options.shell.writeShortcutLink = () => false;
  await assert.rejects(ensureAppRegistration(failed.options), /could not register/);
  assert.equal(failed.registry.length, 0);
  const corrupt = fixture({ target: exe }); corrupt.options.shell.writeShortcutLink = () => true;
  await assert.rejects(ensureAppRegistration(corrupt.options), /did not match/);
  const denied = fixture(); denied.options.run = (_file, _args, _options, done) => done(Error('denied'));
  await assert.rejects(ensureAppRegistration(denied.options), /denied/);
});
test('only a legacy Electron shortcut incorrectly claiming Palisade identity is removed', async () => {
  for (const appUserModelId of ['org.onyxops.palisade', 'com.electron.app']) {
    const f = fixture();
    const read = f.options.shell.readShortcutLink;
    f.options.shell.readShortcutLink = file => file.endsWith('\\Electron.lnk')
      ? { appUserModelId, target: 'C:\\Development\\electron.exe' } : read(file);
    const removed = [];
    f.options.deleteFile = async file => removed.push(file);
    await ensureAppRegistration(f.options);
    assert.equal(removed.length, appUserModelId === 'org.onyxops.palisade' ? 1 : 0);
    if (removed.length) assert.match(removed[0], /Start Menu\\Programs\\Electron\.lnk$/);
  }
});
test('a correct all-users shortcut is reused without an extra per-user entry', async () => {
  const initial = fixture(); await ensureAppRegistration(initial.options);
  const wanted = initial.writes[0].value;
  const f = fixture(); f.options.commonAppData = 'C:\\ProgramData';
  f.options.shell.readShortcutLink = file => {
    if (file.startsWith('C:\\ProgramData\\')) return { ...wanted, description: 'Installed app' };
    throw Error('missing');
  };
  f.options.shell.writeShortcutLink = () => assert.fail('Do not replace an all-users entry');
  const result = await ensureAppRegistration(f.options);
  assert.match(result.shortcut, /^C:\\ProgramData\\/);
  assert.equal(f.writes.length, 0); assert.equal(f.registry.length, 2);
});
