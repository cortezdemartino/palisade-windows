'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs/promises');
const path = require('node:path');
const { EventEmitter } = require('node:events');
async function harness({ response = 0, fail = false } = {}) {
  const calls = [], prompts = [], statuses = [];
  let quit = 0;
  const app = { isPackaged: true, getPath: () => 'C:\\Temp', quit: () => quit++ };
  const fakeFs = { constants: { COPYFILE_EXCL: 1 } };
  const promises = {
    mkdtemp: async prefix => { calls.push(['mkdir', prefix]); return 'C:\\Temp\\manager-123'; },
    copyFile: async (...args) => { calls.push(['copy', ...args]); if (fail) throw new Error('missing manager'); },
    rm: async (...args) => { calls.push(['cleanup', ...args]); }
  };
  const context = { module: { exports: {} }, process: { platform: 'win32', resourcesPath: 'C:\\Apps\\Palisade\\resources', execPath: 'C:\\Apps\\Palisade\\Palisade.exe' },
    require: name => {
      if (name === 'electron') return { app, dialog: { showMessageBox: async (...args) => { prompts.push(args.at(-1)); return { response }; } } };
      if (name === 'node:fs/promises') return promises;
      if (name === 'node:fs') return fakeFs;
      if (name === 'node:path') return path.win32;
      if (name === 'node:child_process') return { spawn: (...args) => {
        calls.push(['spawn', ...args]); const child = new EventEmitter(); child.unref = () => calls.push(['unref']);
        process.nextTick(() => child.emit('spawn')); return child;
      } };
      throw new Error('Unexpected dependency ' + name);
    } };
  vm.runInNewContext(await fs.readFile(path.join(__dirname, '../src/updates/updater.cjs'), 'utf8'), context);
  const controller = context.module.exports.createUpdateController(() => null, value => statuses.push(value));
  return { controller, calls, prompts, statuses, quit: () => quit };
}
test('client delegates updates to a copied manager outside its installation and exits after launch', async () => {
  const h = await harness(); await h.controller.check(true);
  const copy = h.calls.find(x => x[0] === 'copy');
  assert.equal(copy[1], 'C:\\Apps\\Palisade\\resources\\manager\\Palisade-Manager.exe');
  assert.equal(copy[2], 'C:\\Temp\\manager-123\\Palisade-Manager.exe');
  const launch = h.calls.find(x => x[0] === 'spawn');
  assert.equal(launch[1], copy[2]);
  assert.deepEqual(Array.from(launch[2]), ['--installation', 'C:\\Apps\\Palisade']);
  assert.equal(launch[3].shell, false); assert.equal(h.quit(), 1);
});
test('cancel keeps the client running and creates no installer or manager files', async () => {
  const h = await harness({ response: 1 }); await h.controller.check(true);
  assert.equal(h.quit(), 0); assert.equal(h.calls.length, 0);
});
test('missing manager keeps client running and reports recovery without executing setup', async () => {
  const h = await harness({ fail: true }); await h.controller.check(true);
  assert.equal(h.quit(), 0); assert.equal(h.calls.some(x => x[0] === 'spawn'), false);
  assert.equal(h.prompts.at(-1).type, 'warning'); assert.equal(h.calls.at(-1)[0], 'cleanup');
});
test('client startup schedules no independent checks or downloads', async () => {
  const h = await harness(); h.controller.start(); await h.controller.check();
  assert.equal(h.calls.length, 0); assert.equal(h.prompts.length, 0);
  assert.match(h.statuses[0], /managed by Palisade Manager/);
});
