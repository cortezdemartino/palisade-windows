'use strict';
const test = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const { defaults, accelerator, openMediaSettings, userError } = require('../src/media-settings.cjs');
const { createMediaShortcuts } = require('../src/media-shortcuts.cjs');
test('media preferences persist with revision checks, independent stores and corrupt-file preservation', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'palisade-media-'));
  try {
    const store = openMediaSettings(dir); assert.equal(store.snapshot().cameraBlocked, true);
    assert.equal(store.snapshot().backgroundShortcuts, false);
    fs.writeFileSync(path.join(dir, 'desktop-settings.json'), '{"autoCheck":false}');
    const next = store.update({ backgroundShortcuts: true }, 0);
    assert.equal(next.revision, 1); assert.equal(openMediaSettings(dir).snapshot().backgroundShortcuts, true);
    assert.throws(() => store.update({ backgroundShortcuts: false }, 0), /changed/);
    assert.throws(() => store.update({ cameraBlocked: false }, 1), /Unsupported/);
    assert.equal(fs.readFileSync(path.join(dir, 'desktop-settings.json'), 'utf8'), '{"autoCheck":false}');
    next.keybinds.mute.accelerator = 'Control+M'; assert.equal(store.snapshot().keybinds.mute.accelerator, '');
    const file = path.join(dir, 'media-device-policy.json');
    fs.writeFileSync(file, '{broken');
    assert.throws(() => store.update({ backgroundShortcuts: false }, 1));
    const corrupt = openMediaSettings(dir); assert(corrupt.error());
    assert.throws(() => corrupt.update({}, 0), /preserved/); assert.equal(fs.readFileSync(file, 'utf8'), '{broken');
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
test('shortcut validation rejects reserved, duplicate, AltGr and plain typing combinations', () => {
  assert.equal(accelerator('Shift+Control+D'), 'Control+Shift+D');
  assert.doesNotMatch(userError(Error('EACCES C:\\private\\profile.json')), /private|EACCES/);
  for (const key of ['D', 'Shift+A', 'Meta+D', 'Alt+F4', 'Alt+Tab', 'Control+Alt+A', 'Control+R', 'Control+Shift+F12', 'Control+Alt+Shift+F10', 'Control+Control+D']) assert.throws(() => accelerator(key), key);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'palisade-media-'));
  try {
    const s = openMediaSettings(dir), keys = defaults().keybinds;
    keys.mute = { accelerator: 'Control+Shift+D', enabled: true };
    assert.throws(() => s.update({ keybinds: keys }, 0), /already assigned/);
    assert.equal(fs.existsSync(path.join(dir, 'media-device-policy.json')), false);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
test('native shortcut handling admits supported actions only, avoids repeat and cleans owned global registrations', () => {
  const callbacks = new Map(), calls = [], removed = []; let foreground = true;
  const h = createMediaShortcuts({ globalShortcut: { register(key, cb) { callbacks.set(key, cb); return true; }, unregister(key) { callbacks.delete(key); removed.push(key); } },
    focused: () => foreground, available: action => action === 'device-settings', dispatch: action => calls.push(action) });
  const settings = defaults(); settings.keybinds.mute = { accelerator: 'Control+M', enabled: true }; h.configure(settings); assert.equal(callbacks.size, 0);
  let prevented = 0; const event = { preventDefault() { prevented++; } }, key = { type: 'keyDown', code: 'KeyD', key: 'd', control: true, shift: true };
  h.input(event, key); h.input(event, key); h.input(event, { ...key, isAutoRepeat: true }); assert.equal(calls.length, 1);
  h.blur(); h.input(event, key); assert.equal(calls.length, 2); assert.equal(prevented, 2);
  settings.backgroundShortcuts = true; h.configure(settings); assert.equal(callbacks.size, 0, 'globals cannot steal keys from the focused app or recorder');
  foreground = false; h.blur(); assert.equal(callbacks.size, 1);
  const callback = callbacks.get('Control+Shift+D'); callback(); assert.equal(calls.length, 3);
  foreground = true; h.focus(); assert.equal(callbacks.size, 0);
  callback(); assert.equal(calls.length, 3);
  h.configure(defaults()); assert.equal(callbacks.size, 0); assert(removed.length);
  callback(); assert.equal(calls.length, 3, 'queued callbacks from removed bindings are fenced');
  h.stop(); callback(); assert.equal(calls.length, 3);
});
test('failed global registration is reported without claiming activation or registering global PTT', () => {
  const tried = [], status = []; let focused = false; const h = createMediaShortcuts({ globalShortcut: { register(k) { tried.push(k); return false; }, unregister() {} }, focused: () => focused,
    available: () => true, dispatch() { assert.fail('unregistered action'); }, onStatus: failures => status.push(failures) });
  const s = defaults(); s.backgroundShortcuts = true; s.keybinds['push-to-talk'] = { accelerator: 'Control+T', enabled: true }; h.configure(s);
  assert.deepEqual(tried, ['Control+Shift+D']); assert.deepEqual(h.failures(), ['device-settings']);
  focused = true; h.focus(); assert.deepEqual(h.failures(), ['device-settings'], 'registration failure remains visible when the user returns');
  h.configure(defaults()); assert.deepEqual(status.at(-1), []); h.stop();
});
