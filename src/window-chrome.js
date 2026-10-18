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
  document.getElementById('open-updates').addEventListener('click', () => controls.openUpdates());
  const feedback = document.getElementById('settings-feedback');
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
    const button = document.getElementById('maximize');
    button.dataset.maximized = String(maximized);
    button.title = maximized ? 'Restore' : 'Maximize';
    button.setAttribute('aria-label', button.title);
  });
  window.addEventListener('unload', unsubscribe, { once: true });
}
