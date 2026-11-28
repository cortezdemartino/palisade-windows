'use strict';
const { isPalisadeUrl } = require('../security-policy.cjs');

// Diagnostics bridge, protocol `palisade.diagnostics` v1, between the hosted Palisade page and
// the desktop main process (project-palisade apps/web/src/platform/desktopDiagnostics.ts).
// The page can read pending crash summaries, acknowledge (delete) them after queuing them with
// the person's consent, and forward sanitized breadcrumbs. Nothing else: the page cannot read
// files, minidumps or settings, and the main process never receives or holds the Matrix session.
// Requests are accepted only from the trusted main frame on the Palisade origin, are validated
// exactly, and are rate limited.

const PROTOCOL = 'palisade.diagnostics';
const INFO = 'palisade:diagnostics-info';
const PENDING = 'palisade:diagnostics-pending';
const ACK = 'palisade:diagnostics-ack';
const CRUMB = 'palisade:diagnostics-breadcrumb';
const RECOVERY = 'palisade:diagnostics-recovery';
const ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function installDiagnosticsBridge(contents, { diagnostics, store, now = Date.now, ipc = require('electron').ipcMain }) {
  let stopped = false, windowStart = now(), calls = 0, crumbStart = now(), crumbCount = 0, requested = false;
  const trusted = event => !stopped && !contents.isDestroyed() && event.sender === contents &&
    event.senderFrame === contents.mainFrame && isPalisadeUrl(contents.getURL()) && isPalisadeUrl(event.senderFrame?.url);
  const limit = () => {
    const t = now();
    if (t - windowStart >= 1000) { windowStart = t; calls = 0; }
    return ++calls <= 10;
  };
  const guard = (event, args) => {
    if (!trusted(event) || !limit()) throw new Error('Diagnostics request refused');
    return args;
  };

  ipc.handle(INFO, (event, ...args) => {
    guard(event, args);
    if (args.length !== 0) throw new Error('Diagnostics request refused');
    const id = diagnostics.identity();
    const { previous, pending } = diagnostics.recovery();
    return { protocol: PROTOCOL, version: 1, desktopVersion: id.appVersion, buildId: id.buildId, channel: id.channel,
      osVersion: id.os.version, recovery: { uncleanExit: Boolean(previous), crashLoop: Boolean(previous?.crashLoop), pending, requested: consume() } };
  });
  ipc.handle(PENDING, (event, ...args) => {
    guard(event, args);
    if (args.length !== 0) throw new Error('Diagnostics request refused');
    return store.list().slice(0, 20).map(({ localId, report, category }) => ({ localId, report, category }));
  });
  ipc.handle(ACK, (event, ids) => {
    guard(event, [ids]);
    if (!Array.isArray(ids) || ids.length > 20 || !ids.every(id => typeof id === 'string' && ID.test(id))) {
      throw new Error('Diagnostics request refused');
    }
    return { removed: store.remove(ids) };
  });
  const onCrumb = (event, crumb) => {
    if (!trusted(event)) return;
    const t = now();
    if (t - crumbStart >= 1000) { crumbStart = t; crumbCount = 0; }
    if (++crumbCount > 20) return;
    diagnostics.pageBreadcrumb(crumb);
  };
  ipc.on(CRUMB, onCrumb);

  // The person chose "Report the problem". The request waits until a signed-in page asks for
  // it (info().recovery.requested), so it is not lost if the page was still loading.
  function consume() { const was = requested; requested = false; return was; }
  function requestRecovery() {
    requested = true;
    if (!stopped && !contents.isDestroyed() && isPalisadeUrl(contents.getURL())) contents.send(RECOVERY);
  }
  function dispose() {
    if (stopped) return; stopped = true;
    ipc.removeHandler(INFO); ipc.removeHandler(PENDING); ipc.removeHandler(ACK); ipc.removeListener(CRUMB, onCrumb);
    contents.removeListener('destroyed', dispose);
  }
  contents.once('destroyed', dispose);
  return { requestRecovery, dispose };
}

module.exports = { installDiagnosticsBridge, PROTOCOL, INFO, PENDING, ACK, CRUMB, RECOVERY };
