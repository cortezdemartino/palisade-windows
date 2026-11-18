'use strict';
// Real isolated Electron settings UI. All device/update actions use a recorder;
// no OS preferences, installed client, service, input endpoint or installer runs.
const { app, globalShortcut } = require('electron');
app.disableHardwareAcceleration();
app.commandLine.appendSwitch('disable-features', 'CalculateNativeWinOcclusion');
const assert = require('node:assert/strict'), fs = require('node:fs/promises'), path = require('node:path');
const { createDesktopWindow } = require(process.env.PALISADE_WINDOW_SMOKE_ASAR
  ? path.join(process.env.PALISADE_WINDOW_SMOKE_ASAR, 'src/desktop-window.cjs') : '../src/desktop-window.cjs');
app.setPath('userData', path.join(__dirname, '../out/settings-ux-profile'));
app.on('window-all-closed', () => {});
const output = path.join(__dirname, '../out/settings-ux-result.json'), screenshots = path.join(__dirname, '../out/settings-ux');
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
let owner;
const deadline = setTimeout(() => { console.error('Settings UI fixture timed out'); app.exit(1); }, 60000);
app.whenReady().then(async () => {
  const actions = [], cases = [], images = [];
  const mediaRoot = process.env.PALISADE_WINDOW_SMOKE_ASAR
    ? path.join(process.env.PALISADE_WINDOW_SMOKE_ASAR, 'src') : path.join(__dirname, '../src');
  const mediaPrefs = require(path.join(mediaRoot, 'media-settings.cjs'));
  let media = mediaPrefs.defaults();
  let state = { version: '0.3.4', architecture: 'x64', electronVersion: process.versions.electron, buildKind: 'installed',
    keepRunningInTray: true, launchOnLogin: false, loginAvailable: true, updaterAvailable: true, managerAvailable: true,
    updatePhase: 'idle', updateStatus: 'Updates: up to date.', latestVersion: '0.3.4', autoCheck: true, autoDownload: true, installOnQuit: false,
    updateHistory: [{version:'0.3.3',result:'installed',at:Date.parse('2026-11-12T12:00:00Z')},{version:'0.3.4',result:'interrupted',at:Date.parse('2026-11-13T12:00:00Z')}],
    notesStatus: 'Verified notes saved on this PC. Display fixture.', notificationStatus: 'No desktop notifications sent yet.',
    releaseHistory: ['0.3.5','0.3.4'].map(version => ({ version, publishedAt: '2026-11-13T18:00:00.000Z', locale: 'en',
      title: 'Palisade ' + version, summary: 'Desktop improvements and reliability updates.', sections: [{ title: 'Improvements', items: ['A clearer home for Windows preferences.'] }, { title: 'Fixed', items: ['Text only: <img src=x onerror=alert(1)>'] }], compatibility: ['Windows 10 and 11, x64.'] })) };
  const desktop = createDesktopWindow({ partition: 'settings-ux-fixture', onSettings: () => ({ ...state, media }),
    onMediaSettings: request => {
      actions.push(request);
      assert.equal(request.revision, media.revision);
      if (request.op === 'reset') media = { ...mediaPrefs.defaults(), revision: media.revision + 1 };
      if (request.op === 'background') media = { ...media, revision: media.revision + 1, backgroundShortcuts: request.enabled };
      if (request.op === 'binding') media = mediaPrefs.validate({ ...media, revision: media.revision + 1, keybinds: { ...media.keybinds, [request.action]: { accelerator: request.accelerator, enabled: request.enabled } } });
    },
    onDeviceAction: action => {
      actions.push(action);
      if (action === 'tray-off') state = { ...state, keepRunningInTray: false };
      if (action === 'tray-on') state = { ...state, keepRunningInTray: true };
      if (action === 'update-auto-check-off') state = { ...state, autoCheck: false };
      if (action === 'update-snooze') state = { ...state, updatesPaused:true };
      if (action === 'update-resume') state = { ...state, updatesPaused:false };
      if (action === 'copy-diagnostics') state = { ...state, deviceFeedback: 'Troubleshooting information copied.' };
    } });
  owner = desktop.window;
  const controls = owner.contentView.children[1].webContents, remote = desktop.contents;
  const js = code => controls.executeJavaScript(code);
  await new Promise((resolve, reject) => { controls.once('did-finish-load', resolve); controls.once('did-fail-load', (_e,_c,text) => reject(Error(text))); });
  await remote.loadURL('data:text/html,<title>Isolated hosted placeholder</title><p>No accounts or services are loaded.</p>');
  owner.setContentSize(1000, 700); owner.showInactive();
  await fs.mkdir(screenshots, { recursive: true });
  desktop.showSettings(); await pause(100);
  const select = async name => { await js(`document.getElementById('tab-${name}').click()`); await pause(25); };
  const screenshot = async name => { await pause(160); const image = await controls.capturePage(undefined, { stayAwake: true });
    assert(!image.isEmpty()); await fs.writeFile(path.join(screenshots, name + '.png'), image.toPNG()); images.push(name); };
  const layout = async () => {
    const report = await js(`(() => { const dialog=document.getElementById('device-settings'), tabs=document.querySelector('.settings-tabs'), nav=tabs.getClientRects().length?tabs:document.querySelector('.compact-navigation'), content=document.getElementById('settings-content'), close=document.getElementById('dismiss-settings'), footer=document.querySelector('footer');
      const inside=e=>{const r=e.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth+1&&r.top>=0&&r.bottom<=innerHeight+1};
      return { overflow:dialog.scrollWidth>dialog.clientWidth+1||content.scrollWidth>content.clientWidth+1,
        close:inside(close), footer:inside(footer), nav:inside(nav)&&nav.clientHeight>0, contentHeight:content.clientHeight,
        selected:document.querySelectorAll('[role=tab][aria-selected=true]').length,
        panels: [...document.querySelectorAll('[role=tabpanel]')].filter(e=>!e.hidden).length }; })()`);
    assert.equal(report.overflow, false, JSON.stringify(report)); assert(report.close && report.footer && report.nav);
    assert(report.contentHeight >= 50, JSON.stringify(report)); assert.equal(report.selected, 1); assert.equal(report.panels, 1);
  };
  for (const category of ['general','permissions','keybinds','updates','whats-new','about','account']) {
    await select(category); await layout(); await screenshot(category);
    assert.equal(await js(`document.getElementById('section-${category}').hidden`), false);
  }
  cases.push('seven rendered categories and persistent navigation');
  await select('keybinds');
  assert.equal(await js("document.getElementById('device-shortcut').value"), 'Control+Shift+D');
  await js("document.getElementById('clear-shortcut').click()"); await pause(25);
  assert.equal(media.keybinds['device-settings'].enabled, false);
  await js("document.getElementById('record-shortcut').click()");
  await js("document.getElementById('device-shortcut').dispatchEvent(new KeyboardEvent('keydown', { key:'k', code:'KeyK', ctrlKey:true, shiftKey:true, bubbles:true }))"); await pause(25);
  assert.equal(media.keybinds['device-settings'].accelerator, 'Control+Shift+K');
  await js("document.getElementById('background-media-shortcuts').click()"); await pause(25); assert.equal(media.backgroundShortcuts, true);
  await js("document.getElementById('reset-shortcuts').click()"); await pause(25); assert.equal(media.backgroundShortcuts, false);
  cases.push('shortcut recorder, clear, reset and revisioned background preference');
  const shortcuts = require(path.join(mediaRoot, 'media-shortcuts.cjs')).createMediaShortcuts({
    globalShortcut: { register() { throw Error('No real global registration in the UI fixture'); }, unregister() {} },
    focused: () => true, available: action => action === 'device-settings',
    dispatch: () => desktop.showSettings('permissions'),
  });
  shortcuts.configure(mediaPrefs.defaults());
  remote.on('before-input-event', shortcuts.input);
  remote.on('blur', shortcuts.blur);
  await select('general'); remote.focus();
  remote.sendInputEvent({ type: 'keyDown', keyCode: 'D', modifiers: ['control', 'shift'] });
  remote.sendInputEvent({ type: 'keyUp', keyCode: 'D', modifiers: ['control', 'shift'] });
  await pause(75);
  assert.equal(await js("document.getElementById('section-permissions').hidden"), false);
  shortcuts.stop(); remote.removeListener('before-input-event', shortcuts.input); remote.removeListener('blur', shortcuts.blur);
  cases.push('focused WebContents shortcut opens the actual device-permissions section');
  const registration = mediaPrefs.defaults(); registration.backgroundShortcuts = true;
  registration.keybinds['device-settings'].accelerator = 'Control+Alt+Shift+F9';
  let focused = false;
  const nativeShortcuts = require(path.join(mediaRoot, 'media-shortcuts.cjs')).createMediaShortcuts({
    globalShortcut, focused: () => focused, available: action => action === 'device-settings', dispatch: () => {},
  });
  let nativeRegistration = false;
  try {
    nativeShortcuts.configure(registration);
    nativeRegistration = nativeShortcuts.failures().length === 0;
    if (nativeRegistration) {
      assert.equal(globalShortcut.isRegistered('Control+Alt+Shift+F9'), true);
      focused = true; nativeShortcuts.focus(); assert.equal(globalShortcut.isRegistered('Control+Alt+Shift+F9'), false);
      focused = false; nativeShortcuts.blur(); assert.equal(globalShortcut.isRegistered('Control+Alt+Shift+F9'), true);
    }
  } finally { nativeShortcuts.stop(); }
  assert.equal(globalShortcut.isRegistered('Control+Alt+Shift+F9'), false);
  cases.push(nativeRegistration ? 'real native global registration, focus suspension and cleanup; no OS key injection' : 'native registration Not Verified: fixture shortcut occupied');
  assert.equal(await remote.executeJavaScript('typeof window.desktopControls'), 'undefined');
  cases.push('hosted renderer has no settings bridge');
  await select('whats-new');
  assert.equal(await js("document.getElementById('release-version').value"), '0.3.4');
  assert.equal(await js("document.querySelector('#release-notes img')"), null);
  await js("document.querySelector('#release-notes details').open=true");
  assert((await js("document.getElementById('release-notes').textContent")).includes('<img src=x onerror=alert(1)>'));
  await js("document.getElementById('release-version').value='0.3.5';document.getElementById('release-version').dispatchEvent(new Event('change'))");
  desktop.refreshSettings(); await pause(25);
  assert.equal(await js("document.getElementById('release-version').value"), '0.3.5');
  cases.push('installed release selected initially, older selection preserved, notes remain text only');
  await js("document.getElementById('view-updates').click()");
  assert.equal(await js("document.getElementById('section-updates').hidden"), false);
  await select('general');
  await js("document.getElementById('tray-setting').click()"); await pause(25);
  assert.equal(actions.at(-1), 'tray-off');
  assert((await js("document.getElementById('close-behavior').textContent")).includes('taskbar'));
  await js("document.getElementById('tray-setting').focus()");
  controls.sendInputEvent({type:'keyDown',keyCode:'Space'}); controls.sendInputEvent({type:'keyUp',keyCode:'Space'}); await pause(25);
  assert.equal(actions.at(-1),'tray-on');
  await js("document.getElementById('login-setting').click()"); await pause(25); assert.equal(actions.at(-1),'login-on');
  state = { ...state, loginAvailable: false }; desktop.refreshSettings(); await pause(25);
  assert.equal(await js("document.getElementById('login-setting').disabled"), true);
  assert.equal(await js("document.getElementById('login-unavailable').hidden"), false);
  cases.push('tray behavior explanation and unavailable login reason');
  for (const [category, ids] of [['permissions',['test-notification','notification-settings','microphone-settings']], ['about',['copy-diagnostics','reload-client','help-guide','help-issues']]]) {
    await select(category);
    for (const id of ids) { await js(`document.getElementById('${id}').click()`); await pause(10); }
  }
  assert.equal(actions.at(-1),'help-issues'); cases.push('notification, permission, support, copy and reload actions');
  await select('permissions'); await js("document.getElementById('test-notification').focus()");
  controls.sendInputEvent({type:'keyDown',keyCode:'Enter'}); controls.sendInputEvent({type:'char',keyCode:'\r'}); controls.sendInputEvent({type:'keyUp',keyCode:'Enter'}); await pause(25);
  assert.equal(actions.at(-1),'notification-test');
  await select('about');
  for (let n=0;n<2;n++) { await js("document.getElementById('copy-diagnostics').click()"); await pause(25);
    assert.equal(await js("document.getElementById('settings-feedback').textContent"),'Troubleshooting information copied.'); }
  await select('updates');
  assert((await js("document.getElementById('update-history-list').textContent")).includes('Interrupted'));
  for (const [phase, patch] of [
    ['available',{updateCanDownload:true, updateBusy:false}],
    ['downloading',{updateCanDownload:false, updateBusy:true, updateCancelable:true, updateProgress:35}],
    ['awaiting-safe-restart',{updateBusy:true,updateCancelable:false,updateCanRestart:true}],
    ['staged',{updateBusy:false,updateCanRestart:true}],
    ['failed',{updateBusy:false,updateCanRestart:false,updateCanDownload:true}],
  ]) {
    state = { ...state, updatePhase:phase, updateStatus:phase === 'awaiting-safe-restart' ? 'Updates: waiting for assistance cleanup and profile saving…' : 'Updates: ' + phase, ...patch };
    desktop.refreshSettings(); await pause(25); await layout();
    if (phase === 'available') { await js("document.getElementById('update-download').click()"); await pause(10); assert.equal(actions.at(-1),'update-download'); }
    if (phase === 'downloading') { assert.equal(await js("document.getElementById('update-progress').value"),35); assert.equal(await js("document.getElementById('check-updates').disabled"),true); await screenshot('downloading'); await js("document.getElementById('update-cancel').click()"); await pause(10); assert.equal(actions.at(-1),'update-cancel'); }
    if (phase === 'awaiting-safe-restart') assert.equal(await js("document.getElementById('update-restart').disabled"),true);
    if (phase === 'staged') { await js("document.getElementById('update-restart').click()"); await pause(10); assert.equal(actions.at(-1),'update-restart'); }
  }
  await js("document.getElementById('check-updates').click();document.getElementById('update-snooze').click();document.getElementById('update-auto-check').click();document.getElementById('update-auto-download').click();document.getElementById('update-install-on-quit').click()");
  await pause(25);
  assert(actions.includes('check-updates') && actions.includes('update-snooze') && actions.includes('update-auto-check-off') && actions.includes('update-auto-download-off') && actions.includes('update-install-on-quit-on'));
  await js("document.getElementById('update-snooze').click()"); await pause(25); assert.equal(actions.at(-1),'update-resume');
  await js("document.querySelector('.recovery').open=true;document.getElementById('open-updates').click()"); await pause(10); assert.equal(actions.at(-1),'updates');
  cases.push('update availability, progress, safe restart, preferences and Manager recovery');
  await select('about'); await js("document.getElementById('dismiss-settings').click()"); await pause(25);
  assert.equal(await js('document.activeElement.id'),'settings');
  await js("document.getElementById('settings').click()"); await pause(25);
  assert.equal(await js('document.activeElement.id'),'tab-about');
  controls.sendInputEvent({type:'keyDown',keyCode:'Left'}); controls.sendInputEvent({type:'keyUp',keyCode:'Left'}); await pause(25);
  assert.equal(await js('document.activeElement.id'),'tab-whats-new');
  controls.sendInputEvent({type:'keyDown',keyCode:'Home'}); controls.sendInputEvent({type:'keyUp',keyCode:'Home'}); await pause(25);
  assert.equal(await js('document.activeElement.id'),'tab-general');
  await js("document.getElementById('dismiss-settings').focus()");
  controls.sendInputEvent({type:'keyDown',keyCode:'Tab',modifiers:['shift']}); controls.sendInputEvent({type:'keyUp',keyCode:'Tab',modifiers:['shift']}); await pause(25);
  assert.equal(await js("document.getElementById('device-settings').contains(document.activeElement)"),true);
  controls.sendInputEvent({type:'keyDown',keyCode:'Escape'}); controls.sendInputEvent({type:'keyUp',keyCode:'Escape'}); await pause(25);
  assert.equal(await js("document.getElementById('device-settings').hidden"),true);
  cases.push('real Chromium keyboard arrows, Home, focus loop, Escape and reopening focus');
  desktop.showSettings('account'); await pause(25);
  await js("document.getElementById('back-to-palisade').click()"); await pause(25);
  assert.equal(await js("document.getElementById('device-settings').hidden"),true);
  desktop.showSettings('updates'); await pause(25);
  state = { ...state, updateBusy:false, updaterAvailable:false, managerAvailable:false, releaseHistory:[], notesStatus:'Offline. No verified notes saved.' }; desktop.refreshSettings(); await pause(25);
  assert.equal(await js("document.getElementById('update-auto-check').disabled"),true);
  assert.equal(await js("document.getElementById('open-updates').disabled"),true);
  await select('whats-new'); assert.equal(await js("document.getElementById('release-version').disabled"),true);
  assert((await js("document.getElementById('release-notes').textContent")).includes('No verified release notes'));
  cases.push('unavailable updater, disabled recovery, empty offline history');
  owner.setContentSize(640,440); await pause(100); await select('updates'); await layout(); await screenshot('narrow-short');
  assert.equal(await js("document.querySelector('[role=tablist]').getAttribute('aria-orientation')"),'horizontal');
  await js("document.getElementById('settings-content').scrollTop=99999"); await layout();
  cases.push('640x440 short layout and independently scrolling content');
  await js("document.getElementById('settings-content').scrollTop=0");
  controls.setZoomFactor(1.5); await pause(100); await layout(); await screenshot('text-150'); controls.setZoomFactor(1);
  owner.setContentSize(1000,700); await pause(100); await select('general');
  const inspector=controls.debugger; inspector.attach('1.3');
  await inspector.sendCommand('Emulation.setDeviceMetricsOverride',{width:920,height:692,deviceScaleFactor:2,mobile:false}); await pause(100); await layout(); await screenshot('dpi-200');
  await inspector.sendCommand('Emulation.clearDeviceMetricsOverride');
  await inspector.sendCommand('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:'active'},{name:'prefers-reduced-motion',value:'reduce'}]}); await pause(50); await layout(); await screenshot('high-contrast');
  assert.equal(await js("getComputedStyle(document.getElementById('tray-setting'),'::before').transitionDuration"),'0s');
  await inspector.sendCommand('Accessibility.enable'); const tree=await inspector.sendCommand('Accessibility.getFullAXTree');
  assert(tree.nodes.some(node=>node.role?.value==='tab' && node.name?.value==='General'));
  assert(tree.nodes.some(node=>node.role?.value==='switch' && node.name?.value.includes('system tray')));
  inspector.detach(); cases.push('150% text zoom, emulated 200% DPI, forced colors, reduced motion and accessibility tree');
  owner.destroy(); await fs.writeFile(output,JSON.stringify({passed:true,electron:process.versions.electron,cases,images,actionsRecorded:true,realDeviceSettingsChanged:false,actualOsDpiChanged:false,packaged:Boolean(process.env.PALISADE_WINDOW_SMOKE_ASAR)}));
  clearTimeout(deadline); app.exit(0);
}).catch(async error=>{ await fs.writeFile(output,JSON.stringify({passed:false,error:error.stack})); owner?.destroy();clearTimeout(deadline);app.exit(1); });
