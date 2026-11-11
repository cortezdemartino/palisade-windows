'use strict';
const controls = window.desktopControls;
if (controls) {
  document.getElementById('settings').addEventListener('click', () => controls.settings());
  document.getElementById('minimize').addEventListener('click', () => controls.minimize());
  document.getElementById('maximize').addEventListener('click', () => controls.maximize());
  document.getElementById('close').addEventListener('click', () => controls.close());
  document.getElementById('dismiss-settings').addEventListener('click', () => controls.settings());
  document.addEventListener('keydown', (event) => { if (event.key === 'Escape' && !document.getElementById('device-settings').hidden) controls.settings(); });
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
    if (!note) return;
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
  function selectSection(tab) {
    document.querySelectorAll('[data-section]').forEach((button) => {
      const selected = button === tab;
      button.classList.toggle('selected', selected);
      button.setAttribute('aria-pressed', String(selected));
      document.getElementById('section-' + button.dataset.section).hidden = !selected;
    });
    feedback.textContent = '';
  }
  document.querySelectorAll('[data-section]').forEach(tab => tab.addEventListener('click', () => selectSection(tab)));
  for (const [id, action] of [['notification-settings', 'notificationSettings'], ['microphone-settings', 'microphoneSettings'], ['copy-diagnostics', 'copyDiagnostics'], ['reload-client', 'reloadClient']]) {
    document.getElementById(id).addEventListener('click', () => {
      controls[action]();
      feedback.textContent = id === 'copy-diagnostics' ? 'Diagnostic summary requested.' : id === 'reload-client' ? 'Reloading Palisade…' : 'Opening Windows settings…';
    });
  }
  let historyKey;
  const unsubscribe = controls.onState((state) => {
    const { maximized } = state;
    document.getElementById('device-settings').hidden = !state.settingsOpen;
    if (state.settingsOpenTo === 'about') selectSection(document.querySelector('[data-section="about"]'));
    document.getElementById('tray-setting').checked = state.keepRunningInTray;
    document.getElementById('login-setting').checked = state.launchOnLogin;
    document.getElementById('login-setting').disabled = !state.loginAvailable;
    document.getElementById('device-version').textContent = state.version;
    document.getElementById('update-status').textContent = state.updateStatus;
    document.getElementById('latest-version').textContent = state.latestVersion || 'Not checked';
    const lastCheck = document.getElementById('last-update-check');
    lastCheck.textContent = state.lastUpdateCheck ? new Date(state.lastUpdateCheck).toLocaleString() : 'Never';
    const meter = document.getElementById('update-progress');
    meter.hidden = !['downloading', 'verifying'].includes(state.updatePhase);
    meter.value = state.updateProgress || 0;
    document.getElementById('update-unavailable').hidden = state.updaterAvailable;
    document.getElementById('check-updates').disabled = state.updateBusy;
    document.getElementById('open-updates').disabled = state.updateBusy;
    for (const [id, shown, label] of [
      ['update-download', state.updateCanDownload, state.updatePhase === 'failed' ? 'Retry download' : 'Download'],
      ['update-restart', state.updateCanRestart, state.updatePhase === 'failed' ? 'Retry restart' : 'Restart to update'],
    ]) {
      const action = document.getElementById(id);
      action.hidden = !shown;
      action.disabled = state.updateBusy;
      action.textContent = label;
    }
    document.getElementById('update-cancel').hidden = !state.updateCancelable;
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
    if (JSON.stringify(releaseHistory) !== JSON.stringify(state.releaseHistory)) {
      const selected = versionSelector.value;
      releaseHistory = state.releaseHistory || [];
      versionSelector.replaceChildren();
      for (const note of releaseHistory) {
        const option = document.createElement('option'); option.value = note.version;
        option.textContent = `Palisade ${note.version}`; versionSelector.append(option);
      }
      if (releaseHistory.some((note) => note.version === selected)) versionSelector.value = selected;
      versionSelector.disabled = !releaseHistory.length;
      renderNotes();
    }
    document.getElementById('notification-status').textContent = state.notificationStatus;
    const button = document.getElementById('maximize');
    button.dataset.maximized = String(maximized);
    button.title = maximized ? 'Restore' : 'Maximize';
    button.setAttribute('aria-label', button.title);
  });
  window.addEventListener('unload', unsubscribe, { once: true });
}
