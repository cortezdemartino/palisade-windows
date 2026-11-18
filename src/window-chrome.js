'use strict';
const controls = window.desktopControls;
if (controls) {
  document.getElementById('settings').addEventListener('click', () => controls.settings());
  document.getElementById('minimize').addEventListener('click', () => controls.minimize());
  document.getElementById('maximize').addEventListener('click', () => controls.maximize());
  document.getElementById('close').addEventListener('click', () => controls.close());
  document.getElementById('dismiss-settings').addEventListener('click', () => controls.settings());
  const panel = document.getElementById('device-settings');
  let media = null, recording = false;
  const shortcut = document.getElementById('device-shortcut');
  const saveMedia = value => { if (media) controls.setMediaSettings({ ...value, revision: media.revision }); };
  const saveBinding = (accelerator, enabled) => saveMedia({ op: 'binding', action: 'device-settings', accelerator, enabled });
  document.getElementById('device-shortcut-enabled').addEventListener('change', event => saveBinding(media?.binding.accelerator || '', event.target.checked));
  document.getElementById('background-media-shortcuts').addEventListener('change', event => saveMedia({ op: 'background', enabled: event.target.checked }));
  const stopRecording = () => { recording = false; shortcut.value = media?.binding.accelerator || ''; document.getElementById('record-shortcut').textContent = 'Record shortcut'; };
  document.getElementById('record-shortcut').addEventListener('click', () => { recording = true; shortcut.value = 'Press your shortcut…'; shortcut.focus(); document.getElementById('record-shortcut').textContent = 'Recording…'; });
  shortcut.addEventListener('blur', stopRecording);
  shortcut.addEventListener('keydown', event => {
    if (!recording) return;
    if (event.key === 'Tab') { stopRecording(); return; }
    event.preventDefault(); event.stopPropagation();
    if (event.key === 'Escape') { shortcut.value = media?.binding.accelerator || ''; stopRecording(); return; }
    if (event.repeat || event.isComposing || ['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return;
    if (event.metaKey || event.getModifierState('AltGraph')) { document.getElementById('media-shortcut-status').textContent = 'Windows and AltGr typing combinations cannot be assigned.'; return; }
    const key = ({ ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', ' ': 'Space' }[event.key] || (event.key.length === 1 ? event.key.toUpperCase() : event.key));
    stopRecording(); saveBinding([event.ctrlKey && 'Control', event.altKey && 'Alt', event.shiftKey && 'Shift', key].filter(Boolean).join('+'), true);
  });
  document.getElementById('clear-shortcut').addEventListener('click', () => { stopRecording(); saveBinding('', false); });
  document.getElementById('reset-shortcuts').addEventListener('click', () => { stopRecording(); saveMedia({ op: 'reset' }); });
  document.addEventListener('keydown', (event) => {
    if (panel.hidden) return;
    if (event.key === 'Escape') { event.preventDefault(); controls.settings(); }
    if (event.key === 'Tab') {
      const items = [...panel.querySelectorAll('button,input,select,summary,[tabindex="0"]')]
        .filter(item => !item.disabled && item.tabIndex >= 0 && item.getClientRects().length);
      const first = items[0], last = items.at(-1);
      if (event.shiftKey && (document.activeElement === first || !panel.contains(document.activeElement))) {
        event.preventDefault(); last?.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !panel.contains(document.activeElement))) {
        event.preventDefault(); first?.focus();
      }
    }
  });
  document.getElementById('tray-setting').addEventListener('change', (event) => controls.setTray(event.target.checked));
  document.getElementById('login-setting').addEventListener('change', (event) => controls.setLogin(event.target.checked));
  document.getElementById('test-notification').addEventListener('click', () => controls.testNotification());
  document.getElementById('check-updates').addEventListener('click', () => controls.checkForUpdates());
  document.getElementById('open-updates').addEventListener('click', () => controls.openUpdates());
  document.getElementById('update-download').addEventListener('click', () => controls.downloadUpdate());
  document.getElementById('update-restart').addEventListener('click', () => controls.restartForUpdate());
  document.getElementById('update-cancel').addEventListener('click', () => controls.cancelUpdate());
  let updatesPaused = false;
  document.getElementById('update-snooze').addEventListener('click', () => controls.setUpdatesPaused(!updatesPaused));
  const updatePreferences = [['auto-check', 'autoCheck'], ['auto-download', 'autoDownload'], ['install-on-quit', 'installOnQuit']];
  for (const [preference] of updatePreferences)
    document.getElementById('update-' + preference).addEventListener('change', event => controls.setUpdatePreference(preference, event.target.checked));
  const feedback = document.getElementById('settings-feedback');
  let releaseHistory = [];
  const versionSelector = document.getElementById('release-version');
  function renderNotes() {
    const target = document.getElementById('release-notes');
    target.replaceChildren();
    const note = releaseHistory.find((item) => item.version === versionSelector.value);
    if (!note) { const empty = document.createElement('p'); empty.className = 'hint'; empty.textContent = 'No verified release notes are saved yet. Check for updates to look for notes.'; target.append(empty); return; }
    const append = (parent, tag, value) => {
      const element = document.createElement(tag); element.textContent = value; parent.append(element); return element;
    };
    append(target, 'h2', note.title);
    const date = append(target, 'time', new Date(note.publishedAt).toLocaleDateString());
    date.dateTime = note.publishedAt;
    append(target, 'p', note.summary);
    const details = document.createElement('details');
    append(details, 'summary', 'Release details');
    for (const section of [...note.sections, ...(note.compatibility.length ? [{ title: 'Compatibility', items: note.compatibility }] : [])]) {
      append(details, 'h3', section.title);
      const list = document.createElement('ul');
      for (const item of section.items) append(list, 'li', item);
      details.append(list);
    }
    target.append(details);
  }
  versionSelector.addEventListener('change', renderNotes);
  const tabs = [...document.querySelectorAll('[data-section]')];
  const scroller = document.getElementById('settings-content'), scrollPositions = new Map();
  let selectedSection = 'general';
  const categorySelector = document.getElementById('settings-category');
  function focusCategory() { (categorySelector.getClientRects().length ? categorySelector : document.querySelector(`[data-section="${selectedSection}"]`)).focus(); }
  function selectSection(tab, focus = false) {
    if (!tab) return;
    scrollPositions.set(selectedSection, scroller.scrollTop);
    document.querySelectorAll('[data-section]').forEach((button) => {
      const selected = button === tab;
      button.classList.toggle('selected', selected);
      button.setAttribute('aria-selected', String(selected));
      button.tabIndex = selected ? 0 : -1;
      document.getElementById('section-' + button.dataset.section).hidden = !selected;
    });
    selectedSection = tab.dataset.section;
    categorySelector.value = selectedSection;
    scroller.scrollTop = scrollPositions.get(selectedSection) || 0;
    feedback.textContent = '';
    if (focus) focusCategory();
  }
  categorySelector.addEventListener('change', () => selectSection(document.querySelector(`[data-section="${categorySelector.value}"]`)));
  tabs.forEach(tab => {
    tab.addEventListener('click', () => selectSection(tab));
    tab.addEventListener('keydown', event => {
      const step = ['ArrowRight', 'ArrowDown'].includes(event.key) ? 1 : ['ArrowLeft', 'ArrowUp'].includes(event.key) ? -1 : 0;
      let next = step ? tabs[(tabs.indexOf(tab) + step + tabs.length) % tabs.length] : event.key === 'Home' ? tabs[0] : event.key === 'End' ? tabs.at(-1) : null;
      if (next) { event.preventDefault(); selectSection(next, true); }
    });
  });
  const narrow = window.matchMedia('(max-width:620px)');
  const orientation = () => document.querySelector('.settings-tabs').setAttribute('aria-orientation', narrow.matches ? 'horizontal' : 'vertical');
  narrow.addEventListener('change', orientation); orientation();
  document.getElementById('view-updates').addEventListener('click', () => selectSection(document.getElementById('tab-updates'), true));
  document.getElementById('back-to-palisade').addEventListener('click', () => controls.backToPalisade());
  for (const [id, action] of [['notification-settings', 'notificationSettings'], ['microphone-settings', 'microphoneSettings'], ['copy-diagnostics', 'copyDiagnostics'], ['reload-client', 'reloadClient'], ['help-guide', 'helpGuide'], ['help-issues', 'helpIssues']]) {
    document.getElementById(id).addEventListener('click', () => {
      lastFeedback = null;
      controls[action]();
      feedback.textContent = id === 'copy-diagnostics' ? 'Copying troubleshooting information…' : id === 'reload-client' ? 'Reload requested.' : id.startsWith('help-') ? 'Opening your browser…' : 'Opening Windows settings…';
    });
  }
  let historyKey, wasOpen = false, lastFeedback;
  const unsubscribe = controls.onState((state) => {
    media = state.media || null;
    if (!state.settingsOpen) stopRecording();
    if (!recording) shortcut.value = media?.binding.accelerator || '';
    document.getElementById('device-shortcut-enabled').checked = media?.binding.enabled === true;
    document.getElementById('background-media-shortcuts').checked = media?.backgroundShortcuts === true;
    for (const id of ['device-shortcut-enabled', 'background-media-shortcuts', 'record-shortcut', 'clear-shortcut', 'reset-shortcuts']) document.getElementById(id).disabled = !media || Boolean(state.mediaError);
    document.getElementById('media-shortcut-status').textContent = state.mediaError || (state.mediaShortcutFailures?.length ? 'Windows could not register your shortcut. It may be used by another app. Change the shortcut or turn off background shortcuts.' : 'Changes save immediately on this PC.');
    const { maximized } = state;
    document.getElementById('device-settings').hidden = !state.settingsOpen;
    if (state.settingsOpenTo) selectSection(document.querySelector(`[data-section="${state.settingsOpenTo}"]`), true);
    if (state.settingsOpen && !wasOpen) focusCategory();
    if (!state.settingsOpen && wasOpen && !state.returnToApp) document.getElementById('settings').focus();
    wasOpen = state.settingsOpen;
    document.getElementById('tray-setting').checked = state.keepRunningInTray;
    document.getElementById('login-setting').checked = state.launchOnLogin;
    document.getElementById('login-setting').disabled = !state.loginAvailable;
    document.getElementById('login-unavailable').hidden = state.loginAvailable;
    document.getElementById('close-behavior').textContent = state.keepRunningInTray
      ? 'Close and Minimize hide the window in the system tray. To exit completely, choose Quit Palisade from the tray menu.'
      : 'Close exits Palisade after assistance cleanup and profile saving. Minimize keeps it on the taskbar.';
    document.getElementById('device-version').textContent = state.version;
    document.getElementById('installed-version').textContent = state.version;
    document.getElementById('about-version').textContent = state.version;
    document.getElementById('build-details').textContent = `Windows ${state.architecture || 'x64'} · ${state.buildKind || 'desktop'} build`;
    document.getElementById('runtime-details').textContent = state.electronVersion ? 'Electron ' + state.electronVersion : 'Not available';
    document.getElementById('update-status').textContent = (state.updateStatus || 'Update status unavailable').replace(/^Updates:\s*/, '');
    document.getElementById('latest-version').textContent = state.latestVersion || 'Not checked';
    const lastCheck = document.getElementById('last-update-check');
    lastCheck.textContent = state.lastUpdateCheck ? new Date(state.lastUpdateCheck).toLocaleString() : 'Never';
    const meter = document.getElementById('update-progress');
    meter.hidden = !['downloading', 'verifying'].includes(state.updatePhase);
    meter.value = state.updateProgress || 0;
    meter.setAttribute('aria-valuetext', state.updatePhase === 'verifying' ? 'Verifying the installer' : `${state.updateProgress || 0}% downloaded`);
    document.getElementById('update-unavailable').hidden = state.updaterAvailable;
    document.getElementById('check-updates').disabled = state.updateBusy;
    document.getElementById('open-updates').disabled = state.updateBusy || !state.managerAvailable;
    for (const [id, shown, label] of [
      ['update-download', state.updateCanDownload, state.updatePhase === 'failed' ? 'Retry download' : 'Download update'],
      ['update-restart', state.updateCanRestart, state.updatePhase === 'failed' ? 'Retry restart' : 'Restart to update'],
    ]) {
      const action = document.getElementById(id);
      action.hidden = !shown;
      action.disabled = state.updateBusy;
      action.textContent = label;
    }
    document.getElementById('update-cancel').hidden = !state.updateCancelable;
    document.getElementById('update-cancel').textContent = state.updatePhase === 'downloading' ? 'Cancel download' : state.updatePhase === 'checking' ? 'Cancel check' : 'Cancel update';
    updatesPaused = state.updatesPaused;
    const snooze = document.getElementById('update-snooze');
    snooze.textContent = updatesPaused ? 'Resume updates' : 'Pause for 24 hours';
    snooze.disabled = !state.updaterAvailable || (state.updateBusy && !['checking', 'downloading'].includes(state.updatePhase));
    for (const [preference, key] of updatePreferences) {
      const input = document.getElementById('update-' + preference);
      input.checked = state[key];
      input.disabled = !state.updaterAvailable;
    }
    const history = state.updateHistory || [];
    const serialized = JSON.stringify(history);
    if (serialized !== historyKey) {
      historyKey = serialized;
      const list = document.getElementById('update-history-list');
      list.replaceChildren();
      for (const item of history) {
        const row = document.createElement('li');
        row.textContent = 'Palisade ' + item.version + ' · ' + (item.result === 'installed' ? 'Installed' : 'Interrupted') +
          ' · ' + new Date(item.at).toLocaleDateString();
        list.append(row);
      }
      if (!history.length) {
        const row = document.createElement('li');
        row.textContent = 'No updates recorded yet.';
        list.append(row);
      }
    }
    document.getElementById('notes-status').textContent = state.notesStatus;
    const currentNotesHint = document.getElementById('notes-current-hint');
    currentNotesHint.hidden = !(state.releaseHistory?.length && !state.releaseHistory.some(note => note.version === state.version));
    currentNotesHint.textContent = `Verified notes for installed version ${state.version} aren't saved. Choose another release below.`;
    if (JSON.stringify(releaseHistory) !== JSON.stringify(state.releaseHistory)) {
      const selected = versionSelector.value;
      releaseHistory = state.releaseHistory || [];
      versionSelector.replaceChildren();
      for (const note of releaseHistory) {
        const option = document.createElement('option'); option.value = note.version;
        option.textContent = `Palisade ${note.version}`; versionSelector.append(option);
      }
      if (releaseHistory.some((note) => note.version === selected)) versionSelector.value = selected;
      else if (releaseHistory.some(note => note.version === state.version)) versionSelector.value = state.version;
      if (!releaseHistory.length) { const option = document.createElement('option'); option.value = ''; option.textContent = 'No verified notes saved yet'; versionSelector.append(option); }
      versionSelector.disabled = !releaseHistory.length;
      renderNotes();
    }
    document.getElementById('notification-status').textContent = state.notificationStatus;
    document.getElementById('assist-native-status').textContent = !state.nativeAssistStatus || state.nativeAssistStatus === 'Native endpoint unavailable'
      ? "Assist isn't available in this build." : state.nativeAssistStatus;
    if (state.deviceFeedback !== lastFeedback) {
      lastFeedback = state.deviceFeedback;
      feedback.textContent = state.deviceFeedback || '';
      feedback.dataset.error = String(state.deviceFeedbackError === true);
    }
    const button = document.getElementById('maximize');
    button.dataset.maximized = String(maximized);
    button.title = maximized ? 'Restore' : 'Maximize';
    button.setAttribute('aria-label', button.title);
  });
  window.addEventListener('unload', unsubscribe, { once: true });
}
