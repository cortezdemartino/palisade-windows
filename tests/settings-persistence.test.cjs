'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path'), vm = require('node:vm');
const { loadSettings, saveSettings } = require('../src/settings.cjs');

test('saving a preference preserves corrupt, oversized or invalid existing settings for recovery', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'palisade-settings-recovery-'));
  try {
    const filename = path.join(directory, 'desktop-settings.json');
    for (const raw of ['{broken', 'null', '[]', '"text"', ' '.repeat(4097)]) {
      fs.writeFileSync(filename, raw);
      assert.equal(loadSettings(directory).autoCheck, true);
      assert.throws(() => saveSettings(directory, { autoCheck: false }), /preserved/);
      assert.equal(fs.readFileSync(filename, 'utf8'), raw);
    }
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
test('changing a desktop preference preserves older update choices and session files across reload', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'palisade-ux-settings-'));
  try {
    const file = path.join(directory, 'desktop-settings.json');
    fs.writeFileSync(file, JSON.stringify({ keepRunningInTray:false, autoCheck:false, autoDownload:false, installOnQuit:true, snoozeUntil:Date.now()+60000 }));
    fs.writeFileSync(path.join(directory,'session-fixture'), 'authenticated session fixture');
    const prior = loadSettings(directory);
    saveSettings(directory, { keepRunningInTray:true });
    assert.deepEqual(loadSettings(directory), { ...prior, keepRunningInTray:true });
    saveSettings(directory, { autoCheck:true });
    assert.deepEqual(loadSettings(directory), { ...prior, keepRunningInTray:true, autoCheck:true });
    assert.equal(fs.readFileSync(path.join(directory,'session-fixture'),'utf8'),'authenticated session fixture');
  } finally { fs.rmSync(directory,{recursive:true,force:true}); }
});
test('invalid older preferences fall back safely without rewriting the stored file', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'palisade-ux-settings-'));
  try {
    const file=path.join(directory,'desktop-settings.json'), original=JSON.stringify({ keepRunningInTray:false, autoCheck:'yes', autoDownload:0, installOnQuit:'true' });
    fs.writeFileSync(file,original);
    const settings=loadSettings(directory);
    assert.equal(settings.keepRunningInTray,false); assert.equal(settings.autoCheck,true); assert.equal(settings.autoDownload,true); assert.equal(settings.installOnQuit,false);
    assert.equal(fs.readFileSync(file,'utf8'),original);
  } finally { fs.rmSync(directory,{recursive:true,force:true}); }
});
test('a failed atomic replacement leaves preferences intact and removes only its temporary file', () => {
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),'palisade-ux-settings-'));
  try {
    const file=path.join(directory,'desktop-settings.json'), original='{"keepRunningInTray":false,"autoCheck":false}';
    fs.writeFileSync(file,original);
    const fixtureFs={...fs,renameSync(){throw Object.assign(Error('simulated replace failure'),{code:'EACCES'});}};
    const context={module:{exports:{}},Buffer,require:name=>name==='node:fs'?fixtureFs:require(name)};
    vm.runInNewContext(fs.readFileSync(require.resolve('../src/updates/state-file.cjs'),'utf8'),context);
    assert.throws(()=>context.module.exports.writeState(file,{keepRunningInTray:true}),/replace failure/);
    assert.equal(fs.readFileSync(file,'utf8'),original); assert.deepEqual(fs.readdirSync(directory),['desktop-settings.json']);
    assert.equal(loadSettings(directory).keepRunningInTray,false);
  } finally { fs.rmSync(directory,{recursive:true,force:true}); }
});
