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
  document.querySelectorAll('[data-section]').forEach((tab) => tab.addEventListener('click', () => {
    document.querySelectorAll('[data-section]').forEach((button) => {
      const selected = button === tab;
      button.classList.toggle('selected', selected);
      button.setAttribute('aria-pressed', String(selected));
      document.getElementById('section-' + button.dataset.section).hidden = !selected;
    });
    feedback.textContent = '';
  }));
  for (const [id, action] of [['notification-settings', 'notificationSettings'], ['microphone-settings', 'microphoneSettings'], ['copy-diagnostics', 'copyDiagnostics'], ['reload-client', 'reloadClient']]) {
    document.getElementById(id).addEventListener('click', () => {
      controls[action]();
      feedback.textContent = id === 'copy-diagnostics' ? 'Diagnostic summary requested.' : id === 'reload-client' ? 'Reloading Palisade…' : 'Opening Windows settings…';
    });
  }
  const unsubscribe = controls.onState((state) => {
    const { maximized } = state;
    document.getElementById('device-settings').hidden = !state.settingsOpen;
    document.getElementById('tray-setting').checked = state.keepRunningInTray;
    document.getElementById('login-setting').checked = state.launchOnLogin;
    document.getElementById('login-setting').disabled = !state.loginAvailable;
    document.getElementById('device-version').textContent = state.version;
    document.getElementById('update-status').textContent = state.updateStatus;
    document.getElementById('latest-version').textContent = state.latestVersion || 'Not checked';
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
