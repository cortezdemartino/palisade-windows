'use strict';
const { app, dialog, powerMonitor } = require('electron');
const { fetchRelease, signedBytes, compareVersions } = require('./release.cjs');
const { downloadInstaller } = require('./downloader.cjs');
const { createNotesHistory } = require('./notes-history.cjs');
const { createJournal } = require('./journal.cjs');
const { loadSettings } = require('../settings.cjs');
const path = require('node:path');
const fs = require('node:fs/promises');
const { spawn } = require('node:child_process');
const { createHash, randomBytes, randomUUID } = require('node:crypto');
const CHECK_INTERVAL = 6 * 60 * 60 * 1000;
const RETRY_DELAY = 15 * 60 * 1000;
const releaseDigest = (manifest) => createHash('sha256').update(signedBytes(manifest)).digest('hex');

async function writeManifest(directory, envelope) {
  const bytes = Buffer.from(JSON.stringify(envelope), 'utf8');
  if (!bytes.length || bytes.length > 32 * 1024) throw Error('Invalid signed release manifest.');
  const file = await fs.open(path.join(directory, 'release-v3.json'), 'wx', 0o600);
  try { await file.writeFile(bytes); await file.sync(); } finally { await file.close(); }
}

function createUpdateController(getWindow, onStatus = () => {}, {
  onAvailable = () => {}, fetchFn = globalThis.fetch, now = Date.now, beforeManager,
  native, journal: journalOverride, download = downloadInstaller, persistManifest = writeManifest,
  preferences = () => loadSettings(app.getPath('userData')), canInstallOnQuit = () => true,
} = {}) {
  if (typeof beforeManager !== 'function') throw Error('Exit preparation hook required');
  const supported = () => app.isPackaged && process.platform === 'win32';
  const journal = journalOverride || (supported() ? createJournal(path.join(app.getPath('userData'), 'updates')) : null);
  let managerBusy = false, started = false, stopped = false, handoffAcknowledged = false, handoffUncertain = false;
  let intervalTimer, retryTimer, snoozeTimer, handoffTimer, operation, lastAttempt = null, failures = 0;
  let trust, notesHistory, offer, verified, notifiedVersion, latestVersion = '';
  let notesStatus = 'Release notes have not been checked.';
  let state = 'idle', status = 'Updates: idle', progress = 0, commitSent = false;
  const paused = () => preferences().snoozeUntil > now();
  const active = () => Boolean(operation) || managerBusy || handoffAcknowledged || handoffUncertain;
  const notesState = () => {
    const record = journal?.get();
    const prefs = preferences();
    return { latestVersion, notesStatus, releaseHistory: notesHistory?.snapshot() || [],
      updaterAvailable: supported() && Boolean(native), updatePhase: state, updateProgress: progress,
      updateBusy: active(), updateCancelable: Boolean(operation) && !commitSent && operation.kind !== 'recover',
      updateCanDownload: Boolean(offer && native && !verified), updateCanRestart: Boolean(verified && native),
      lastUpdateCheck: record?.lastCheck || 0, updateHistory: record?.history || [],
      autoCheck: prefs.autoCheck, autoDownload: prefs.autoDownload, installOnQuit: prefs.installOnQuit,
      updatesPaused: paused(),
    };
  };
  function publish(next, message, percent = progress) {
    state = next; status = 'Updates: ' + message; progress = percent;
    onStatus(status);
  }
  function notify(options) {
    const win = getWindow();
    return win && !win.isDestroyed()
      ? dialog.showMessageBox(win, { ...options, noLink: true })
      : dialog.showMessageBox({ ...options, noLink: true });
  }
  function perform(kind, task) {
    if (!supported() || stopped || active()) return Promise.resolve(false);
    const current = { kind, cancel: new AbortController() };
    operation = current;
    const result = Promise.resolve().then(() => task(current.cancel.signal)).catch(() => {
      if (!stopped) publish('failed', 'could not finish. Try again or open Update Manager for repair.');
      return false;
    }).finally(() => {
      if (operation === current) operation = undefined;
      if (!stopped) onStatus(status);
    });
    current.result = result;
    return result;
  }
  async function getTrust() {
    if (trust) return trust;
    const root = path.join(__dirname, '..', '..', 'assets');
    const [key, history] = await Promise.all([
      fs.readFile(path.join(root, 'update-public-key.pem'), 'utf8'),
      fs.readFile(path.join(root, 'update-key-transitions.json'), 'utf8'),
    ]);
    const transitions = JSON.parse(history);
    notesHistory = createNotesHistory({ directory: path.join(app.getPath('userData'), 'updates'), root: key, transitions, fetchFn });
    await notesHistory.load();
    trust = { key, transitions };
    return trust;
  }
  async function lookup(signal) {
    const { key, transitions } = await getTrust();
    const bounded = AbortSignal.any([signal, AbortSignal.timeout(30000)]);
    const result = await fetchRelease((url, options = {}) => fetchFn(url, {
      ...options, signal: AbortSignal.any([bounded, options.signal].filter(Boolean)),
    }), key, app.getVersion(), transitions);
    bounded.throwIfAborted();
    return result;
  }
  function matches(receipt, stage) {
    return receipt?.stage === stage.id && receipt.version === stage.version && receipt.releaseSha256 === stage.releaseSha256;
  }
  async function discard(stage, signal) {
    if (!stage) return;
    await native.discard(stage.id, { signal });
    journal.set({ stage: null });
    verified = undefined;
  }
  async function recover(signal) {
    await getTrust();
    const stage = journal.get().stage;
    if (!stage || !native) return;
    const installed = app.getVersion();
    if (compareVersions(installed, stage.version) >= 0) {
      await discard(stage, signal);
      if (installed === stage.version && stage.phase === 'handoff') {
        journal.set({ history: [{ version: installed, at: now(), result: 'installed' }, ...journal.get().history] });
        publish('completed', 'Palisade ' + installed + ' is running.', 100);
      }
      return;
    }
    if (stage.phase === 'download') {
      await discard(stage, signal);
      publish('recovered', 'interrupted download cleared. Ready to check again.', 0);
      return;
    }
    publish('verifying', 'checking the saved installer…', 100);
    try {
      const receipt = await native.inspect(stage.id, { signal });
      if (!matches(receipt, stage)) throw Error('Saved update differs.');
      verified = receipt;
      latestVersion = receipt.version;
      if (stage.phase === 'handoff') {
        journal.set({ stage: { ...stage, phase: 'staged' }, history: [
          { version: stage.version, at: now(), result: 'interrupted' }, ...journal.get().history,
        ] });
        publish('recovered', 'restart interrupted. Retry restart or open Update Manager for repair.');
      } else publish('staged', 'Palisade ' + stage.version + ' is ready to restart.');
    } catch {
      if (signal.aborted) return;
      await discard(stage, signal);
      publish('failed', 'saved update could not be verified. Check again.', 0);
    }
  }

  async function check(manual = false) {
    if (!supported()) {
      if (manual) await notify({ type: 'info', message: 'Update checks are available in packaged Windows builds.', buttons: ['OK'] });
      return;
    }
    let shouldDownload = false;
    const result = await perform('check', async signal => {
      lastAttempt = now();
      clearTimeout(retryTimer);
      publish('checking', 'checking…');
      try {
        const manifest = await lookup(signal);
        if (stopped || signal.aborted) return;
        failures = 0;
        offer = manifest;
        latestVersion = manifest?.version || app.getVersion();
        try { journal.set({ lastCheck: now() }); } catch { /* Checking does not require writable state. */ }
        if (verified && (!manifest || verified.releaseSha256 !== releaseDigest(manifest))) {
          await discard(journal.get().stage, signal);
        }
        try {
          if (manifest?.signedEnvelope) await notesHistory.fetchFor(manifest.signedEnvelope, signal);
          else if (!manifest) await notesHistory.fetchVersion(app.getVersion(), signal);
          notesStatus = notesHistory.snapshot().some(note => note.version === latestVersion)
            ? 'Signed release notes are available offline.' : 'Release notes are unavailable for this version.';
        } catch { notesStatus = 'Current release notes could not be verified. Cached signed notes remain available.'; }
        if (stopped || signal.aborted) return;
        if (verified) {
          publish('staged', 'Palisade ' + verified.version + ' is ready to restart.', 100);
        } else if (manifest) {
          publish(paused() ? 'paused' : 'available', manifest.version + ' available' +
            (paused() ? '. Automatic updates are paused.' : '.'), 0);
          if (notifiedVersion !== manifest.version) {
            notifiedVersion = manifest.version;
            try { onAvailable(manifest); } catch { /* Keep Settings available if notifications fail. */ }
          }
          shouldDownload = Boolean(native && preferences().autoDownload && !paused());
        } else {
          publish('idle', 'up to date.', 0);
        }
        return manifest;
      } catch {
        if (stopped || signal.aborted) return;
        failures++;
        publish('failed', 'could not verify the latest release. Check your connection and try again.');
        if (started && preferences().autoCheck) {
          retryTimer = setTimeout(() => { if (!paused()) void check(); },
            Math.min(CHECK_INTERVAL, RETRY_DELAY * 2 ** Math.min(failures - 1, 5)));
          retryTimer.unref?.();
        }
      }
    });
    if (shouldDownload) void downloadUpdate();
    return result;
  }

  function downloadUpdate() {
    if (!native || !offer || verified) return Promise.resolve(false);
    return perform('download', async signal => {
      const manifest = offer;
      const stage = { id: randomUUID(), from: app.getVersion(), version: manifest.version,
        releaseSha256: releaseDigest(manifest), phase: 'download' };
      try {
        await discard(journal.get().stage, signal);
        journal.set({ stage });
        publish('downloading', 'downloading Palisade ' + stage.version + '…', 0);
        const created = await native.stage(stage.id, { signal });
        signal.throwIfAborted();
        await persistManifest(created.directory, manifest.signedEnvelope);
        await download(manifest, created.directory, fetchFn, (bytes, total) => {
          const percent = Math.min(100, Math.floor(bytes * 100 / total));
          if (!signal.aborted && percent !== progress) publish('downloading', 'downloading Palisade ' + stage.version + '…', percent);
        }, { signal });
        signal.throwIfAborted();
        publish('verifying', 'verifying the downloaded installer…', 100);
        const receipt = await native.inspect(stage.id, { signal });
        signal.throwIfAborted();
        if (!matches(receipt, stage)) throw Error('Downloaded update differs from the signed release.');
        journal.set({ stage: { ...stage, phase: 'staged' } });
        verified = receipt;
        publish('staged', 'Palisade ' + stage.version + ' is ready to restart.', 100);
        return true;
      } catch {
        if (!stopped && journal.get().stage?.id === stage.id) {
          try { await discard(stage); } catch { /* The next recovery or retry can settle an interrupted stage. */ }
        }
        if (!stopped) publish(signal.aborted ? 'canceled' : 'failed',
          signal.aborted ? 'download canceled.' : 'download or verification failed. Try again.', 0);
        return false;
      }
    });
  }

  async function restart({ onQuit = false } = {}) {
    if (!native || !verified || active() || stopped) return false;
    const result = await perform('restart', async signal => {
      const stage = journal.get().stage;
      if (!stage || !matches(verified, stage)) return false;
      try {
        if (!onQuit) {
          const answer = await notify({ type: 'question', title: 'Restart Palisade?',
            message: 'Restart to install Palisade ' + stage.version + '?',
            detail: 'Save your work and finish calls and assistance before restarting.',
            buttons: ['Restart and update', 'Later'], defaultId: 1, cancelId: 1 });
          if (answer.response !== 0 || signal.aborted) return false;
        }
        publish('verifying', 'rechecking the signed release…');
        const fresh = await lookup(signal);
        if (!fresh || releaseDigest(fresh) !== stage.releaseSha256) {
          await discard(stage, signal);
          offer = fresh;
          latestVersion = fresh?.version || app.getVersion();
          publish('failed', 'release changed. Check for the current update before restarting.', 0);
          return false;
        }
        const receipt = await native.inspect(stage.id, { signal });
        if (!matches(receipt, stage)) throw Error('Stage verification failed.');
        signal.throwIfAborted();
        publish('awaiting-safe-restart', 'waiting for assistance cleanup and profile saving…');
        try {
          const preparation = beforeManager('update');
          if (!preparation || typeof preparation.then !== 'function' || await preparation !== undefined)
            throw Error('Exit preparation incomplete.');
        } catch {
          publish('deferred', 'restart postponed until assistance cleanup finishes.');
          return false;
        }
        signal.throwIfAborted();
        const nonce = randomBytes(32).toString('hex');
        journal.set({ stage: { ...stage, phase: 'handoff', nonce } });
        await native.apply(receipt, nonce, { signal, onCommit: () => {
          commitSent = true;
          publish('awaiting-safe-restart', 'waiting for the native updater to confirm the restart…');
        } });
        handoffAcknowledged = true;
        publish('installing', 'installing Palisade ' + stage.version + ' after exit.');
        return true;
      } catch (error) {
        if (!stopped) {
          if (commitSent || error?.commitSent) {
            handoffUncertain = true;
            publish('failed', 'restart could not be confirmed. An authorized update may run when Palisade exits.');
            void checkHandoff();
          } else {
            if (journal.get().stage?.phase === 'handoff') {
              try { journal.set({ stage: { ...stage, phase: 'staged' } }); } catch { /* Recovery will recheck the stage. */ }
            }
            publish(signal.aborted ? 'canceled' : 'failed',
              signal.aborted ? 'restart canceled.' : 'installer verification or restart failed. Try again or open Update Manager.');
          }
        }
        return false;
      } finally { commitSent = false; }
    });
    if (result && !onQuit) app.quit();
    return result;
  }
  async function checkHandoff() {
    if (stopped || !handoffUncertain) return;
    try {
      const response = await native.status({ timeout: 3000 });
      if (stopped) return;
      if (response.state === 'idle') {
        handoffUncertain = false;
        publish('recovered', 'the previous update has stopped. You can try again.');
        return;
      }
    } catch { /* Only an authenticated idle response settles uncertain ownership. */ }
    if (!stopped) {
      handoffTimer = setTimeout(() => { void checkHandoff(); }, 5000);
      handoffTimer.unref?.();
    }
  }
  async function openManager() {
    if (active() || stopped) return;
    managerBusy = true;
    let directory, preparing = false;
    try {
      if (!app.isPackaged || process.platform !== 'win32') {
        await notify({ type: 'info', message: 'The update manager is included in installed Windows builds.', buttons: ['OK'] });
        return;
      }
      const answer = await notify({ type: 'question', title: 'Palisade Update Manager',
        message: 'Open the update manager and exit Palisade?',
        detail: 'Save your work first. The manager checks for releases and handles installation, updates and repairs. Your profile is retained.',
        buttons: ['Open Update Manager and exit', 'Cancel'], defaultId: 1, cancelId: 1 });
      if (answer.response !== 0 || stopped) return;
      preparing = true;
      onStatus('Updates: preparing to exit');
      const preparation = beforeManager('update');
      if (!preparation || typeof preparation.then !== 'function' || await preparation !== undefined)
        throw Error('Exit preparation incomplete');
      preparing = false;
      if (stopped) return;
      const source = path.join(process.resourcesPath, 'manager', 'Palisade-Manager.exe');
      // Run a copy outside the client folder so updates can replace the bundled manager.
      directory = await fs.mkdtemp(path.join(app.getPath('temp'), 'palisade-manager-'));
      const executable = path.join(directory, 'Palisade-Manager.exe');
      if (stopped) { await fs.rm(directory, { recursive: true, force: true }); directory = null; return; }
      await fs.copyFile(source, executable, require('node:fs').constants.COPYFILE_EXCL);
      if (stopped) { await fs.rm(directory, { recursive: true, force: true }); directory = null; return; }
      const child = spawn(executable, ['--installation', path.dirname(process.execPath)], {
        detached: true, stdio: 'ignore', shell: false, windowsHide: false
      });
      await new Promise((resolve, reject) => { child.once('error', reject); child.once('spawn', resolve); });
      child.unref();
      onStatus('Updates: manager opened');
      app.quit();
    } catch {
      if (directory) await fs.rm(directory, { recursive: true, force: true }).catch(() => {});
      if (preparing) {
        onStatus('Updates: exit postponed');
        await notify({ type: 'warning', message: 'Palisade could not safely prepare to exit.',
          detail: 'Wait for any assistance or file transfers to finish, then try again. The update manager was not opened.', buttons: ['OK'] });
        return;
      }
      onStatus('Updates: manager unavailable');
      await notify({ type: 'warning', message: 'Palisade could not open its update manager.',
        detail: 'Use the official Palisade Update Manager to update or repair this installation. Palisade has not been changed.', buttons: ['OK'] });
    } finally { managerBusy = false; }
  }
  async function onQuit() {
    if (operation?.kind === 'restart') {
      await operation.result;
      return;
    }
    if (native && verified && !active() && !paused() && preferences().installOnQuit && canInstallOnQuit())
      await restart({ onQuit: true });
  }
  function resume() {
    if (preferences().autoCheck && !paused() && (lastAttempt === null || now() - lastAttempt >= CHECK_INTERVAL)) void check();
  }
  function scheduleSnooze() {
    clearTimeout(snoozeTimer);
    if (!started || !paused()) return;
    snoozeTimer = setTimeout(() => {
      if (!paused() && preferences().autoCheck) void check();
      else onStatus(status);
    }, Math.min(24 * 60 * 60 * 1000, preferences().snoozeUntil - now()));
    snoozeTimer.unref?.();
  }
  function cancel() {
    if (!commitSent && operation && operation.kind !== 'recover') {
      operation.cancel.abort();
      if (operation.kind === 'check') publish('canceled', 'check canceled.');
    }
  }
  function settingsChanged() {
    if (operation?.kind === 'restart' || handoffAcknowledged || handoffUncertain) return;
    scheduleSnooze();
    if (paused()) {
      if (operation?.kind === 'download' || operation?.kind === 'check') cancel();
      publish('paused', 'automatic updates paused.');
    } else if (!active() && preferences().autoCheck) {
      void check();
    } else onStatus(status);
  }
  function stop() {
    stopped = true; started = false;
    clearTimeout(retryTimer); clearTimeout(snoozeTimer); clearTimeout(handoffTimer); clearInterval(intervalTimer);
    if (!commitSent) operation?.cancel.abort();
    powerMonitor.removeListener('resume', resume);
    app.removeListener('will-quit', stop);
  }
  function start() {
    if (started || !supported()) return;
    started = true; stopped = false;
    intervalTimer = setInterval(() => {
      if (preferences().autoCheck && !paused()) void check();
    }, CHECK_INTERVAL);
    intervalTimer.unref?.();
    powerMonitor.on('resume', resume);
    app.once('will-quit', stop);
    scheduleSnooze();
    void perform('recover', recover).then(() => { if (!stopped) resume(); });
  }
  return { check, openManager, start, stop, notesState, downloadUpdate, restart, cancel, settingsChanged, onQuit };
}
module.exports = { createUpdateController };
