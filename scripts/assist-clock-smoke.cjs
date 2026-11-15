'use strict';
// No authority, windows, network, input, clipboard or user profile access.
// The native clock test launches the exact locked Electron executable.
const { app } = require('electron');
const path = require('node:path');
app.setPath('userData', path.resolve(__dirname, '../out/assist-clock-' + process.pid));
const timeout = setTimeout(() => app.exit(1), 10000);
app.whenReady().then(() => {
  console.log('PALISADE_CLOCK=' + JSON.stringify({ electron: process.versions.electron,
    node: process.versions.node, uv: process.versions.uv, stamp: process.hrtime.bigint().toString() }));
  clearTimeout(timeout); app.exit(0);
});
