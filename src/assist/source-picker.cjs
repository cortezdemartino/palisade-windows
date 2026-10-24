'use strict';
const { BrowserWindow, ipcMain } = require('electron');
const path = require('node:path');
const { installControlsProtocol } = require('../desktop-window.cjs');
const URL = 'palisade-controls://app/assist-picker.html';
function chooseSource(sources, { parent, signal }) {
  if (signal.aborted || !sources.length) return Promise.resolve(null);
  return new Promise(resolve => {
    const picker = new BrowserWindow({ parent, modal: true, width: 760, height: 580, minWidth: 480, minHeight: 420,
      show: false, title: 'Share a screen or window', autoHideMenuBar: true, backgroundColor: '#141a23',
      webPreferences: { partition: 'palisade-assist-picker', sandbox: true, contextIsolation: true,
        nodeIntegration: false, devTools: false, webSecurity: true, webviewTag: false,
        preload: path.join(__dirname, 'picker-preload.cjs') } });
    const contents = picker.webContents;
    installControlsProtocol(contents.session);
    let settled = false;
    const finish = id => {
      if (settled) return; settled = true;
      signal.removeEventListener('abort', cancel); ipcMain.removeListener('palisade:assist-pick', receive);
      if (!picker.isDestroyed()) picker.destroy();
      resolve(id);
    };
    const cancel = () => finish(null);
    const receive = (event, value) => {
      if (event.sender !== contents || event.senderFrame !== contents.mainFrame ||
          event.senderFrame?.url !== URL || contents.getURL() !== URL) return;
      if (value === null) { cancel(); return; }
      if (typeof value === 'string' && sources.some(source => source.id === value)) finish(value);
    };
    ipcMain.on('palisade:assist-pick', receive); signal.addEventListener('abort', cancel, { once: true });
    contents.setWindowOpenHandler(() => ({ action: 'deny' }));
    contents.on('will-navigate', event => event.preventDefault());
    contents.on('will-redirect', event => event.preventDefault());
    contents.session.setPermissionRequestHandler((_wc, _permission, done) => done(false));
    contents.session.setPermissionCheckHandler(() => false);
    contents.session.setDevicePermissionHandler(() => false);
    contents.once('did-finish-load', () => {
      if (settled) return;
      contents.send('palisade:assist-sources', sources.map(source => ({ id: source.id,
        name: source.name.slice(0, 160), type: source.id.startsWith('screen:') ? 'Display' : 'Window',
        thumbnail: source.thumbnail.isEmpty() ? '' : source.thumbnail.toDataURL() })));
      picker.show();
    });
    picker.once('closed', cancel); contents.once('did-fail-load', cancel);
    void picker.loadURL(URL).catch(cancel);
  });
}
module.exports = { chooseSource };
