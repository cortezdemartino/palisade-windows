'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { EventEmitter } = require('node:events');
test('unread state accepts only the trusted main frame, shows the ring only in the system tray, clears on read, and throttles generic notices', () => {
  const ipcMain = new EventEmitter(), notices = [], icons = [], trayIcons = [], flashes = [];
  let focused = false;
  let clock = 0;
  const window = Object.assign(new EventEmitter(), { isDestroyed: () => false, setIcon: icon => icons.push(icon), isFocused: () => focused, flashFrame: value => flashes.push(value) });
  const contents = Object.assign(new EventEmitter(), { mainFrame: { url: 'https://palisade.onyxops.org/' }, getURL: () => contents.mainFrame.url });
  const context = { module: { exports: {} }, __dirname: path.join(__dirname, '../src'), require: name => name === 'electron' ? { ipcMain, nativeImage: { createFromPath: filename => path.basename(filename) } } : name === './security-policy.cjs' ? require('../src/security-policy.cjs') : require(name) };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/chat-notifications.cjs'), 'utf8'), context);
  context.module.exports.installChatNotifications(window, contents, { notify: text => notices.push(text), openWindow() {}, now: () => clock, setUnreadIndicator: icon => trayIcons.push(icon) });
  const emit = (value, sender = contents, senderFrame = contents.mainFrame) => ipcMain.emit('palisade:unread-state', { sender, senderFrame }, value);
  emit({ count: 10, baseline: true }); assert.equal(trayIcons.at(-1), 'icon-unread.png'); assert.equal(icons.at(-1), 'icon.ico'); assert.equal(notices.length, 0);
  emit({ count: 11, baseline: false }); assert.equal(notices.length, 1); assert.match(notices[0], /unread messages/);
  emit({ count: 12, baseline: false }); assert.equal(notices.length, 1);
  assert.equal(flashes.filter(Boolean).length, 2);
  window.emit('focus'); assert.equal(flashes.at(-1), false);
  focused = true; emit({ count: 13, baseline: false }); assert.equal(flashes.at(-1), false);
  focused = false;
  emit({ count: 0, baseline: false }); assert.equal(icons.at(-1), 'icon.ico'); assert.equal(trayIcons.at(-1), 'icon.ico'); assert.equal(flashes.at(-1), false);
  const total = icons.length;
  emit({ count: 2, baseline: false }, {}); emit({ count: 2, baseline: false }, contents, {});
  emit({ count: -1, baseline: false }); emit({ count: 10000, baseline: false }); emit({ count: '2', baseline: false });
  assert.equal(icons.length, total);
  contents.mainFrame.url = 'https://example.com/'; emit({ count: 2, baseline: false }); assert.equal(icons.length, total);
  contents.mainFrame.url = 'https://palisade.onyxops.org/';
  clock = 6000; emit({ count: 1, baseline: false }); assert.equal(notices.length, 2);
  contents.emit('did-start-navigation', {}, 'https://palisade.onyxops.org/', false, true); assert.equal(icons.at(-1), 'icon.ico');
  assert.equal(trayIcons.at(-1), 'icon.ico');
  assert.ok(icons.every(icon => icon === 'icon.ico'));
  emit({ count: 20, baseline: true }); assert.equal(notices.length, 2);
  window.emit('closed'); assert.equal(ipcMain.listenerCount('palisade:unread-state'), 0);
});
test('isolated unread observer ignores preview, channel duplicates and invitations without exposing a page bridge', () => {
  const sent = []; let ready, mutation, clock = 0, live = false;
  const buttons = [ { label: 'Direct messages, 2 unread' }, { label: 'Space, 3 mentions, 5 unread' }, { label: 'Invitation, 99 unread', invite: true } ];
  const rail = { querySelector: () => live ? {} : null, querySelectorAll: () => buttons.map(button => ({ classList: { contains: () => button.invite === true }, getAttribute: () => button.label, querySelector: () => button.label.includes(' unread') ? {} : null })) };
  const context = { require: () => ({ ipcRenderer: { send: (channel, value) => { assert.equal(channel, 'palisade:unread-state'); sent.push(value); } } }),
    window: { addEventListener: (name, callback) => { if (name === 'DOMContentLoaded') ready = callback; } },
    document: { documentElement: {}, querySelector: () => rail }, Date: { now: () => clock },
    MutationObserver: class { constructor(callback) { mutation = callback; } observe() {} disconnect() {} },
    setTimeout: callback => { callback(); return 1; }, clearTimeout() {},
  };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../src/chat-notifications-preload.cjs'), 'utf8'), context);
  ready(); assert.equal(sent.length, 0);
  live = true; mutation(); assert.equal(sent.at(-1).count, 7); assert.equal(sent.at(-1).baseline, true);
  clock = 1000; buttons[0].label = 'Direct messages, 3 unread'; mutation(); assert.equal(sent.at(-1).baseline, true);
  clock = 6000; buttons[0].label = 'Direct messages, 4 unread'; mutation(); assert.equal(sent.at(-1).count, 9); assert.equal(sent.at(-1).baseline, false);
  mutation(); assert.equal(sent.length, 3);
  live = false; mutation(); assert.equal(sent.at(-1).count, 0); assert.equal(sent.at(-1).baseline, true);
  assert.equal(context.window.desktopControls, undefined);
  assert.equal(Object.keys(sent[0]).sort().join(','), 'baseline,count');
});
