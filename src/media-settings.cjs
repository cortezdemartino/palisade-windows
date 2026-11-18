'use strict';
const path = require('node:path');
const { readState, writeState } = require('./updates/state-file.cjs');
const ACTIONS = Object.freeze(['mute', 'deafen', 'camera', 'camera-privacy', 'push-to-talk', 'volume-up', 'volume-down', 'device-settings']);
const MODIFIERS = ['Control', 'Alt', 'Shift'];
// Windows/Assist/application shortcuts are not available to this recorder.
const RESERVED = new Set(['Alt+F4', 'Alt+Tab', 'Control+Alt+Delete', 'Control+Shift+Escape',
  'Control+R', 'Control+Shift+R', 'Control+W', 'Control+Q', 'Control+Shift+I', 'Control+Shift+F12', 'Control+Alt+Shift+F10', 'F5', 'F11', 'F12']);
function userError(error) {
  return new Set(['Choose a supported keyboard shortcut.', 'Use Control or Alt with a supported key.',
    'That shortcut is reserved by Windows or Palisade.', 'Choose a shortcut that does not conflict with AltGr typing.',
    'That shortcut is already assigned.', 'Media settings changed. Try again.',
    'Media settings changed on disk. Reopen Palisade before saving.',
    'Saved media settings were removed. Reopen Palisade before saving.',
    'Saved media settings could not be read. The original file has been preserved.']).has(error?.message)
    ? error.message : 'Media settings could not be saved. Your previous preferences are unchanged.';
}
function accelerator(value) {
  if (typeof value !== 'string' || value.length > 64) throw Error('Choose a supported keyboard shortcut.');
  if (value === '') return '';
  const parts = value.split('+'), key = parts.pop();
  if (!/^(?:[A-Z0-9]|F(?:[1-9]|1[0-2])|Up|Down|Left|Right|Space)$/.test(key) ||
      parts.some(part => !MODIFIERS.includes(part)) || new Set(parts).size !== parts.length ||
      !parts.some(part => part === 'Control' || part === 'Alt')) throw Error('Use Control or Alt with a supported key.');
  const normalized = [...MODIFIERS.filter(part => parts.includes(part)), key].join('+');
  if (parts.includes('Control') && parts.includes('Alt') && /^[A-Z0-9]$/.test(key)) throw Error('Choose a shortcut that does not conflict with AltGr typing.');
  if (RESERVED.has(normalized)) throw Error('That shortcut is reserved by Windows or Palisade.');
  return normalized;
}
function defaults() {
  return { version: 1, revision: 0, cameraBlocked: true, backgroundShortcuts: false,
    keybinds: Object.fromEntries(ACTIONS.map(action => [action, { accelerator: action === 'device-settings' ? 'Control+Shift+D' : '', enabled: action === 'device-settings' }])) };
}
function validate(value) {
  if (!value || Object.keys(value).sort().join(',') !== 'backgroundShortcuts,cameraBlocked,keybinds,revision,version' ||
      value.version !== 1 || !Number.isSafeInteger(value.revision) || value.revision < 0 ||
      typeof value.cameraBlocked !== 'boolean' || typeof value.backgroundShortcuts !== 'boolean' ||
      !value.keybinds || Object.keys(value.keybinds).sort().join(',') !== [...ACTIONS].sort().join(',')) throw Error('The saved media settings need recovery.');
  const result = { ...value, keybinds: {} }, used = new Set();
  for (const action of ACTIONS) {
    const binding = value.keybinds[action];
    if (!binding || Object.keys(binding).sort().join(',') !== 'accelerator,enabled' || typeof binding.enabled !== 'boolean') throw Error('Invalid media shortcut.');
    const key = accelerator(binding.accelerator);
    if (key && used.has(key)) throw Error('That shortcut is already assigned.');
    if (key) used.add(key);
    result.keybinds[action] = { accelerator: key, enabled: binding.enabled };
  }
  return result;
}
function openMediaSettings(directory) {
  const file = path.join(directory, 'media-device-policy.json');
  let current, error = '';
  try { current = validate(readState(file, 16384)); }
  catch (reason) { current = defaults(); if (reason.code !== 'ENOENT') error = 'Saved media settings could not be read. The original file has been preserved.'; }
  const snapshot = () => structuredClone(current);
  return Object.freeze({ snapshot, error: () => error,
    update(patch, expectedRevision) {
      if (error) throw Error(error);
      if (expectedRevision !== current.revision) throw Error('Media settings changed. Try again.');
      if (!patch || Object.keys(patch).some(key => !['backgroundShortcuts', 'keybinds'].includes(key))) throw Error('Unsupported media preference.');
      if (current.revision === Number.MAX_SAFE_INTEGER) throw Error('Media settings need recovery.');
      // Camera unblock requires the separately negotiated track-stop/capture
      // policy adapter; it cannot be enabled through a generic preference patch.
      const next = validate({ ...current, ...patch, revision: current.revision + 1 });
      try {
        const disk = validate(readState(file, 16384));
        if (JSON.stringify(disk) !== JSON.stringify(current)) throw Error('Media settings changed on disk. Reopen Palisade before saving.');
      } catch (reason) { if (reason.code !== 'ENOENT') throw reason; if (current.revision !== 0) throw Error('Saved media settings were removed. Reopen Palisade before saving.'); }
      writeState(file, next); current = next; return snapshot();
    },
  });
}
module.exports = { ACTIONS, accelerator, defaults, validate, openMediaSettings, userError };
