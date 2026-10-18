'use strict';
const { ipcMain, nativeImage } = require('electron');
const path = require('node:path');
const { isPalisadeUrl } = require('./security-policy.cjs');
function installChatNotifications(window, contents, { notify, openWindow, now = Date.now }) {
  const icon = nativeImage.createFromPath(path.join(__dirname, '..', 'assets', 'icon.ico'));
  const unreadIcon = nativeImage.createFromPath(path.join(__dirname, '..', 'assets', 'icon-unread.png'));
  let previous = null, lastNotice = -Infinity;
  function update(event, value) {
    if (window.isDestroyed() || event.sender !== contents || event.senderFrame !== contents.mainFrame ||
        !isPalisadeUrl(event.senderFrame?.url) || !isPalisadeUrl(contents.getURL()) ||
        !value || Object.keys(value).sort().join(',') !== 'baseline,count' ||
        !Number.isInteger(value.count) || value.count < 0 || value.count > 9999 || typeof value.baseline !== 'boolean') return;
    window.setIcon(value.count ? unreadIcon : icon);
    if (!value.baseline && previous !== null && value.count > previous && now() - lastNotice >= 5000) {
      lastNotice = now();
      try { notify('You have new unread messages in Palisade.', openWindow); } catch { /* A failed toast must not interrupt unread tracking. */ }
    }
    previous = value.count;
  }
  function reset() { previous = null; if (!window.isDestroyed()) window.setIcon(icon); }
  ipcMain.on('palisade:unread-state', update);
  contents.on('did-start-navigation', (_event, _url, inPlace, mainFrame) => { if (mainFrame && !inPlace) reset(); });
  window.once('closed', () => ipcMain.removeListener('palisade:unread-state', update));
}
module.exports = { installChatNotifications };
