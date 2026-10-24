'use strict';
const { ipcMain } = require('electron');
const { isPalisadeUrl } = require('../security-policy.cjs');
const PROTOCOL = 'palisade.assist.bridge';
const REQUEST = 'palisade:assist-capabilities';
const CHANGED = 'palisade:assist-capabilities-changed';
function validRequest(value) {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value) &&
    [Object.prototype, null].includes(Object.getPrototypeOf(value)) &&
    Object.keys(value).sort().join(',') === 'versions' && Array.isArray(value.versions) &&
    value.versions.length > 0 && value.versions.length <= 8 &&
    value.versions.every(v => Number.isInteger(v) && v > 0 && v <= 65535));
}
function installAssistBridge(contents, { desktopVersion, now = Date.now, captureAvailable = () => false }) {
  if (!/^\d{1,4}\.\d{1,4}\.\d{1,6}$/.test(desktopVersion)) throw new Error('Invalid desktop version');
  let stopped = false, navigating = false, since = now(), count = 0;
  const report = () => ({ protocol: PROTOCOL, version: 1, desktopVersion,
    capture: captureAvailable() ? 'desktop-picker' : 'unavailable', nativeEndpoint: 'absent',
    localRevocation: false, emergencyStop: false });
  const trusted = event => !stopped && !navigating && !contents.isDestroyed() && event.sender === contents &&
    event.senderFrame === contents.mainFrame && isPalisadeUrl(contents.getURL()) && isPalisadeUrl(event.senderFrame?.url);
  ipcMain.handle(REQUEST, (event, request) => {
    if (!trusted(event) || !validRequest(request)) throw new Error('Assist request refused');
    const time = now(); if (time < since || time - since >= 1000) { since = time; count = 0; }
    if (++count > 12) throw new Error('Assist rate limit');
    // v1 handoff specifies a highest-supported report when no offered version matches.
    return report();
  });
  function changed() {
    if (trusted({ sender: contents, senderFrame: contents.mainFrame })) contents.send(CHANGED, report());
  }
  function reset(_event, _url, inPlace, mainFrame) {
    if (mainFrame && !inPlace) { navigating = true; since = now(); count = 0; }
  }
  function ready() { navigating = false; }
  function dispose() {
    if (stopped) return; stopped = true;
    ipcMain.removeHandler(REQUEST); contents.removeListener('did-start-navigation', reset);
    contents.removeListener('did-finish-load', ready);
    contents.removeListener('destroyed', dispose);
  }
  contents.on('did-start-navigation', reset); contents.on('did-finish-load', ready); contents.once('destroyed', dispose);
  return { report, changed, dispose };
}
module.exports = { installAssistBridge, validRequest, PROTOCOL, REQUEST, CHANGED };
