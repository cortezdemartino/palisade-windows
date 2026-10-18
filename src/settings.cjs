'use strict';
const fs = require('node:fs');
const path = require('node:path');

const DEFAULTS = Object.freeze({ keepRunningInTray: true });
function loadSettings(directory) {
  try {
    const value = JSON.parse(fs.readFileSync(path.join(directory, 'desktop-settings.json'), 'utf8'));
    return { keepRunningInTray: typeof value?.keepRunningInTray === 'boolean' ? value.keepRunningInTray : DEFAULTS.keepRunningInTray };
  } catch { return { ...DEFAULTS }; }
}
function saveSettings(directory, value) {
  fs.mkdirSync(directory, { recursive: true });
  const filename = path.join(directory, 'desktop-settings.json');
  fs.writeFileSync(`${filename}.tmp`, JSON.stringify({ keepRunningInTray: value.keepRunningInTray === true }));
  fs.renameSync(`${filename}.tmp`, filename);
}
module.exports = { loadSettings, saveSettings };
