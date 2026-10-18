'use strict';
const path = require('node:path');
const { execFile } = require('node:child_process');
const { access } = require('node:fs/promises');
const APP_ID = 'org.onyxops.palisade';
async function ensureNotificationBranding({ packaged, platform = process.platform, resourcesPath = process.resourcesPath,
  systemRoot = process.env.SystemRoot || 'C:\\Windows', run = execFile, checkFile = access }) {
  if (platform !== 'win32' || !packaged) return;
  const icon = path.join(resourcesPath, 'notification-icon.png');
  await checkFile(icon);
  const executable = path.join(systemRoot, 'System32', 'reg.exe');
  const key = 'HKCU\\Software\\Classes\\AppUserModelId\\' + APP_ID;
  for (const [name, value] of [['DisplayName', 'Palisade'], ['IconUri', icon]]) {
    await new Promise((resolve, reject) => run(executable,
      ['add', key, '/v', name, '/t', 'REG_SZ', '/d', value, '/f'],
      { windowsHide: true, shell: false, timeout: 10000 }, error => error ? reject(error) : resolve()));
  }
}
module.exports = { APP_ID, ensureNotificationBranding };
