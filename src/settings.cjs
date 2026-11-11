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
  const settings = normalize({ ...loadSettings(directory), ...value });
  writeState(path.join(directory, 'desktop-settings.json'), settings);
  return settings;
}
module.exports = { loadSettings, saveSettings };
