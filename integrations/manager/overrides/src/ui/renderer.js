'use strict';
const $ = id => document.getElementById(id);
let latest;
let notesHistory = [];
function renderNotes() {
  const target = $('release-notes'); target.replaceChildren();
  const note = notesHistory.find((item) => item.version === $('release-version').value);
  if (!note) return;
  const append = (parent, tag, value) => { const e = document.createElement(tag); e.textContent = value; parent.append(e); return e; };
  append(target, 'h2', note.title);
  const date = append(target, 'time', new Date(note.publishedAt).toLocaleDateString()); date.dateTime = note.publishedAt;
  append(target, 'p', note.summary);
  const details = document.createElement('details'); append(details, 'summary', 'Release details');
  for (const section of [...note.sections, ...(note.compatibility.length ? [{ title: 'Compatibility', items: note.compatibility }] : [])]) {
    append(details, 'h3', section.title);
    const list = document.createElement('ul'); for (const item of section.items) append(list, 'li', item); details.append(list);
  }
  target.append(details);
}
$('release-version').addEventListener('change', renderNotes);
function text(id, value) { $(id).textContent = value; }
function render(s) {
  latest = s; document.documentElement.dataset.theme = s.preferences?.theme || 'dark';
  const installed = s.current?.status === 'installed';
  const incomplete = s.current?.status === 'incomplete';
  const unknown = !s.current && s.phase !== 'initializing';
  text('status', s.phase === 'initializing' ? 'INITIALIZING' : installed ? 'INSTALLED' : incomplete ? 'NEEDS ATTENTION' : 'READY TO INSTALL');
  text('headline', installed ? 'Keep Palisade ready.' : incomplete ? 'Let’s restore Palisade.' : 'Welcome to Palisade.');
  text('description', installed ? 'Your desktop application is installed. Check for a verified update or repair your installation.' : incomplete ? 'An installation was found with missing files. Choose a verified package to repair it.' : 'Choose a verified release to get started. Install directly from this manager.');
  if (unknown) {
    text('status', 'STATUS UNAVAILABLE'); text('headline', 'Check installation status.');
    text('description', 'Detection could not finish. Open Troubleshooting and refresh detection before installing.');
  }
  text('current', s.current?.version || (s.phase === 'initializing' ? 'Checking…' : 'Not installed'));
  text('install-state', installed ? 'Application files detected' : incomplete ? 'Installation incomplete' : 'Fresh installation');
  if (unknown) { text('current', 'Unknown'); text('install-state', 'Detection needs attention'); }
  text('available', s.available?.version || 'Not checked');
  text('source', s.available ? (s.source === 'offline' ? 'Verified offline package' : 'Signed stable release') : 'Check for a signed release');
  text('path', s.current?.path || 'Detecting…'); text('message', s.message); text('service', s.service);
  text('footer-status', s.busy ? 'Operation in progress' : 'Ready');
  const primary = $('primary');
  primary.dataset.command = s.available ? 'install' : 'check';
  primary.textContent = !s.available ? 'Check for updates' : !installed && !incomplete ? 'Install Palisade' : s.current?.version === s.available.version ? 'Repair Palisade' : 'Update Palisade';
  $('launch').hidden = !installed; $('check').hidden = !s.available;
  document.querySelectorAll('[data-command]').forEach(b => { b.disabled = Boolean(s.busy) && b.dataset.command !== 'cancel'; });
  $('cancel').hidden = !s.canCancel;
  $('progress').hidden = !s.busy;
  if (s.progress) {
    $('progress').value = s.progress.bytes / s.progress.total * 100;
    text('progress-text', (s.progress.bytes / 1048576).toFixed(1) + ' / ' + (s.progress.total / 1048576).toFixed(1) + ' MB · ' + Math.floor(s.progress.bytes / s.progress.total * 100) + '%');
  } else { $('progress').removeAttribute('value'); text('progress-text', s.phase === 'installing' ? 'Installing application files. Please wait; Windows may request administrator permission.' : ''); }
  $('error').hidden = !s.error; text('error', s.error || '');
  $('recovery').hidden = !s.recovery; text('recovery', s.recovery || '');
  text('release-title', s.available ? 'Palisade ' + s.available.version : 'No release selected');
  text('release-size', s.available ? (s.available.size / 1048576).toFixed(1) + ' MB · Windows x64 · ' + s.available.publishedAt.slice(0, 10) : '');
  text('release-verification', s.available ? 'Ed25519 release signature verified. Package integrity is checked again before execution.' : 'Check for updates or select a signed offline package.');
  text('notes', s.notes || 'Verified release notes are unavailable.');
  if (JSON.stringify(notesHistory) !== JSON.stringify(s.releaseHistory || [])) {
    const selected = $('release-version').value; notesHistory = s.releaseHistory || [];
    $('release-version').replaceChildren();
    for (const note of notesHistory) {
      const option = document.createElement('option'); option.value = note.version; option.textContent = `Palisade ${note.version}`; $('release-version').append(option);
    }
    if (notesHistory.some((note) => note.version === selected)) $('release-version').value = selected;
    $('release-version').disabled = !notesHistory.length; renderNotes();
  }
  $('theme').value = s.preferences?.theme || 'dark'; $('shortcut').checked = s.preferences?.desktopShortcut ?? true;
  $('theme').disabled = Boolean(s.busy); $('shortcut').disabled = Boolean(s.busy);
  $('history-list').replaceChildren();
  if (!s.history?.length) text('history-list', 'No installation activity yet.');
  for (const item of [...(s.history || [])].reverse()) {
    const row = document.createElement('div'); row.className = 'history-row';
    for (const value of [item.version, item.result, new Date(item.at).toLocaleString()]) {
      const cell = document.createElement('span'); cell.textContent = value; row.append(cell);
    }
    $('history-list').append(row);
  }
}
document.querySelectorAll('[data-page]').forEach(button => button.addEventListener('click', () => {
  document.querySelectorAll('.page').forEach(p => { p.hidden = p.id !== button.dataset.page; });
  document.querySelectorAll('[data-page]').forEach(b => {
    b.classList.toggle('active', b === button);
    if (b === button) b.setAttribute('aria-current', 'page'); else b.removeAttribute('aria-current');
  });
}));
document.querySelectorAll('[data-command]').forEach(button => button.addEventListener('click', () => window.palisade.command(button.dataset.command)));
for (const id of ['theme', 'shortcut']) $(id).addEventListener('change', () => window.palisade.command('preferences', { theme: $('theme').value, desktopShortcut: $('shortcut').checked }));
window.palisade.onState(render);
window.palisade.snapshot().then(render);
