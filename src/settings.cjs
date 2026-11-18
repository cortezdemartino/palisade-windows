'use strict';
const path = require('node:path');
const { readState, writeState } = require('./updates/state-file.cjs');

const DEFAULTS = Object.freeze({
  keepRunningInTray: true, autoCheck: true, autoDownload: true, installOnQuit: false, snoozeUntil: 0,
});
function normalize(value) {
  const result = { ...DEFAULTS };
  for (const name of ['keepRunningInTray', 'autoCheck', 'autoDownload', 'installOnQuit']) {
    if (typeof value?.[name] === 'boolean') result[name] = value[name];
  }
  if (Number.isSafeInteger(value?.snoozeUntil) && value.snoozeUntil > Date.now() &&
      value.snoozeUntil <= Date.now() + 24 * 60 * 60 * 1000) result.snoozeUntil = value.snoozeUntil;
  return result;
}
function loadSettings(directory) {
  try {
    return normalize(readState(path.join(directory, 'desktop-settings.json'), 4096));
  } catch { return { ...DEFAULTS }; }
}
function saveSettings(directory, value) {
  // Reading defaults is safe for startup, but writing them over an unreadable
  // file would destroy preferences the user may still be able to recover.
  let previous;
  try {
    previous = readState(path.join(directory, 'desktop-settings.json'), 4096);
    if (!previous || typeof previous !== 'object' || Array.isArray(previous)) throw Error('Invalid settings');
  } catch (error) {
    if (error.code !== 'ENOENT') throw Error('Your saved settings could not be read. They have been preserved.');
    previous = DEFAULTS;
  }
  const settings = normalize({ ...previous, ...value });
  writeState(path.join(directory, 'desktop-settings.json'), settings);
  return settings;
}
module.exports = { loadSettings, saveSettings };
