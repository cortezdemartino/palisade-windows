'use strict';
const { app, BrowserWindow } = require('electron');
const fs = require('node:fs');
const path = require('node:path');
app.disableHardwareAcceleration();
app.whenReady().then(async () => {
  const window = new BrowserWindow({ width: 256, height: 256, show: false, frame: false, transparent: true,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false, javascript: false, offscreen: true } });
  for (const name of ['normal', 'unread']) {
    const svg = fs.readFileSync(path.join(__dirname, '../assets/icon-' + name + '.svg'), 'utf8');
    await window.loadURL('data:text/html;charset=utf-8,' + encodeURIComponent('<style>html,body{margin:0;background:transparent;overflow:hidden}svg{width:256px;height:256px}</style>' + svg));
    await new Promise(resolve => setTimeout(resolve, 300));
    const image = await window.webContents.capturePage();
    fs.writeFileSync(path.join(__dirname, '../assets/icon-' + name + '.png'), image.toPNG());
    if (name === 'normal') {
      const sizes = [16, 24, 32, 48, 64, 128, 256];
      const images = sizes.map(size => image.resize({ width: size, height: size }).toPNG());
      const header = Buffer.alloc(6 + sizes.length * 16); header.writeUInt16LE(1, 2); header.writeUInt16LE(sizes.length, 4);
      let offset = header.length;
      sizes.forEach((size, i) => { const entry = 6 + i * 16; header[entry] = size % 256; header[entry + 1] = size % 256; header.writeUInt16LE(1, entry + 4); header.writeUInt16LE(32, entry + 6); header.writeUInt32LE(images[i].length, entry + 8); header.writeUInt32LE(offset, entry + 12); offset += images[i].length; });
      fs.writeFileSync(path.join(__dirname, '../assets/icon.ico'), Buffer.concat([header, ...images]));
    }
  }
  window.destroy(); app.quit();
}).catch(error => { console.error(error); app.exit(1); });
