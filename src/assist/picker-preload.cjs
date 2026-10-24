'use strict';
const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('sourcePicker', {
  onSources(listener) {
    if (typeof listener !== 'function') return;
    ipcRenderer.once('palisade:assist-sources', (_event, sources) => listener(sources));
  },
  choose(id) { if (typeof id === 'string' && /^(screen|window):\d+:\d+$/.test(id)) ipcRenderer.send('palisade:assist-pick', id); },
  cancel() { ipcRenderer.send('palisade:assist-pick', null); },
});
